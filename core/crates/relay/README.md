# actnet push relay

Standalone HTTP service that mediates between homeservers and APNs / FCM /
UnifiedPush distributors. Homeservers never see device push tokens — they POST
wakeup requests addressed to opaque per-(user, server) pseudonyms. The relay
maps pseudonyms to device tokens and fires content-free silent pushes,
dispatching by the `platform` stored at registration:

- `apns` — APNs token-based push (`a2`).
- `fcm` — FCM HTTP v1 (service-account JWT → OAuth2; data-only, high priority).
- `unifiedpush` — the `device_token` is the distributor endpoint URL; the relay
  POSTs a content-free body to it (SSRF-guarded: https only, global hosts only).

## Endpoints

Client-facing (called by `app-core` when a device registers / rotates):

- `POST /v1/register`   `{ pseudonym, device_token, platform, environment }`
  - `platform` is `"apns"`, `"fcm"`, or `"unifiedpush"`. For `unifiedpush`,
    `device_token` carries the distributor endpoint URL.
  - `environment` is `"sandbox"` (debug iOS builds) or `"production"`
    (TestFlight / App Store). Used only for APNs routing; ignored for
    `fcm`/`unifiedpush`.
- `POST /v1/unregister` `{ pseudonym }` — marks rotated, kept 7d

Homeserver-facing:

- `POST /v1/wakeup` `{ pseudonyms: [..] }` — sends silent push to each

## Rate limits

Per source IP (extracted from `X-Forwarded-For` when behind Caddy, peer IP
otherwise). Excess requests get HTTP 429.

| Endpoints | Sustained | Burst |
|---|---|---|
| `/v1/register`, `/v1/unregister` | 10/min | 5 |
| `/v1/wakeup` | 60/min | 30 |

Request bodies are capped at 4 KB by `DefaultBodyLimit`.

## Running locally

```bash
# Logged-only mode (no APNs send, useful for end-to-end plumbing tests):
make relay

# Real APNs mode (serves both sandbox + production at once — clients pick
# which endpoint to use by passing `environment` at registration):
APNS_KEY_PATH=./AuthKey_XXXXXXXXXX.p8 \
APNS_KEY_ID=XXXXXXXXXX \
APNS_TEAM_ID=YYYYYYYYYY \
APNS_BUNDLE_ID=net.theavalanche.app \
make relay
```

If `APNS_KEY_PATH` is unset the relay still runs and logs the wakeup
intent, but does not contact Apple — convenient for testing the
server→relay→pseudonym-lookup chain without a `.p8` to hand.

## Env vars

| Var | Default | Purpose |
|---|---|---|
| `RELAY_BIND_ADDR` | `0.0.0.0:3002` | HTTP bind address |
| `DATA_DIR` | `.` | Directory holding `relay.db` |
| `APNS_KEY_PATH` | _(unset)_ | Path to `.p8` auth key. If unset, APNs is disabled. |
| `APNS_KEY_ID` | _(required if key set)_ | 10-char key ID |
| `APNS_TEAM_ID` | _(required if key set)_ | 10-char team ID |
| `APNS_BUNDLE_ID` | _(required if key set)_ | App bundle ID |
| `FCM_SA_PATH` | _(unset)_ | Path to FCM service-account JSON. If unset, FCM is disabled. |
| `FCM_PROJECT_ID` | _(JSON `project_id`)_ | Override the FCM project; defaults to the service account's own. |

UnifiedPush needs no config — those wakeups are plain outbound HTTPS POSTs.

A single relay instance handles both sandbox and production tokens. The
client passes `environment` ("sandbox" or "production") at registration
based on its build flavor (`#if DEBUG`); the relay stores it and routes
each wakeup to the matching APNs endpoint. Sending a sandbox token to
the production endpoint (or vice versa) returns `BadDeviceToken`, which
is why the split matters.

## Smoke-testing APNs auth without the relay

```bash
APNS_KEY_PATH=... APNS_KEY_ID=... APNS_TEAM_ID=... APNS_BUNDLE_ID=... \
APNS_ENVIRONMENT=sandbox \
cargo run -p relay --example send_test_push -- <device_token_hex>
```

(`APNS_ENVIRONMENT` here is read by the standalone example only — the
relay itself ignores it and uses the per-registration value instead.)

A `code: 200` response means the key, bundle ID, entitlement, and
provisioning profile are all correctly aligned.
