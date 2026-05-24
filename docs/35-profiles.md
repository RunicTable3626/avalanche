# Profiles

Display names, avatars, and bios — how they're stored, encrypted, distributed, and refreshed.

## Design principles

1. **Copy Signal's approach.** Signal's profile system is well-tested and handles the same trust model we need: the server stores encrypted blobs it can't read; contacts decrypt with a key distributed via messages.

2. **The server never sees plaintext profile data.** A seized server yields encrypted blobs that are useless without per-user profile keys. This is a meaningful protection for activists — it's the difference between seizing a list of DIDs and seizing a membership roll with real names.

3. **Two profile layers.** The substrate profile (name, avatar, bio) is encrypted and distributed via profile keys. Project profiles (attendee directory fields, team roles, etc.) are separate — collected by the Project, scoped to the Project, explicitly consented to. These are different systems serving different purposes.

## Substrate profile

### Profile contents

For stage 4, the profile contains only:

- `display_name` (required, set at account creation)

Future additions (same mechanism, richer blob):

- `avatar` (URL to an encrypted attachment + decryption key)
- `bio` (short text)
- `bio_emoji` (single emoji)

### Profile key

A 32-byte random symmetric key generated at account creation. Stored in the local SQLCipher database alongside the account's identity keys. The profile key is the single secret that controls who can read your profile.

The profile key does NOT change when you update your profile. It only rotates when you want to revoke access (e.g., after blocking someone), which forces re-distribution to all remaining contacts. Profile key rotation is out of scope for stage 4.

### Encrypted profile blob

The profile is serialized as JSON, encrypted with the profile key using AES-256-GCM, and uploaded to the homeserver. The server stores the blob as opaque bytes keyed by account ID.

```json
{"display_name": "Alice"}
```

Future fields (`avatar_url`, `bio`) are added to the same JSON object. No schema version needed — unknown fields are ignored by older clients.

When the user changes their display name:
1. Client re-encrypts the profile with the same profile key
2. Client uploads the new blob, replacing the old one
3. Nothing is pushed to contacts — they discover the change on their own

### Server endpoints

```
PUT  /v1/profile        — upload encrypted profile blob (authenticated, uses your own account)
GET  /v1/profile/{did}  — fetch encrypted profile blob for any user (authenticated)
```

The `PUT` endpoint replaces the stored blob. The `GET` endpoint returns opaque bytes. The server never interprets the contents.

**Both endpoints require authentication.** The `GET` endpoint is authenticated to prevent unauthenticated membership confirmation — an unauthenticated adversary probing `GET /v1/profile/{did}` could determine whether a DID is registered on the server. For small activist org servers, this effectively leaks org membership. The encrypted blob provides content confidentiality (only contacts with the profile key can decrypt), but the existence of a blob at all confirms membership. The `GET` endpoint returns 404 identically whether the DID doesn't exist or exists but has no profile, so authenticated users cannot distinguish the two cases either.

This means the invite flow cannot use an unauthenticated profile fetch. Instead, the invite token carries the inviter's display name directly (see "Invite token changes" below), and the new user fetches the encrypted profile after registration.

See `docs/00-design.md` (Threat model, "membership confirmation") for the broader design principle.

The registration endpoint (`POST /v1/accounts`) accepts an optional `encrypted_profile` field so the blob can be uploaded atomically with account creation.

### Server schema

```sql
CREATE TABLE profiles (
    account_id     BIGINT PRIMARY KEY REFERENCES accounts(id),
    encrypted_blob BYTEA NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Separate table from `accounts` because the access patterns are independent — auth/registration never reads profile blobs, and profile fetches never read auth fields. Keeps the hot `accounts` table lean, especially as profile blobs grow with avatars.

## Profile key distribution

The profile key is distributed to contacts inside encrypted messages. This is how recipients learn the key they need to fetch and decrypt your profile. It is NOT a change notification — it is purely key distribution.

### When the profile key is included in outgoing messages

Following Signal's model, the profile key is included in outgoing `ContentMessage` envelopes **only when the recipient is someone you've chosen to share your profile with.** For stage 4, this is effectively everyone you DM — there's no concept of contacts you're hiding your profile from yet.

Specifically, the profile key is attached to:
- Regular text messages
- Receipt messages (delivery, read)
- Any other `ContentMessage` variant

### Invite tokens

The inviter's profile key is embedded in the invite token payload. When the new user registers and the auto-DM is created, they can immediately fetch and decrypt the inviter's profile. This is the bootstrap path — after that, profile keys travel with regular messages.

## Profile fetching and caching

### When profiles are fetched

Recipients do NOT fetch the profile on every incoming message. Instead:

1. **On receiving a new profile key** — when a message contains a profile key that differs from the cached one (or no key was cached), the client fetches and decrypts the profile. If the key matches what's already cached, no fetch occurs.

2. **On opening a conversation** — the primary change detection mechanism. When the user navigates to a conversation, the client re-fetches the other participant's encrypted profile blob and decrypts it with the cached profile key. If the decrypted name differs from the cached name, the UI updates. This is how Signal detects profile changes and it's the most common path.

3. **Rendering an unknown name** — if the UI needs to display a name for a contact with no cached profile, a fetch is triggered (rate-limited).

4. **Periodic background refresh** (future, not stage 4) — a daily job that re-fetches stale profiles, similar to Signal's `StaleProfileFetcher`.

### Caching

Decrypted profiles are cached in the local SQLCipher database:

```sql
CREATE TABLE contact_profiles (
    did          TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    profile_key  BLOB NOT NULL,
    fetched_at   INTEGER NOT NULL  -- unix timestamp
);
```

The cache is keyed by DID. When a fetch returns a profile that differs from the cache, the cache is updated and the UI is notified.

### Rate limiting and deduplication

- Opportunistic fetches (conversation open) are skipped if a successful fetch occurred in the last 5 minutes for that contact.
- Fetches for unknown contacts are rate-limited to once per 30 minutes per DID.
- Multiple fetch requests for the same DID are deduplicated (only one in-flight request per DID).

## Protobuf changes

```protobuf
message ContentMessage {
  oneof body {
    TextMessage    text       = 1;
    MediaMessage   media      = 2;
    ReceiptMessage receipt    = 3;
    TypingMessage  typing     = 4;
    ExpiryUpdate   expiry     = 5;
  }

  uint64 timestamp_ms  = 15;
  uint32 expiry_timer  = 16;
  bytes  profile_key   = 17;  // sender's profile key (when sharing)
}
```

The `profile_key` field is on the outer `ContentMessage`, not inside any body variant, because it can accompany any message type.

## Invite token changes

The invite token payload currently includes:

- Server URL
- Token secret
- Inviter DID (optional)

Add:

- Inviter profile key (optional, present when inviter DID is present)
- Inviter display name (plaintext, as a fallback for display before the profile blob can be fetched)

The plaintext display name in the token is a UX convenience — it lets the app show "Alice invited you" on the invite acceptance screen before any server communication. The encrypted profile blob is the source of truth once fetched.

## Display in the app

### Conversation list

Each conversation row shows the contact's cached display name. If no name is cached (profile key not yet received, or fetch hasn't completed), show "Unknown" or the DID truncated as a placeholder.

### Message bubbles

Incoming messages show the sender's cached display name. In 1:1 DMs (stage 4), this is redundant with the conversation header but consistent.

### Settings

A "Your Profile" screen where the user can:
- See their current display name
- Edit it (re-encrypts and re-uploads the blob)
- (Future) set avatar and bio

## How this extends to Projects

When Projects arrive (stage 6+), the substrate profile mechanism doesn't change. Projects interact with it through scoped permissions:

1. A Project (e.g., Attendee Directory) asks users to share their profile with the Project during onboarding.
2. The user consents. Their profile key is shared with the Project's bot via the normal encrypted channel (the bot is a group member).
3. The bot fetches and decrypts the user's profile blob, caching the result in Project-scoped storage.
4. The Project displays the cached name in its directory UI.

Alternatively, a Project can collect its own fields ("Organization," "Role," "Dietary restrictions") that don't exist in the substrate profile. These are Project-owned data, stored in the Project's own tables, visible only to that Project's members.

The key architectural point: substrate profiles and Project profiles are separate systems. The substrate profile is your identity to your contacts (encrypted, key-gated). A Project profile is what you've explicitly chosen to share with a specific Project. No migration is needed between the two — they coexist.

## Stage 4 implementation scope

What to build now:
- Profile key generation at account creation
- Encrypted profile blob (display name only) upload at registration
- `PUT /v1/profile` and `GET /v1/profile/{did}` server endpoints
- Profile key included in outgoing messages
- Profile key in invite tokens
- Fetch + decrypt on new profile key received
- Re-fetch on conversation open
- Local cache in SQLCipher
- Edit display name in iOS settings
- Show cached names in conversation list and message bubbles

What to defer:
- Avatar and bio fields
- Profile key rotation
- Stale profile background job
- Versioned profile fetches (Signal's credential-authenticated fetch)
- Unidentified access / sealed sender for profile fetches
