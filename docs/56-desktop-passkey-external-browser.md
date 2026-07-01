# 56 - Desktop passkeys via the external browser

Status: DESIGN / spec (not yet implemented). Spec-before-code per root `CLAUDE.md`.

Owner-review required before implementation.

## 1. Goal

Give the desktop app (Tauri/Solid) a real WebAuthn passkey flow for **new-account
signup** and **account recovery**, reaching the same authenticators desktop users
actually have -- platform authenticators (iCloud Keychain, Windows Hello), software
providers (1Password, Bitwarden), and hardware security keys -- **on Windows,
macOS, and Linux alike**, and making passkey the **default** signup credential with
the recovery phrase kept as a fallback.

This removes the current sanctioned divergence in `desktop/CLAUDE.md` (lines
"Passkey / recovery divergence (sanctioned)") whereby desktop used the recovery
phrase as the only signup credential because the Tauri system WebViews cannot
perform WebAuthn reliably.

## 2. Why the external browser (decision)

Three approaches were considered (full analysis in the design discussion that
produced this doc):

- **A. In-app WebView `navigator.credentials`.** Rejected. webkit2gtk (Linux) ships
  no WebAuthn at all; WKWebView (macOS) requires the browser-only
  `com.apple.developer.web-browser.public-key-credential` entitlement; only
  WebView2 (Windows) works. Non-starter for cross-platform.
- **B. Native OS ceremony in Rust** (`webauthn.dll` / `ASAuthorization` Swift shim /
  `libfido2`). Viable but three separate native integrations; macOS needs an
  entitlement + AASA + notarization; Windows native callers get **no app<->domain
  binding** (any local app can claim `theavalanche.net`); Linux reduces to
  hardware-key-only and cannot reach 1Password (no system passkey provider on
  Linux). Kept as a possible future UX enhancement (section 12), not v1.
- **C. External browser + loopback handback (this spec).** The app opens the user's
  default browser to a page hosted at `https://theavalanche.net`, the browser runs
  the ceremony (with every provider/extension the user has, on every OS **including
  Linux**), and the result is handed back to the app over a `127.0.0.1` loopback
  channel. This is the OAuth-for-native-apps pattern (RFC 8252 loopback redirect).

Why C wins:

- **Uniform coverage, one implementation.** Same code path on all three OSes,
  including Linux and including software providers (the only way to reach 1Password
  on Linux).
- **Correct origin binding for free.** The ceremony runs on a real
  `https://theavalanche.net` origin, so the browser enforces WebAuthn origin binding
  natively. No Associated Domains entitlement, no per-app AASA, no
  assetlinks-for-native, no notarization-for-passkeys, and it sidesteps the Windows
  "native app can spoof any rpId" weakness.
- **Portable web passkeys.** The credential created is an ordinary
  `theavalanche.net` web passkey; it syncs via the user's provider and reproduces
  the **identical PRF -> identical DID** as iOS/Android for the same passkey.
- **No app-CSP relaxation.** The app WebView never runs WebAuthn, so the strict
  production content security policy (`desktop/CLAUDE.md` security constraints) is untouched.

Cost: a browser "bounce" (out to the browser and back). Acceptable for
signup/recovery, which are infrequent, high-stakes, and already a browser-bounce
experience in OAuth.

## 3. Non-goals

- No change to iOS/Android passkeys -- they already do native ceremonies. This
  brings desktop to **capability parity** using a mechanism suited to desktop; the
  cross-platform parity rule is satisfied at the capability level, not by copying
  the browser mechanism to mobile.
- No change to the Rust `crypto`/`store`/`app-core` recovery logic or the
  DID/PRF/recovery-blob contract. This spec is a new **consumer** of that contract.
- Not a messaging change -- DM/group parity rule is N/A.
- Native in-app ceremony (approach B) is explicitly deferred.

## 4. Load-bearing contract (must stay identical across web + iOS + Android)

The whole scheme depends on the browser ceremony producing PRF bytes that are
bit-identical to the mobile ceremonies for the same passkey. That requires all
three consumers to agree on:

- **RP ID** `theavalanche.net` -- matches iOS `PasskeyManager` and Android
  `PasskeyManager.RELYING_PARTY` (`mobile/android/.../Services/PasskeyManager.kt:49`).
- **PRF salt** `actnet-recovery-v1` -- matches Android `PasskeyManager.PRF_SALT`
  (`.../PasskeyManager.kt:57`) and iOS `PasskeyManager.prfSalt`.
- **`user.id` / userHandle = the signup server URL (UTF-8 bytes)** -- matches
  Android `register(... userHandle = signupServerUrl.toByteArray())`
  (`.../PasskeyManager.kt:95`), so `derive_did_from_passkey(prf, signup_server_url)`
  recomputes the same DID.
- **residentKey required** (discoverable credential) so recovery can find it with no
  `allowCredentials`.

Per root `CLAUDE.md` "never make a breaking contract change without review": this
spec **reuses** the above; it must not diverge from mobile. If the spike (section
10) shows the browser PRF differs from mobile PRF, that is a blocker, not a
license to fork the salt/RP/userHandle.

## 5. Architecture overview

```
Desktop app (Tauri)                         Default browser            theavalanche.net
------------------                          ---------------            ----------------
1. bind 127.0.0.1:<port>  (FIRST)
2. gen ephemeral X25519 (pk_pub/pk_priv)
3. open browser  ---- https://theavalanche.net/webauthn-bridge/?op=..&port=..&pk=..&nonce=..&user=.. -->
                                            4. GET page (real origin = theavalanche.net)
                                            5. navigator.credentials.create/get
                                               with extensions.prf.eval.first=salt
                                               (provider/platform/hw-key prompt)
                                            6. obtain 32-byte PRF (+ userHandle on auth)
                                            7. seal payload to pk_pub
8. loopback POST  <--- POST http://127.0.0.1:<port>/callback {nonce, sealed} (CORS) ---
9. verify nonce+origin, decrypt with pk_priv -> PRF bytes  (bytes stay in Rust)
10. call app-core FFI (create_account / recover_from_blob)
11. return non-secret AccountSummary to the Solid frontend
```

Key property: the 32-byte PRF secret is decrypted **in the Rust backend and never
enters the app WebView**. (This is stricter than today's phrase flow, which passes
the seed through JS; see section 11 assumptions.)

The WebAuthn ceremony MUST run on the hosted `theavalanche.net` page (so the rpId is
correct and the passkey is portable). The `127.0.0.1` listener is only the return
channel -- running WebAuthn on a loopback page would scope the passkey to
`localhost` and break cross-device recovery.

## 6. Wire protocol

### 6.1 Launch URL (app -> browser) -- all non-secret

```
https://theavalanche.net/webauthn-bridge/?
  v=1
  &op=register | authenticate
  &port=<loopback TCP port>
  &pk=<base64url X25519 ephemeral public key, 32 bytes>
  &nonce=<base64url 16 random bytes>            # binds response to this request
  &user=<base64url signup_server_url>           # register only -> user.id
  &name=<url-encoded display name>              # register only -> user.name/displayName
```

The PRF salt is the fixed constant `actnet-recovery-v1`; the page hardcodes it
(kept in sync with mobile). It is not passed in the URL.

### 6.2 Callback (browser -> app), POST `http://127.0.0.1:<port>/callback`

```
Content-Type: application/json
Origin: https://theavalanche.net
{ "v":1, "op":"register|authenticate", "nonce":"<echo>", "sealed":"<base64url>" }
```

Decrypted `sealed` payload:

```
{ "prf":"<base64url 32 bytes>",
  "userHandle":"<base64url signup_server_url>"   # authenticate only
  "credentialId":"<base64url>"                    # optional, debug/telemetry only
}
```

On failure the page POSTs `{v,op,nonce,error}` with `error` in
`{cancelled, prf_unsupported, no_credential, unknown}` so the app can surface a
clean message and fall back to the phrase flow.

### 6.3 Sealing (browser -> app confidentiality)

Recommended: an ECIES-style sealed box the page can build with **WebCrypto only**
(no new web dependency): page generates its own ephemeral X25519 keypair, does ECDH
with the app's `pk`, HKDF-SHA256 -> AES-256-GCM key, encrypts the payload, and sends
`{epk, iv, ct}` inside `sealed`. Rust side: `x25519-dalek` + `hkdf` + `aes-gcm`
(RustCrypto; prefer crates already in `Cargo.lock`).

Fallback for older browsers lacking WebCrypto X25519: bundle `libsodium.js`
(`crypto_box_seal`) on the page and use a libsodium-compatible sealed box in Rust
(`dryoc` or `crypto_box`'s SealedBox). Adds a WASM dep to the site and requires
`wasm-unsafe-eval` in the site `CSP`. Decide during the spike (section 10) based on
target-browser coverage. Default to the WebCrypto path.

## 7. Component changes

### 7.1 Web ceremony page (NEW -- `theavalanche.net`)

- `web/static/webauthn-bridge/index.html` + `web/static/webauthn-bridge/ceremony.js`
  (served at `/webauthn-bridge/`). Self-contained, minimal chrome, served over HTTPS.
- Correction to prior assumption: commit `b6694b2` ("fix passkeys on website") only
  added `web/static/.well-known/apple-app-site-association` and `assetlinks.json`
  (the native-app association files). There is **no existing WebAuthn ceremony page**
  on the site -- it is built new here. The `.well-known` files are unrelated to
  approach C (they support the native app paths) and can stay as-is.
- Responsibilities:
  - Parse launch params; validate `v`, `op`, `port`, `pk`, `nonce`.
  - `register`: `navigator.credentials.create()` with `rp.id=theavalanche.net`,
    `user.id=<signup server url bytes>`, `residentKey:"required"`,
    `pubKeyCredParams` ES256(-7)/RS256(-257), `extensions.prf.eval.first=<salt>`,
    a random challenge (no server attestation verification -- the credential's value
    is the PRF, not attestation). If PRF is absent on `create` output (common), do a
    follow-up `navigator.credentials.get()` on the new credential id with
    `prf.eval.first` to obtain the PRF (create-then-get).
  - `authenticate`: `navigator.credentials.get()` with `rpId=theavalanche.net`, no
    `allowCredentials` (discoverable), `prf.eval.first=<salt>`; extract PRF and
    `response.userHandle`.
  - Seal the payload to `pk` (section 6.3); POST to the loopback with the `nonce`
    echoed; the loopback returns permissive CORS for `https://theavalanche.net`.
  - Human-readable status: "Follow the prompt from your passkey provider", "Success
    -- return to Avalanche", and error states. `window.close()` is best-effort
    (browser-opened tabs usually cannot self-close) -- show a "you can close this
    tab" message.
- Mirror the field conventions of `mobile/android/.../PasskeyManager.kt` (the JSON
  builder there is the reference for options + PRF extraction).

### 7.2 `desktop/src-tauri/src/lib.rs` (Rust backend)

- New module (e.g. `passkey.rs`) providing:
  - An ephemeral loopback listener bound to `127.0.0.1:0` (OS-assigned port),
    single-shot, ~120s timeout. Minimal HTTP (`tiny_http` or `hyper`). Handles the
    CORS preflight, verifies `Origin == https://theavalanche.net` and the `nonce`,
    accepts exactly one POST, then shuts down.
  - Ephemeral X25519 keypair generation + seal-open (section 6.3).
  - Default-browser launch via `tauri-plugin-opener` (`open_url`) scoped to the
    fixed `https://theavalanche.net/webauthn-bridge` URL only.
- New Tauri commands (`#[tauri::command]`, `#[specta::specta]`):
  - `passkey_register(server_url, display_name, invite_token) -> AccountSummary`
    -- runs the register ceremony, then calls the already-exposed
    `AppCore::create_account(..., prf_output, ...)`
    (`core/crates/app-core/src/lib.rs:1309`; Tauri wrapper at
    `desktop/src-tauri/src/lib.rs:375`). PRF stays in Rust.
  - `passkey_authenticate() -> { did, signup_server_url, resolved_server_url? }`
    -- runs the authenticate ceremony, calls
    `derive_did_from_passkey(prf, signup_server_url)`
    (`core/crates/app-core/src/lib.rs:3311`) and, for `did:plc:*`,
    `resolve_homeserver_from_plc(did)` (`:3239`, called directly in Rust -- no new
    Tauri command needed). Stashes the PRF in a single pending-recovery slot in
    `AppState` (mutex-guarded, cleared on finish/cancel/timeout). Returns non-secret
    fields only.
  - `passkey_recover_finish(server_url, display_name) -> AccountSummary`
    -- consumes the stashed PRF and calls `recover_from_blob(server_url, did,
    prf_output, ...)` (`core/crates/app-core/src/lib.rs:1447`; Tauri wrapper at
    `:417`), then clears the slot.
- Register all three in the command list (`desktop/src-tauri/src/lib.rs:138-237`).
- Run `make desktop-bindings` to regenerate and commit
  `desktop/src/bindings.ts` (per `desktop/CLAUDE.md` FFI checklist step 7).

### 7.3 Rust core (`app-core`)

No changes. All required FFI already exists and is reused: `create_account`,
`recover_from_blob`, `derive_did_from_passkey`, `resolve_homeserver_from_plc`
(see section 7.2 citations). The ceremony is 100% outside the core, as on mobile.

### 7.4 Desktop frontend (Solid)

- `services/AvalancheService.ts` interface + `DevServerAvalancheService.ts`
  (real, via generated `commands.*`) + `MockAvalancheService.ts` (stub): add
  - `passkeyRegister(serverUrl, displayName, inviteToken): Promise<AccountSummary>`
  - `passkeyAuthenticate(): Promise<{did; signupServerUrl; resolvedServerUrl: string | null}>`
  - `passkeyRecoverFinish(serverUrl, displayName): Promise<AccountSummary>`
  Mock returns a deterministic fake DID/summary with no browser, for tests.
- `state/AppContext.tsx`: add `createAccountWithPasskey(...)` and the two-stage
  `recoverWithPasskey()` / `finishPasskeyRecovery(...)`. All account-entry paths must
  finish through the existing shared `enterApp()` helper (per `desktop/CLAUDE.md`
  "All account-entry paths converge on one enter-app step").
- Onboarding views (`desktop/src/views/onboarding/`):
  - Signup: add `PasskeyExplainerView.tsx` (+ `.css`) OR extend `NewAccountView.tsx`
    -- primary button "Create account with a passkey" (the new **default**),
    secondary "Use a recovery phrase instead" -> existing `RecoveryPhraseSetupView`.
    A waiting state ("Complete the passkey step in your browser, then return here")
    while the Tauri command is in flight; on `cancelled`/`prf_unsupported`, offer the
    phrase fallback.
  - Recovery: extend `RecoveryExplainerView.tsx` -- primary "Recover with a passkey"
    alongside the phrase path. Passkey path -> `passkeyAuthenticate()` ->
    `RecoveryConsoleView` seeded with `did` + `resolvedServerUrl` (prompt for the
    server URL when null, reusing the existing `needsServerUrl` UI in
    `RecoveryConsoleView.tsx:229-271`) -> `passkeyRecoverFinish(...)`.
  - `RecoveryConsoleView.tsx`: branch the finish call between `recoverFromPhrase`
    (existing) and `passkeyRecoverFinish` (new) based on how the console was entered;
    keep the manual-server-URL fallback.
  - Wire every `onBack` through `OnboardingFlow`'s back-stack (`goBack()`), never a
    hardcoded target.
- Styling: co-located `.css`, tokens from `src/styles/theme.css`, no inline styles
  (production CSP forbids them) -- per `desktop/CLAUDE.md`.

### 7.5 `desktop/src-tauri/tauri.conf.json` / capabilities

- Add the `tauri-plugin-opener` permission scoped to the single
  `https://theavalanche.net/webauthn-bridge` URL (do not grant open-any-URL).
- The loopback listener is native Rust (not a Tauri plugin) -- no capability entry;
  it binds `127.0.0.1` only.
- App production CSP unchanged (the app WebView never loads the ceremony page).
- Site (`theavalanche.net`) CSP: allow the page's own `'self'` script; add
  `wasm-unsafe-eval` only if the libsodium.js fallback (section 6.3) is chosen.

## 8. Security model

- **Interception of the PRF in transit** -> sealed to the app's ephemeral X25519
  public key; a co-resident process that sniffs the loopback POST sees only
  ciphertext.
- **Loopback port hijack** -> the app binds the port **before** launching the
  browser, so no other process can claim it. Loopback is preferred over a custom
  `avalanche://` scheme (any app can register a scheme, and the payload would leak
  into browser history/logs). No secret ever appears in a URL.
- **Stray/forged POSTs** -> listener verifies `Origin` and the `nonce`, accepts one
  POST, then closes; forged payloads can't be opened without the ephemeral private
  key and can't forge a valid PRF anyway.
- **Secret never in the WebView** -> decrypted and consumed in Rust.
- **Residual, accepted risks** (documented, not mitigated here, none a regression):
  - A malicious **browser extension** could read the PRF in-page -- identical to the
    risk of logging into the website itself; out of scope.
  - A malicious local app could *launch* the flow to phish an approval -- gated by
    the provider's user-verification prompt, same as every native path.
- **Deeper truth:** because the credential is bound to the RP ID + fixed salt (not
  to any app), the real protection on the recovery secret is **possession +
  user-verification of the authenticator** -- which is exactly what makes the DID
  reproduce across devices. This is inherent to the identity design (docs
  `50-identity-auth-recovery.md`), not introduced here.

## 9. Files touched (summary)

New:
- `docs/56-desktop-passkey-external-browser.md` (this doc) + map row in
  `docs/00-design.md` (done).
- `web/static/webauthn-bridge/index.html`, `web/static/webauthn-bridge/ceremony.js`.
- `desktop/src-tauri/src/passkey.rs`.
- `desktop/src/views/onboarding/PasskeyExplainerView.tsx` (+ `.css`).

Edited:
- `desktop/src-tauri/src/lib.rs` (3 commands + registration + AppState pending slot).
- `desktop/src-tauri/Cargo.toml` (loopback HTTP + sealed-box crates).
- `desktop/src-tauri/tauri.conf.json` (opener capability).
- `desktop/src/bindings.ts` (regenerated; committed).
- `desktop/src/services/{AvalancheService,DevServerAvalancheService,MockAvalancheService}.ts`.
- `desktop/src/state/AppContext.tsx`.
- `desktop/src/views/onboarding/{NewAccountView,RecoveryExplainerView,RecoveryConsoleView}.tsx`
  (+ any `.css`).
- `desktop/CLAUDE.md` (retire/adjust the "Passkey / recovery divergence (sanctioned)"
  section once implemented).
- `docs/61-desktop-implementation.md` (parity table: passkey rows).
- the onboarding manual test plan (kept outside the repo, intentionally not
  checked in -- add the scenarios in section 11).

No change: `core/crates/*` (Rust core).

## 10. Phase 0 -- feasibility spike (gates everything)

Before building, confirm the make-or-break assumption with a throwaway page +
minimal loopback:

1. Create a passkey via the ceremony page in a real browser and pull the 32-byte
   PRF; repeat and confirm the PRF is **stable** for the same (passkey, salt).
2. Confirm the PRF is **bit-identical** to what iOS/Android produce for the **same
   passkey** (create on the web page with 1Password, then authenticate on mobile
   with the same 1Password passkey; compare derived DIDs). This is the correctness
   gate for cross-device recovery.
3. Confirm **create-then-get** yields PRF with 1Password, iCloud Keychain, and a
   hardware key.
4. Decide the sealing path (WebCrypto X25519 vs libsodium.js) from target-browser
   coverage.

If (2) fails, stop -- the whole approach (and the shared-RP recovery model) needs
rethinking before any code lands.

## 11. Test plan

- **Rust unit/integration:** loopback accepts one valid POST and decrypts; rejects
  wrong `nonce`/`Origin`; times out cleanly; sealed-box round-trips against a known
  WebCrypto/libsodium test vector.
- **Mock service (frontend):** signup-with-passkey and recover-with-passkey run to
  completion with no browser, both converging on `enterApp()`; cancellation falls
  back to the phrase path; `tsc` + `npm run build` clean.
- **Manual E2E matrix (add to the onboarding manual test plan):**
  - Register: Windows(1Password), Windows(hardware key), macOS(iCloud),
    macOS(1Password), macOS(hardware key), Linux(Chrome+1Password),
    Linux(Firefox+hardware key).
  - Cross-device portability: create on desktop-browser, recover on mobile, and
    vice-versa (verifies PRF/DID equivalence end-to-end).
  - PLC resolution success + manual-server-URL fallback.
  - Cancel/deny in the browser -> clean fallback to phrase.
  - Loopback: timeout, port-in-use, a second POST rejected.
  - Security: POST to the loopback from a non-`theavalanche.net` origin -> rejected;
    confirm sealed payload can't be opened without the ephemeral key.

## 12. Deferred / future

- **Native in-app ceremony (approach B)** as an optional slicker path where the
  browser bounce chafes: Windows `webauthn.dll`, macOS `ASAuthorization` Swift shim
  (reuse the `.well-known/apple-app-site-association` already hosted), hardware keys
  via `libfido2`. Would coexist with the browser path (which stays the Linux and
  software-provider answer).
- **Tighten the phrase flow** to also keep its seed out of the WebView, matching the
  PRF-stays-in-Rust posture here.

## 13. Open questions / risks

1. **PRF bit-equivalence across providers/platforms** (spike, section 10) -- the
   central risk.
2. Sealing choice + site CSP impact (section 6.3).
3. Provider support for PRF-at-create vs create-then-get (section 7.1).
4. Browser focus return -- cannot reliably auto-close the tab; UX must guide the
   user back.
5. If the user's default browser is itself webkit2gtk-based (e.g. GNOME Web), the
   ceremony fails; mitigate with guidance + the phrase fallback (optionally detect
   and warn).
6. Self-hosted deployments depend on `theavalanche.net` hosting the ceremony page --
   already true of the shared-RP design on mobile; note in deployment docs.
7. Secret-in-Rust (pending slot) vs simpler secret-in-JS (parity with today's phrase
   flow). This spec recommends secret-in-Rust.
