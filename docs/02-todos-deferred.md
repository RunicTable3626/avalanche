# Deferred TODOs

## Dev Infra
- Make simple makefile commands to build the entire ios -- currently we can build the Rust code, but I think it's worth having a command that runs xcodebuild, since Claude likes to run it to ensure its changes compile. In general we should make 'compile everything' commands at the root with a Makefile.

## Mobile app
- Mobile app 'console': nerdly scrolling log which appears during long loads and debugging tools (currently everything is fast so maybe not needed)
- Account recovery is not yet implemented / working
- Written-down recovery phrase alternative to passkey (generate memorable phrase, encrypt recovery blob with it, cache derived key in Secure Enclave)
- Delivery receipts — auto-send on message receive (see docs/31-read-tracking.md, Stage D)
- Read receipt user preference toggle (send_read_receipts setting)
- Scroll position: remove invisible "bottom" anchor hack in ConversationView (Color.clear spacer) when scroll position saving is implemented
- Banner/notification for incoming messages while app is in foreground
- Offline indicator (show when server is unreachable / WS disconnected)
- Persist message history locally (currently messages are only in memory)
- Account switcher UI for multi-account support
- My QR Code screen uses `accounts.first` — should use the active/selected account once multi-account is implemented

## Privacy / identity
- PLC directory privacy: the DID document currently includes the homeserver URL as a service endpoint, which means anyone can resolve a DID and learn which server a user is on. For small servers this effectively leaks group membership. Consider removing the homeserver URL from the PLC document entirely and relying on out-of-band discovery (invite links, contact exchange). The PLC document would only contain the identity key for verification.
- DID update operation for key rotation after recovery (submit new signing key to PLC directory, signed by rotation key)
- Re-encrypt and re-upload recovery blob to all servers when joining a new server (update server list)
- Cache recovery derived key in Secure Enclave so re-encryption doesn't require re-prompting passkey/phrase
- Consider whether we want to bother moving the persisted account list out of UserDefaults into a Secure-Enclave-keyed SQLCipher `manifest.db`. Today the list of accounts (own DID, display name, server URLs, db filename) lives in UserDefaults, which is encrypted at rest by the device data-protection class but not by a user-controlled key. An attacker pulling the iOS sandbox snapshot gets the list of homeservers the user is on plus their own DIDs — enough to link the device to specific orgs. The contact graph and message history are not exposed (they're inside the SQLCipher per-account DBs) so it's maybe not that important. A small manifest DB keyed from the Secure Enclave (same approach as the per-account DBs) could list the other DBs while closing this particular loophole.
- Contact list backup: we're interested in persisting the user's contacts separately from their identity keys, in hopes that if they lose identity keys at least they can reestablish contact with the people they were previously communicating with under a new ID. The contacts aren't that sensitive, but the tricky bit is that each of your contact is attached to one of your own identities and we don't want to mix them up. You might also want to be able to manually export your contacts list in some standard format that can be processed by other apps too.

## Android app

The iOS app (`mobile/ios/`) is the reference implementation. The Android app (Kotlin/Jetpack Compose) should mirror it structurally. See `docs/33-android.md` for the full implementation guide, including directory structure, iOS→Android mapping table, build setup, and code sketches for each layer.

### Infrastructure
- [ ] Scaffold Gradle project at `mobile/android/` (see `docs/33-android.md` §3 for directory structure)
- [ ] Add `make android-ndk` Makefile target: compile `libapp_core.so` for `arm64-v8a` and `x86_64` via `cargo-ndk`
- [ ] Add `make android` Makefile target: `make bindings` + `make android-ndk`
- [ ] Update `CLAUDE.md` build commands section to document Android targets and prerequisites
- [ ] Configure `gradle/libs.versions.toml` with Compose BOM, Navigation, ViewModel, DataStore, JNA, CameraX, ML Kit

### Core layer
- [ ] `AppCoreInterface.kt` — Kotlin interface mirroring `AppCoreProtocol` from `ActnetService.swift`
- [ ] `MockActnetService.kt` + `MockAppCore` — mock implementation (mirrors `MockActnetService.swift`): 100 ms send delay, echo reply after 1.5 s, seed conversations on `createAccount`
- [ ] `DevServerActnetService.kt` — wraps UniFFI-generated `AppCore` class directly
- [ ] `AppViewModel.kt` — `AndroidViewModel` with `StateFlow` (mirrors `AppState.swift`):
  - `restoreAccounts()` called from `init`; loads from `DataStore<Preferences>`
  - Per-account `AppCoreInterface` instances in `MutableMap<String, AppCoreInterface>`
  - WebSocket loop as `viewModelScope.launch(Dispatchers.IO)` coroutine per account, 2 s backoff on error
  - `ServiceMode` enum (MOCK / DEV_SERVER); switching mode clears all state

### Models
- [ ] `Account.kt` — data class with `id` (DID), `displayName`, `avatarData`, `servers`
- [ ] `Conversation.kt` — `@Serializable`; exclude `lastMessage` from JSON (`@Transient`) for same security reason as iOS
- [ ] `Message.kt` — `DeliveryStatus` enum (SENDING/SENT/DELIVERED/READ matching raw Int values from iOS)
- [ ] `ProjectInfo.kt`, `InviteToken.kt`

### UI — Onboarding
- [ ] `SplashScreen.kt` — scan QR / enter invite link entry points
- [ ] `InviteLinkEntryScreen.kt` — parse `actnet://invite/<server>/<token>` deep links
- [ ] `IdentityPickerScreen.kt` — existing account list + "Create fresh identity"
- [ ] `NewAccountScreen.kt` — display name input, optional avatar, calls `vm.createAccount(…)`
- [ ] `JoiningServerScreen.kt` — join new server with existing account
- [ ] `QRScannerScreen.kt` — CameraX preview + ML Kit `BarcodeScanning.getClient()`

### UI — Chats tab
- [ ] `ChatsScreen.kt` — `LazyColumn` sorted by `lastMessageDateMs`; unread badge; FAB for compose
- [ ] `ConversationScreen.kt` — message thread, auto-scroll to bottom, mark read on appear
- [ ] `MessageBubble.kt` — sent (right, blue) / received (left, gray); delivery icons ⏱/✓/✓✓gray/✓✓blue
- [ ] `ComposeMessageScreen.kt` — recipient DID input, account picker

### UI — Calls tab
- [ ] `CallsScreen.kt` — placeholder (mirrors iOS `CallsView.swift`)

### UI — Network tab
- [ ] `NetworkScreen.kt` — server/project list, async load, token request on tap
- [ ] `ProjectWebScreen.kt` — `AndroidView { WebView(context) }` for Project UIs

### UI — Common
- [ ] `AccountAvatar.kt` — avatar composable with initials fallback (mirrors `AccountAvatar.swift`)
- [ ] `DevSettingsScreen.kt` — service mode toggle, account/conversation counts (mirrors `DevSettingsView.swift`)

### Permissions & manifest
- [ ] `INTERNET` and `CAMERA` permissions
- [ ] `POST_NOTIFICATIONS` permission (API 33+) for future push
- [ ] `actnet://invite` intent filter for deep links (mirrors iOS URL scheme)
- [ ] FCM service stub for when push notifications are implemented

### Testing
- [ ] `MockServiceTest.kt` — verify `MockAppCore.receiveMessagesWs()` delivers echo reply after ≥1.5 s
- [ ] Cross-platform interop test: iOS sends encrypted DM, Android decrypts it against a real test homeserver (add to `core/crates/app-core/tests/`)
## Crypto / protocol
- Stale device detection: when a device re-registers (new identity key, new prekeys), the server should reject messages sent to the old device state. `POST /v1/messages` should check that the sender's session is compatible with the recipient's current registration (e.g., compare `registration_id`). On rejection, the sender's client should fetch the new prekey bundle and re-establish the session. Without this, messages encrypted to old keys are silently undeliverable after a key reset.

## Server
- WebSocket request/response framing: tunnel HTTP-style request/response pairs over the WebSocket (like Signal does), with request IDs and correlated responses. Move message sends and acks onto the WS transport, replacing the current split of HTTP sends + WS acks. This gives persistent-connection benefits while keeping clear success/failure semantics per operation.
- Timer change sync message: add a `TimerChangeMessage` body variant to the ContentMessage protobuf so that when a user changes the conversation expiry timer, a control message is sent to the other participant(s) to update their local setting

## Project-wide
- Mass rename: rename repo, update bundle IDs, update all remaining `actnet` references in code and docs to `avalanche`

## Big milestones (not yet started)
- Groups: action-bound (zkgroup) and cross-server casual (Sender Keys)
- Invite links & onboarding: QR codes, deep links, auto-enrollment into groups/Projects
- Projects framework: SDK, scoped bot permissions, JS bridge for webviews
- First-party Projects: channel directory, team assignment, action-day map, Q&A bot, collab docs, engagement tracking
- Federation: server-to-server protocol, cross-server DMs, full DID portability (PLC directory), guest access
- Android app (see `docs/33-android.md` for full implementation guide)
- Calls: voice and video (VoIP)
- Public profiles: client-owned profile blobs (display name, avatar, bio) pushed to servers
- Multi-account support in mobile app

## Mesh Fallback / BitChat protocol (optional — implement only after core features are stable)

See `docs/32-bitchat-fallback.md` for the full design. BLE mesh transport as a fallback when the homeserver is unreachable.

## Push Notifications

### 1. Push relay service (`core/crates/relay/`)
- [ ] DB table: `(pseudonym) → (device_token, platform, registered_at)`
- [ ] Client endpoint: register/update/delete pseudonym-to-token mapping
- [ ] Homeserver endpoint: accept wakeup-by-pseudonym, fire content-free push to APNs/FCM
- [ ] Pseudonym rotation: grace period (~1 week) where old pseudonym still works
- [ ] APNs integration (content-free wakeup payload)
- [ ] FCM integration (content-free wakeup payload)

### 2. Server integration
- [ ] On message delivery to offline device, look up push pseudonym and ping relay
- [ ] Hook into existing WebSocket connection tracking to determine online/offline
- [ ] Server config: relay URL

### 3. Mobile client (iOS first, then Android)
- [ ] Request push permission during signup
- [ ] Register device token with APNs/FCM
- [ ] Register per-(user, server) pseudonym with relay on account creation
- [ ] On wakeup: connect WebSocket, fetch queued messages
- [ ] Periodic pseudonym rotation (default weekly)
- [ ] Opt-out setting for high-risk users (poll-only mode)

### 4. Testing & privacy
- [ ] Verify relay payloads contain zero user-identifiable content
- [ ] Verify relay logs contain only pseudonyms + timestamps
- [ ] Pseudonym rotation grace period test
- [ ] APNs/FCM sandbox integration test

## Desktop client (future)

A desktop client (macOS, Windows, Linux) can share most of its codebase with the Android app via **Kotlin Multiplatform + Compose Multiplatform** (JetBrains). The UniFFI-generated Kotlin bindings use JNA under the hood, and JNA loads native libraries on the desktop JVM too (`.dylib` on macOS, `.so` on Linux, `.dll` on Windows) — so the same `AppCoreInterface` / `AppViewModel` layer from the Android app is reusable with minimal changes.

Defer until the Android app reaches a stable milestone.

- [ ] Evaluate Compose Multiplatform maturity for production desktop use
- [ ] Add `make desktop-macos`, `make desktop-linux`, `make desktop-windows` Makefile targets to compile `libapp_core` as a shared library for each OS (requires cross-compilation toolchain setup)
- [ ] Scaffold a `desktop/` Kotlin Multiplatform module that shares `AppViewModel`, models, and service interfaces from `mobile/android/`
- [ ] Handle per-OS secure storage: macOS Keychain, Windows Credential Manager, Linux Secret Service / `libsecret`
- [ ] Handle per-OS notification APIs: macOS `UserNotifications`, Windows `Windows.UI.Notifications`, Linux `libnotify`
- [ ] Handle per-OS deep link / URL scheme registration for `actnet://`

## Push Notifications (remaining work)
- Android client: FCM token registration, pseudonym lifecycle, wakeup handling
- Relay: real APNs sending via `a2` crate (env vars: APNS_KEY_PATH, APNS_KEY_ID, APNS_TEAM_ID, APNS_BUNDLE_ID)
- Relay: real FCM sending
- iOS: periodic pseudonym rotation (weekly timer)
- iOS: opt-out setting for high-risk users (poll-only mode)
- Testing: verify relay payloads contain zero user-identifiable content; APNs/FCM sandbox integration test
