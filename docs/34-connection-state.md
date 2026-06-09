# Connection state & offline UX

We need to know when the server is unreachable for the UI to display a warning.

## Problem

Three user-visible bugs share one root cause:

1. The offline banner sticks around after the WebSocket has reconnected, because iOS infers "online" only from a successful `receive_messages_ws` *return*, which blocks until traffic flows.
2. Launching the app while the server is unreachable shows a blank UI. `AppCore::login` performs a synchronous challenge/response (`login_inner` in `app-core/src/lib.rs:1285`); when that fails, no `AppCore` is constructed, so the local-DB-backed conversation list never renders.
3. The offline banner only appears after a delivery failure — not at launch, even when the server is plainly unreachable.

The root cause is that **connection state is not modelled anywhere**. iOS infers it from receive-loop side effects, and the Rust core treats every operation as one-shot. There is no observable "are we online?" signal.

## Goal

Make `AppCore` the single source of truth for connection state. iOS renders directly from that state, with no inference, no flicker, and no stuck banner.

## Design

### `ConnectionState` enum

A new public type, exported via UniFFI:

```rust
pub enum ConnectionState {
    /// Initial state, or after explicit logout. No reconnect task running.
    Disconnected,
    /// An attempt is in flight (handshake, lazy auth).
    Connecting,
    /// WebSocket is open. Steady state.
    Connected,
    /// Last attempt failed; backing off until `next_attempt_at_ms`.
    Reconnecting { next_attempt_at_ms: i64 },
}
```

`next_attempt_at_ms` is owned by the core — iOS no longer computes the countdown, it just subtracts from "now".

### State ownership

`AppCore` gains:

```rust
state_tx: tokio::sync::watch::Sender<ConnectionState>,
reconnect_task: Mutex<Option<JoinHandle<()>>>,
event_tx: tokio::sync::mpsc::UnboundedSender<IncomingEvent>,
event_rx: Mutex<tokio::sync::mpsc::UnboundedReceiver<IncomingEvent>>,
```

`state_rx` is **not** stored — each waiter calls `state_tx.subscribe()` to get its own receiver. This keeps the API simple (no shared rx lifetime) and supports any number of concurrent waiters.

The `watch` channel is the canonical state observable. The `mpsc` channel carries a single sum type:

```rust
pub enum IncomingEvent {
    Message(DecryptedMessage),
    ReceiptUpdate(DeliveryStatusUpdate),
}
```

This collapses what is currently two FFI paths (`receive_messages_ws` + `drain_receipt_updates`) into one. The background task emits whichever event type it produces; iOS drains a batch per call. Ordering between messages and receipts is preserved.

`publish()` is a thin wrapper around `state_tx.send_if_modified(|s| { ... })` so we never notify waiters about no-op transitions (e.g. `Reconnecting{1000}` → `Reconnecting{1000}`). Requires `ConnectionState: PartialEq + Clone`.

### Background reconnect task

`AppCore::login` / `AppCore::create_account` / `AppCore::finalize_account` / `AppCore::recover_from_blob` all spawn the reconnect task before returning. No separate `start()` FFI — the task is an invariant of a constructed `AppCore`.

The task is the only thing that touches the WebSocket; FFI methods read state via the watch channel and events via the mpsc.

Pseudocode:

```rust
async fn reconnect_loop(core: Weak<AppCore>) {
    let mut backoff_sec: u64 = 1;
    loop {
        let Some(core) = core.upgrade() else { break };
        core.publish(ConnectionState::Connecting);

        match core.try_connect_ws().await {
            Ok(mut ws) => {
                backoff_sec = 1;
                core.publish(ConnectionState::Connected);
                core.run_receive_loop(&mut ws).await; // returns on error/close
                // fall through to Reconnecting
            }
            Err(e) => {
                tracing::warn!("ws connect failed: {e}");
            }
        }

        // Jittered backoff to avoid synchronised retries across reinstalls / restarts.
        let jitter = rand::random::<f64>() * 0.5 + 0.75; // 0.75x–1.25x
        let sleep_ms = ((backoff_sec as f64) * 1000.0 * jitter) as i64;
        let next_ms = now_ms() + sleep_ms;
        core.publish(ConnectionState::Reconnecting { next_attempt_at_ms: next_ms });
        drop(core); // release Arc before sleeping so AppCore can drop
        tokio::time::sleep(Duration::from_millis(sleep_ms as u64)).await;
        backoff_sec = (backoff_sec * 2).min(30);
    }
}
```

`try_connect_ws` handles lazy challenge/response when the in-memory token is missing. To avoid holding `inner` lock across both the HTTP challenge and the WS handshake (which would block `send_dm` for seconds), the implementation clones the bits it needs (server URL, token, identity public key handle) out of the lock before issuing the network calls.

`run_receive_loop` pulls messages off the WS, decrypts them, and pushes one `IncomingEvent::Message` per content message and one `IncomingEvent::ReceiptUpdate` per receipt into `event_tx`. Both `connect` and `receive` failures fall through to the backoff branch.

The task is cancelled by `AppCore` drop: each iteration starts with `Weak::upgrade()`, and the long sleep happens with no strong refs held, so dropping the last FFI Arc causes the next iteration's upgrade to fail and the task to exit. Worst-case shutdown latency is the in-flight network call (TCP connect timeout, typically ≤30s). Acceptable for v1; a `CancellationToken` is a clean future extension.

### FFI surface

Three methods added, two removed.

```rust
#[uniffi::export]
impl AppCore {
    /// Cheap snapshot of current state. Non-blocking.
    pub fn connection_state(&self) -> ConnectionState { ... }

    /// Block until state differs from `last`, return the new state.
    /// iOS runs this in a tight loop on a dedicated task.
    pub fn wait_for_connection_state_change(&self, last: ConnectionState)
        -> Result<ConnectionState, AppErrorFfi> { ... }

    /// Block until at least one event is available, drain the queue.
    /// Single-consumer: concurrent callers serialize on the receiver mutex
    /// (they don't deadlock, but only one returns at a time).
    pub fn next_events(&self) -> Result<Vec<IncomingEvent>, AppErrorFfi> { ... }
}
```

Implementation note for `wait_for_connection_state_change`: subscribe **before** comparing to `last`, so a transition that races with the call is not missed.

```rust
let mut rx = self.state_tx.subscribe();
let current = rx.borrow().clone();
if current != last { return Ok(current); }
rx.changed().await.map_err(...)?;
Ok(rx.borrow().clone())
```

**Removed:** `receive_messages_ws` and `drain_receipt_updates`. The background task owns the WS; iOS reads everything through `next_events`.

**Not added:** `connect_ws`, `reconnect_now`. The background task probes continuously; explicit probe is redundant. (`reconnect_now` will become useful when the deferred `NWPathMonitor` integration lands — design space called out below.)

### `login` becomes offline-safe

`login_inner` is rewritten:

```rust
async fn login_inner(store: store::Store) -> Result<AppCoreInner, AppError> {
    let identity = store.load_identity().await?.ok_or(AppError::NoAccount)?;
    let reg = store.load_registration().await?.ok_or(AppError::NoAccount)?;

    // No network call here. Build an unauthenticated client.
    let client = net::Client::new(&reg.server_url);

    Ok(AppCoreInner {
        store, client,
        local_address: DeviceAddress::new(AccountId::new(&reg.account_id), DeviceId::new(reg.device_id)),
        did: reg.account_id,
        device_id: reg.device_id,
    })
}
```

The reconnect task performs challenge/response on demand the first time it tries to open the WS. Tokens are still ephemeral in memory — we are not adding token persistence in this PR. (If the WS reconnects on backoff after a 401, the task simply does challenge/response again.)

### Lazy challenge/response + transparent 401 retry

The offline-safe `login` creates a `Client` with no session token, so token management has to move inside `Client`. The reconnect task can no longer be the only place that thinks about tokens — every authenticated HTTP call needs to ensure-or-acquire a token, and to recover from a server-side expiry mid-session.

To keep layering clean, `net::Client` gains a `Signer` abstraction. The signer captures whatever the implementer needs to produce an Ed25519 signature over a server nonce — for `app-core` that's the identity private key from `store`. `net` stays unaware of crypto/store types.

```rust
// net::
pub trait Signer: Send + Sync {
    fn sign(&self, nonce: &[u8]) -> Result<Vec<u8>, NetError>;
}

pub struct Client {
    server_url: String,
    http: reqwest::Client,
    signer: Option<Arc<dyn Signer>>,
    auth: RwLock<Option<AuthState>>,  // { did, device_id, token }
}

impl Client {
    pub fn new(server_url: &str) -> Self { ... }  // no signer — only registration paths
    pub fn with_signer(mut self, did: String, device_id: u32, signer: Arc<dyn Signer>) -> Self { ... }

    /// Acquire a token if we don't have one. Idempotent under concurrent callers.
    async fn ensure_authenticated(&self) -> Result<(), NetError> {
        // Fast path: token already present.
        if self.auth.read().await.as_ref().and_then(|a| a.token.as_ref()).is_some() {
            return Ok(());
        }
        // Slow path: take write lock, double-check, do challenge/response.
        let mut auth = self.auth.write().await;
        if auth.as_ref().and_then(|a| a.token.as_ref()).is_some() { return Ok(()); }
        let signer = self.signer.as_ref().ok_or(NetError::NoSigner)?;
        let did = auth.as_ref().map(|a| a.did.clone()).ok_or(NetError::NoSigner)?;
        let device_id = auth.as_ref().map(|a| a.device_id).unwrap();
        // Drop the lock during network I/O? — see "Concurrent re-auth" risk below.
        let nonce = self.challenge_raw(&did, device_id).await?;
        let sig = signer.sign(&nonce)?;
        let token = self.authenticate_raw(&did, device_id, &nonce, &sig).await?;
        auth.as_mut().unwrap().token = Some(token);
        Ok(())
    }

    /// Internal helper: every authenticated method goes through here.
    /// Issues the request; on 401, drops the token, re-auths, retries once.
    async fn request_authenticated<F, Fut, T>(&self, build: F) -> Result<T, NetError>
    where F: Fn(String /*token*/) -> Fut, Fut: Future<Output = Result<reqwest::Response, NetError>>
    {
        self.ensure_authenticated().await?;
        let token = self.current_token();
        let resp = build(token).await?;
        if resp.status() != StatusCode::UNAUTHORIZED {
            return parse(resp).await;
        }
        // Token rejected — clear, re-auth, retry exactly once.
        self.auth.write().await.as_mut().map(|a| a.token = None);
        self.ensure_authenticated().await?;
        let token = self.current_token();
        parse(build(token).await?).await
    }
}
```

Every existing authenticated method in `Client` (`send_dm`, `fetch_messages`, `upload_prekeys`, `get_account_info`, etc.) gets wrapped in `request_authenticated`. The reconnect task no longer needs its own ensure helper — opening the WS calls `ensure_authenticated` on the Client, same as any HTTP send.

In `app-core`, the `IdentitySigner` adapter wires store → signer:

```rust
// app-core::
struct IdentitySigner { identity: IdentityKeyPair }

impl net::Signer for IdentitySigner {
    fn sign(&self, nonce: &[u8]) -> Result<Vec<u8>, NetError> {
        self.identity.private_key()
            .calculate_signature(nonce, &mut rand::rngs::OsRng.unwrap_err())
            .map(|s| s.into_vec())
            .map_err(|e| NetError::Crypto(e.to_string()))
    }
}

// at construction:
let identity = store.load_identity().await?.ok_or(AppError::NoAccount)?;
let signer = Arc::new(IdentitySigner { identity });
let client = net::Client::new(&reg.server_url).with_signer(reg.account_id.clone(), reg.device_id, signer);
```

**What this fixes:** the two regressions that the bare "no upfront login" design would introduce:

1. **Launch-window race** — user sends before reconnect task has auth'd. `send_dm` calls `request_authenticated` → ensure → challenge/response → retry. Succeeds.
2. **Silent token expiry** — server expires our token. Next `send_dm` gets 401 → drop token → re-auth → retry. Succeeds. The reconnect task's WS doesn't even need to know; if the server invalidates the WS too, the receive loop errors and the reconnect task picks up a fresh token.

**Concurrent re-auth.** Two parallel HTTP calls both seeing no token would each attempt challenge/response. The `RwLock<AuthState>` with double-check pattern serialises them — first writer does the round trip, second writer sees the token populated and returns. Acceptable; the cost is one wasted nonce request in the race window, no behavioural issue.

**Lock-during-IO.** The pseudocode above holds the auth write-lock across the challenge + authenticate round trip. That blocks parallel calls for ~1 RTT during a re-auth. Cleaner to drop the lock during I/O and re-acquire to write the token — adds complexity but avoids tail-latency hits. Implementation decision; not load-bearing for the design.

### iOS wiring

`AppState` gains a per-account state map (multi-account support):

```swift
@Published var connectionStates: [String: ConnectionState] = [:]  // keyed by DID
private var stateTasks: [String: Task<Void, Never>] = [:]
private var eventTasks: [String: Task<Void, Never>] = [:]

/// Derived: the "worst" state across all accounts, used by the banner.
var aggregateConnectionState: ConnectionState {
    // Connected wins if all are Connected; otherwise pick the most-offline
    // state with the earliest next_attempt_at_ms.
}
```

Multi-account aggregation rule: if any account is `Reconnecting` or `Connecting`, the banner shows. If all are `Connected`, banner hides. Earliest `next_attempt_at_ms` wins the countdown. Pragmatic for v1 — we can switch to per-account banners later if users have accounts on multiple servers in practice.

`restoreAccounts`:
1. Opens `AppCore` for each account (now pure-local; no network).
2. Flips `isOnboarding = false`.
3. `loadConversationsFromStore()` — works from local DB.
4. For each account, starts a `stateTask` (loops `wait_for_connection_state_change`, publishes into `connectionStates[did]`) and an `eventTask` (loops `next_events`, routes messages and receipt updates to existing handlers).

`OfflineBanner` reads `appState.aggregateConnectionState`:
- `.connected` → hidden.
- `.connecting` → shown, "Reconnecting…" + spinner, no countdown.
- `.reconnecting(next)` → shown, "Offline · retrying in Ns" with 1Hz countdown.
- `.disconnected` → shown, "Offline" (transient; shouldn't persist).

**Removed from `AppState`:** `isOffline`, `nextReconnectAtMs`, `messageWsLoop`, `markOffline`, `markOnline`, `wsLoopTasks`, `startMessagePolling` (replaced by the per-account task launchers).

**`sendMessage`:** unchanged. If the HTTP send throws, the message bubble flips to `.failed` per the recent change. The banner is no longer driven by sends.

**Logging:** the reconnect task uses `tracing`; for this PR those events stay in the Rust log only (Xcode console). Surfacing them in the in-app log viewer is deferred — see "Out of scope".

## Migration

No schema migration. No protocol change. All changes are within `app-core` and the iOS app. Android port is a follow-up.

## Risks

- **Lock held across long network I/O.** If `try_connect_ws` holds `inner` lock during the HTTP challenge + WS handshake, `send_dm` and other inner-using FFI calls block for seconds. Mitigation: clone client + identity handle out of the lock before issuing network calls.
- **Cancellation of the reconnect task on logout.** Dropping the last `Arc<AppCore>` causes `Weak::upgrade` to fail on the next iteration; the task exits cleanly. Worst case latency = the in-flight `connect_async` or `next_message` call (TCP timeout, ≤30s). A `CancellationToken` is a clean upgrade later if needed.
- **Spurious state transitions.** Use `state_tx.send_if_modified(...)` in `publish()` so duplicate state writes don't wake iOS. Requires `ConnectionState: PartialEq`.
- **First-launch transient `Disconnected`.** AppCore is constructed in `Disconnected`; the reconnect task runs ~immediately and transitions to `Connecting`. iOS may briefly observe `Disconnected` if it subscribes in that window. Banner code treats `Disconnected` like `Connecting` visually so this is not a flicker risk.
- **Multi-account state explosion.** N accounts × 2 background tasks each = up to ~6 long-lived Tasks for a power user. Each is an `await` on a channel; idle cost is essentially zero. Acceptable.
- **`next_events` single-consumer.** Wrapped in `Mutex<Receiver>`. If two iOS tasks accidentally both call it, they serialise on the mutex (one returns events, the other waits). Not a deadlock but worth documenting in the FFI doc comment.
- **Concurrent FFI calls during reconnect.** `send_dm` etc. continue to use the HTTP client and don't touch the WS. If the HTTP call is mid-flight when the network drops, it fails as today. No new races.
- **Testbot / integration test fallout.** Any existing test that calls `receive_messages_ws` or `drain_receipt_updates` directly must switch to `next_events`. Audit `core/crates/app-core/tests/` and the testbot during implementation.

## Out of scope (follow-ups)

- **WS-level keepalive pings** (silent-death detection — TCP-open but server-unresponsive). Server already has a ping/pong path; client needs to schedule sends and treat missed pongs as disconnect.
- **`NWPathMonitor` integration on iOS** — immediate retry on OS-reported network change. Needs a new `reconnect_now()` FFI that signals the reconnect task (via `tokio::sync::Notify`) to skip its current sleep.
- **Core logs in the in-app viewer.** Today the reconnect task's `tracing` events only land in the Xcode console — the in-app log viewer can't see them. Options (UniFFI callback `LogSink`, OSLog/`OSLogStore` integration, or a hybrid) discussed separately; deferred until we pick one.
- **Session token persistence across launches.** Tokens are in-memory only. First launch (and any launch after a process kill) re-auths via challenge/response. Acceptable cost; the larger signed-request auth migration would obviate tokens entirely.
- **Android port** — mirror the iOS task layer and banner.

## Implementation order

Each step is independently reviewable; PR can be one commit per step or all-in-one depending on appetite.

1. **Types.** `ConnectionState` enum + `IncomingEvent` enum + UniFFI exports. Add `PartialEq + Clone` derives.
2. **`net::Signer` trait + `Client` token internalisation.** Move token state into `Client` (RwLock). Add `with_signer`, `ensure_authenticated`, `request_authenticated`. Wrap every existing authenticated method to go through `request_authenticated` (handles 401 retry transparently).
3. **`app-core::IdentitySigner` adapter.** Implements `net::Signer` using the identity key from store.
4. **`AppCore` plumbing.** Add fields (`state_tx`, `event_tx`, `event_rx`, `reconnect_task`); wire into the constructor paths so they start in `Disconnected`.
5. **Offline-safe `login_inner`.** Remove the challenge/response call; build the `Client` with `with_signer` and no token. Tokens populate on first authenticated call (either reconnect-task WS open or any HTTP).
6. **Reconnect task.** `try_connect_ws` (calls `client.ensure_authenticated()` then opens WS), `run_receive_loop`, backoff loop with jitter. Spawn from `login` / `create_account` / `finalize_account` / `recover_from_blob` before returning the `Arc`.
7. **New FFI methods.** `connection_state`, `wait_for_connection_state_change`, `next_events`.
8. **Remove old FFI methods.** `receive_messages_ws`, `drain_receipt_updates`. Update any in-tree Rust callers (testbot, integration tests).
9. **iOS plumbing.** `AppState.connectionStates`, per-account `stateTask` + `eventTask`, route events into existing handlers.
10. **iOS UI.** Rewrite `OfflineBanner` to read `aggregateConnectionState`. Delete `isOffline`, `nextReconnectAtMs`, `messageWsLoop`, `markOnline`, `markOffline`, `startMessagePolling`.
11. **Manual test plan:**
    - Kill server pre-launch → app launches, cached conversations visible, banner shows with countdown.
    - Restart server → banner clears within current backoff window.
    - Send message while offline → bubble shows `.failed`, banner stays visible.
    - **Send message during launch window** (within ~1s of opening app with server up) → succeeds (validates 401-retry / lazy-auth on first send).
    - **Manually invalidate session token server-side** (delete from `sessions` table), then send a message → succeeds transparently (validates 401 retry).
    - WS disconnect mid-session (kill server, leave app running) → banner appears, retries, dismisses on restart.
    - Two accounts on different servers, one down → banner shows; bring server back → banner clears.
    - Force-quit + relaunch with server down → reproduces (1).
    - Background app for 5 min → foreground → connection re-established quickly (verifies the loop is still running).
