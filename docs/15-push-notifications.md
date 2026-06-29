# Push Notifications

## Platform dispatch

| Platform | Mechanism |
|---|---|
| iOS | APNs via push relay |
| Android (standard) | FCM via push relay |
| Android (degoogled) | UnifiedPush via push relay if a distributor is installed; foreground WebSocket otherwise |
| Desktop (Tauri) | WebSocket frame → local OS notification via `tauri-plugin-notification`; no external push |

**All three external transports route through the relay under the same rotating
pseudonym.** The homeserver only ever POSTs content-free wakeups addressed to
pseudonyms (`/v1/wakeup`); it never holds a device token, FCM token, or endpoint
URL, and never POSTs to a third party itself. The relay stores `(pseudonym →
token, platform)` and dispatches by `platform`: `apns` → APNs, `fcm` → FCM HTTP
v1, `unifiedpush` → POST to the stored endpoint URL. The **client** picks which
transport to register; to the homeserver they are indistinguishable. Desktop
never registers a push token.

## Relay / privacy model (all external transports)

Homeservers never hold device tokens. Instead they send content-free wakeups to per-(user, server) **pseudonyms** at the push relay (`https://relay.theavalanche.net`, not yet deployed). The relay maps pseudonyms → tokens and fires empty payloads. Apple/Google/the distributor see only a ping; the relay sees pseudonym-level timing but no identity, content, or cross-server linkage. Pseudonyms rotate periodically to limit linkability. High-risk users can opt out and poll manually. Multiple relays are supported so the Avalanche-operated relay is not a privileged singleton. See `docs/41-relay-deployment.md` for ops.

## FCM (standard Android)

The client registers its FCM token with the relay as platform `fcm`. The relay sends via **FCM HTTP v1**, minting an OAuth2 access token by signing a short-lived RS256 JWT with a service account (the legacy server-key API is decommissioned). Wakeups are **data-only, high-priority** messages (no `notification` block, so the app's `onMessageReceived` runs even when backgrounded) and content-free — the app wakes and syncs. Relay config: `FCM_SA_PATH` (service-account JSON), `FCM_PROJECT_ID` (optional; defaults to the JSON's `project_id`). If unset, FCM wakeups are logged only.

## UnifiedPush (degoogled Android)

[UnifiedPush](https://unifiedpush.org) lets users choose their own push distributor (e.g. ntfy, NextPush). The app registers with the distributor and gets an **endpoint URL**, which it registers with the relay exactly like a token — platform `unifiedpush`, the URL in the `device_token` field. When a wakeup is needed the **relay** POSTs a content-free body to that URL (not the homeserver — this keeps the homeserver out of the token business and the privacy model uniform). The distributor forwards it to the app's `PushService`, which wakes and syncs.

Because the endpoint URL is client-supplied and relay registration is unauthenticated, the relay's POST is **SSRF-guarded**: https only, and the resolved host must be a global address (loopback, private, link-local incl. the cloud-metadata `169.254.169.254`, CGNAT, and ULA ranges are rejected), with redirects disabled and a short timeout. Web Push (RFC 8291) payload encryption is not used — the wakeup carries no content, and the endpoint URL is the secret.

**Transport selection (client):** on login and app foreground the app picks one transport — Google Play Services present (incl. microG) → FCM; else a UnifiedPush distributor installed → UnifiedPush; else no push (foreground WebSocket only). Switching is automatic: installing a distributor on a degoogled phone is the only user action needed. Connector 3.x (`PushService` + `UnifiedPush.register`) is used; the client re-registers each foreground so a removed/changed distributor is picked up. A distributor **picker** for the multi-distributor case, and a **persistent foreground-service WebSocket keepalive** for the no-distributor case, are deferred follow-ups (`docs/02`).
