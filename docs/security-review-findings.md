# Design Review Findings

Compiled findings from a full read of `docs/` (as of upstream `6ca095a`).
Each item has a severity tier, a citation, and a description of the downstream
risk if left unaddressed. Items within a tier are ordered roughly by urgency.

**Tiers:**

- **Tier 1 — Potentially launch-blocking.** Either a security hole, a GDPR gap,
  or a prerequisite gate that doesn't exist yet.
- **Tier 2 — Should be addressed before beta users.** Real risk, but workable
  without blocking a limited/testflight release.
- **Tier 3 — Design decisions worth explicit acknowledgment.** Known tradeoffs
  that should be written down in the threat model so future contributors don't
  relitigate them or silently undo them.

---

## Tier 1 — Potentially Launch-Blocking

### 1. Account deletion is architecturally blocked

**Citation:** `docs/03-groups.md:492` (§3.9 schema discipline, open question on
account deletion); `docs/06-identity-device-store-split.md:§12` (event log home
deferred).

The server deliberately maintains no `(DID → groups_joined)` table (§3.9 rule
1), which is load-bearing for seizure resistance. But this means the server
cannot enumerate a user's groups on deletion, so "remove this DID from all
groups" is not implementable server-side. The doc notes this in §3.9:

> *"Operational tooling that requires DID↔group lookup (e.g. 'remove this DID
> from all groups on the server') is not available server-side under this
> discipline. Such operations must be driven by clients holding the relevant
> group keys."*

Client-driven deletion (client iterates its group keys and submits
`remove_members` actions) fails for the lost-device case — the most likely
scenario where a user also wants to delete their account.

Additionally, `docs/06` defers the event log's store home (message history,
reactions, revisions, read marks), which means there's no answer yet for "what
gets deleted from message history when an account is deleted." This has both
GDPR right-to-erasure implications and practical UX implications.

No proposed solution exists for either gap.

---

### 2. `POST /v1/devices/replace` is a single-slot swap, not a full-identity revocation

**Citation:** `docs/04-multi-device.md:§7` ("Implementation gap").

> *"The current `POST /v1/devices/replace` (`devices.rs`) is only a single-slot
> swap — it names one `old_device_id`, deletes that one row (cascading its
> tokens/prekeys/messages), and creates the new device, leaving all other device
> rows untouched. That is correct for today's single-device case but is not a
> whole-identity reset."*

For a multi-device user doing total-loss recovery, all device rows except the
named one stay alive. Those rows' session tokens remain valid; those devices
continue to drain queued messages until the tokens expire naturally. A
lost/stolen device that is not the "primary" (if the user was already
multi-device) receives no revocation at all.

---

### 3. Multi-homing recovery fanout is unspecified

**Citation:** `docs/04-multi-device.md:§7` ("OPEN — multi-homing").

> *"Total-loss recovery must therefore revoke across all of the identity's
> accounts, not just one server's — the rotation key authorizes this on every
> server, but the flow that fans the revocation out across the identity's
> `servers` is unspecified."*

An identity registered on servers A, B, and C that recovers via server A leaves
servers B and C with live, unrevoked device rows and valid session tokens. This
is marked as OPEN with no proposed design.

---

### 4. Project token scoping is not enforced server-side

**Citation:** `docs/20-project-security.md` ("Items deferred"):

> *"Token scoping enforcement on verify endpoint (v2)."*

The verify endpoint (`GET /v1/project-token/verify`) returns `{did,
project_url}` and it is the Project's responsibility to check that the
`project_url` in the response matches its own origin. There is no server-side
rejection of a cross-Project token use. A Project that omits or misimplements
this check allows any token issued for any Project on the homeserver to
authenticate against it — granting that Project's access to the user's DID and
any capabilities the second Project holds (group membership, DM access via bots,
etc.). This is a two-line server-side fix; keeping it deferred makes it a
footgun for every third-party Project developer.

---

### 5. Passkey relying party is bound to `theavalanche.net` — identity recovery is centralized at Avalanche's domain

**Citation:** `docs/50-identity-auth-recovery.md:88`:

> *"Passkey relying party: a universal actnet domain (e.g. `theavalanche.net`),
> not the homeserver's domain. This means recovery of a passkey identity can only
> be done by our official mobile apps and/or web application on our domain."*

The rotation key — the root authority over every user's DID — is deterministically
derived from a WebAuthn passkey bound to `theavalanche.net`. Consequences:

- If Avalanche's domain is seized, suspended, or the organization folds, no user
  can recover their identity. This contradicts the stated server-seizure threat
  model, which is built around ensuring users can reconstitute identity and
  connections on another server.
- Third-party homeserver operators cannot offer their own recovery flows for
  their users — all recovery paths chain through `theavalanche.net`.
- For users who store their passkey in iCloud Keychain (the default onboarding
  path), Apple holds the credential that produces the rotation key. U.S. law
  enforcement can compel Apple for iCloud data. The threat model does not
  surface this vector.

---

### 6. `make dev-all` co-locates adminbot with the homeserver, silently defeating its seizure protection

**Citation:** `docs/22-adminbot.md` ("Deployment shape"):

> *"Co-locating adminbot with the homeserver — the simple `make dev-all` shape —
> forfeits the property: seize the server, seize the bot."*

The adminbot's seizure resistance (headless, no inbound network surface, hard to
locate) only holds when adminbot runs on a different machine from the homeserver.
`make dev-all` is the primary documented workflow in `CLAUDE.md` and the
obvious path for operators following the getting-started guide. A self-hoster
who follows the simplest deployment path gets none of the seizure protection the
adminbot architecture was designed to provide — and they won't know it. The
deployment guide (`docs/40-deployment.md`) needs a prominent call-out, and the
adminbot doc's caveat should be visible at first encounter, not buried mid-section.

---

### 7. Vetted onboarding ("closed registration") is load-bearing but not yet implemented — the gate doesn't exist yet

**Citation:** `docs/24-vetted-onboarding-project.md` ("Trust and gating model"
and "Assumptions audit"):

> *"The homeserver currently has open registration (`51-invite-tokens.md:67`);
> closing it is the load-bearing server change this Project depends on. It must
> fail closed: if no gatekeeper vouches for the token, registration is rejected,
> never waved through."*

> *"Not verified against server code — design references the doc-level seam only."*

The new `registration.gatekeeper` capability does not yet exist in the server's
capability set. An operator who deploys the vetted onboarding Project before the
server-side enforcement is built will believe they have closed registration but
actually have open registration — anyone can create an account. Fail-closed
is marked load-bearing by the doc itself; the absence of enforcement is therefore
a security gap, not just an incomplete feature.

---

## Tier 2 — Should Be Addressed Before Beta

### 8. Silently linked attacker device does not trigger a safety number change

**Citation:** `docs/04-multi-device.md:§8` ("Trust on device-set change"):

> *"Because the identity key is shared, adding or removing a peer's device does
> not change the safety number... a silently-linked (e.g., coerced or
> attacker-controlled) device is automatically encrypted-to with no
> safety-number alarm."*

The suggested mitigation ("surface 'Bob added a new device' as an info event by
diffing device lists / registration IDs") is described as "optional improvement"
that "does not block linking." For an activist context where key exfiltration is
a primary threat — authorities may coerce someone to link a surveillance device
— this notification is not optional. Without it, every contact of a coerced user
silently encrypts to a device they never consented to trust.

---

### 9. Recovery "link vs. replace" confusion is a security footgun with no UX gate

**Citation:** `docs/04-multi-device.md:§7`:

> *"The hazard is doing the wrong one: 'recovering' (replacing the slot) while
> the old device is actually still alive produces two physical devices claiming
> the same `device_id` with different registration IDs — split-brain, each
> clobbering the other's reg-id server-side. Hence link and recover must be
> distinct, explicit user intents gated on 'is the old device gone?'"*

The current onboarding flow in `docs/50-identity-auth-recovery.md` ("Recovery
Explainer" and "Choose ID page") doesn't prominently gate on this question.
Both "recover" and "link" result in the same passkey + Face ID ceremony. A user
who temporarily loses a device but gets it back, or who panics and runs recovery
while a device is still alive, produces a split-brain state with no documented
recovery path.

---

### 10. No-blob recovery path: safety number change indistinguishable from key substitution attack

**Citation:** `docs/50-identity-auth-recovery.md:172` (no-blob recovery path):

> *"Result: Sam's DID is preserved, but contacts see a safety-number change and
> per-server state (group memberships, queued messages, the full server list) is
> lost."*

Safety number changes are the primary indicator of a potential MitM key
substitution in Signal-based systems. The no-blob recovery produces the same
visual signal. The docs do not describe how a contact is supposed to distinguish
"Sam legitimately recovered their device" from "Sam's account was seized and a
new key was substituted." For a high-risk activist context where key substitution
by an adversary is a real threat, the protocol should define an expected
out-of-band verification ceremony for this case.

---

### 11. No runtime user consent for Project scopes — users cannot decline individual data collection

**Citation:** `docs/20-project-security.md` ("Project permissions"):

> *"A runtime user-consent layer is deliberately not added for own-homeserver
> Projects. It would re-litigate a decision the trust chain hands to the admin,
> train users to reflexively tap 'Allow,' and — for identity specifically — be
> partly theatre, since the admin-run homeserver already knows the user's DID."*

The rationale is reasonable for low-sensitivity scopes. But for activists on a
shared homeserver: an admin who installs a Project with `dm:bypass-request`,
`invites:auto-accept`, or `identity:real-did` gives that Project persistent
access to those capabilities for every user on the server, with no per-user
opt-out short of leaving the server entirely. The doc acknowledges users can
"leave groups with bots they don't trust" — an exit option, not a consent
option. For the specifically named high-sensitivity scopes
(`dm:bypass-request`, `invites:auto-accept`), a per-user opt-out mechanism
deserves consideration even under the admin-trust model.

---

### 12. Magic-link social-graph beacon is documented but mitigation is advisory only

**Citation:** `docs/20-project-security.md` ("Handle identity and auth"):

> *"A unique-per-recipient context id in a shared link lets you learn the DID of
> everyone who taps it, tagged by who shared it — a who-clicked / social-graph
> probe. Use a coarse context where you can, and don't retain the correlation
> longer than the feature needs."*

The core threat model (`docs/00-design.md`) calls out "membership lists can be
used to target individuals" as a primary concern. A magic link with a
per-share context ID is exactly a who-clicked membership probe. The mitigation
is non-binding guidance to Project developers. For this user base, this should
be a platform-enforced constraint (e.g., no per-recipient context IDs in links
surfaced via the `identity:magic-links` scope, or a per-tap nonce the platform
controls rather than the Project).

---

### 13. Event log home is deferred — blocks account deletion and data retention design

**Citation:** `docs/06-identity-device-store-split.md:§12`:

> *"Event log (`message_history`, `reactions`, `message_revisions`, read/delivery
> marks) — fold into the IdentityStore, keep in the DeviceStore, or a third
> store? Deferred for now; to be settled in its own doc alongside the
> `docs/04` §5 event-channel design."*

This decision has material downstream consequences:

- **Account deletion:** no design for "delete all message history for this
  account" is possible until this is settled.
- **GDPR right to erasure:** if message history goes into the IdentityStore, it
  survives in encrypted snapshots on backup servers after the primary account is
  deleted, unless the backup policy explicitly handles it.
- **Newly linked device:** if history is DeviceStore (device-local), a freshly
  linked device starts blank — aligned with the §10 explicit non-goal. If it's
  IdentityStore, it backfills on link. The UX expectation differs significantly
  between these choices.

Deferring this also defers the account deletion design (Tier 1 #1).

---

### 14. Client-side snapshot build/restore not yet implemented — total-loss recovery gap

**Citation:** `docs/05-device-data-sync.md:§13`:

> *"Not yet built (client): the periodic backup-push task and the
> `build_snapshot`/`restore_snapshot` core that serialize/hydrate the store.
> Until those land, recovery still relies on the authoritative account's live
> items only, even though the server can now hold a snapshot."*

The server-side snapshot endpoints are built (`014_storage_snapshots.sql`,
`GET`/`PUT /v1/storage/snapshot`). The client side is not. Until it is, the
total-loss recovery path (`docs/05:§11`) cannot use a backup server's snapshot —
it requires the authoritative account to be alive and reachable. A user whose
primary server is seized or offline during recovery cannot recover their durable
identity state (contacts, group keys, settings) even if they registered on
backup servers.

---

## Tier 3 — Design Decisions Worth Explicit Acknowledgment

### 15. PLC (Bluesky's directory) is load-bearing with no fallback plan

**Citation:** `docs/13-federation.md:141`:

> *"PLC is a centralization point. Bluesky's PLC is operated by Bluesky. We'll
> assume it keeps working for now; we could support other PLC types but that's
> for future work."*

PLC is involved in: DID genesis (signup), identity verification on every server
join, migration, and the no-blob recovery path (which submits a PLC update). If
PLC is unavailable, new accounts cannot be created, migrations fail, and the
no-blob recovery path breaks. The threat model is built around resilience against
seizure and single-operator failure — the entire identity layer depending on a
single externally-operated directory contradicts this. Worth a documented
contingency plan even if the plan is "use a mirrored PLC instance at
`plc.theavalanche.net` as fallback."

---

### 16. P-256 rotation key diverges from the rest of the cryptographic stack without noted rationale

**Citation:** `docs/50-identity-auth-recovery.md:18` (rotation key is P-256);
`docs/01-technical-implementation.md:138-146` (rest of stack is Curve25519/Ed25519).

The DID rotation key is P-256 because WebAuthn PRF requires it. Every other key
in the system uses Curve25519 (X25519 for key agreement) or Ed25519 (signatures).
P-256's curve parameters have contested provenance. The philosophy in `01` is
"use audited implementations, don't implement crypto yourself" — P-256 is
audited, but this divergence is not acknowledged anywhere in the docs. A
security-conscious reviewer reading the threat model will notice the mismatch and
not find an explanation for it.

---

### 17. iCloud Keychain as default passkey store means Apple can produce the rotation key under legal compulsion

**Citation:** `docs/50-identity-auth-recovery.md:76` (passkey creation flow uses
iCloud Keychain/1Password as the primary path).

The PRF output from the passkey deterministically produces the DID rotation key
— the root authority over the user's identity. For users who store their passkey
in iCloud Keychain (the default happy path shown in the onboarding flow), Apple
holds the credential. U.S. law enforcement can compel Apple for iCloud data via
a National Security Letter or ECPA order. The threat model explicitly considers
law enforcement seizure of homeservers but does not address this vector. The
onboarding flow does not distinguish between iCloud Keychain (Apple-controlled)
and hardware security keys or a physical recovery phrase (user-controlled) in
terms of their respective security properties for high-risk users.

---

### 18. Federation trust bootstrapping is a chicken-and-egg problem for new operators

**Citation:** `docs/12-abuse-handling.md:244` (§5 Federation trust):

> *"Brand-new peer with no history: Unknown. Reachable only via
> attestation-token-bearing first-contact requests."*

New users on a brand-new homeserver start with "default" attestation quality.
A brand-new homeserver has no attestation history. Two organizations with no
prior contact cannot federate without a user on one side first generating an
attestation-token-bearing contact link and sharing it out-of-band. For the
scenario where a seized homeserver's users migrate to a new server and try to
re-contact peers on other servers, the bootstrap path needs to be more explicitly
designed. The current doc acknowledges this case ("Activist operator coming
online after a seizure") but only notes that "organic contact-adds on day one"
will eventually work — without specifying how long that takes or what the
experience is for users who can't reach any trusted peer immediately.

---

### 19. `docs/CLAUDE.md` index is out of sync with actual file paths

**Citation:** `docs/CLAUDE.md` (lookup table).

The lookup table maps doc numbers to descriptions that don't match the actual
files on disk. For example, `32` → *"Mesh / BitChat fallback over BLE"* but the
actual file is `docs/32-threading.md`; `33` → *"Identity, passkeys, recovery
blob, device loss"* but the actual file is `docs/33-reactions.md`. The index was
written against an earlier numbering scheme. A contributor following this index
will consistently open the wrong file. The `docs/DIGEST.md` newly added in this
upstream batch partially mitigates this with an accurate summary, but the
`CLAUDE.md` index itself should be corrected or removed.

---

## Summary Table

| # | Issue | Tier | Citation |
|---|-------|------|----------|
| 1 | Account deletion architecturally blocked | **1** | `03-groups.md:492`; `06:§12` |
| 2 | `devices/replace` is single-slot, not full revocation | **1** | `04-multi-device.md:§7` |
| 3 | Multi-homing recovery fanout unspecified | **1** | `04-multi-device.md:§7` |
| 4 | Project token scoping not enforced server-side | **1** | `20-project-security.md` |
| 5 | Passkey RP bound to `theavalanche.net` | **1** | `50-identity-auth-recovery.md:88` |
| 6 | `make dev-all` co-locates adminbot, defeats seizure property | **1** | `22-adminbot.md` |
| 7 | Closed registration not implemented; gatekeeper gate missing | **1** | `24-vetted-onboarding-project.md` |
| 8 | Coerced device link doesn't change safety number | **2** | `04-multi-device.md:§8` |
| 9 | Link vs. recover confusion produces split-brain | **2** | `04-multi-device.md:§7` |
| 10 | No-blob recovery safety number change ≡ MitM signal | **2** | `50-identity-auth-recovery.md:172` |
| 11 | No runtime user consent for Project scopes | **2** | `20-project-security.md` |
| 12 | Magic-link who-clicked beacon: advisory mitigation only | **2** | `20-project-security.md` |
| 13 | Event log home deferred; blocks deletion + retention design | **2** | `06-identity-device-store-split.md:§12` |
| 14 | Client snapshot build/restore not built; total-loss recovery gap | **2** | `05-device-data-sync.md:§13` |
| 15 | PLC centralization; no fallback plan documented | **3** | `13-federation.md:141` |
| 16 | P-256 rotation key undocumented divergence from Ed25519 stack | **3** | `50:18`; `01:138-146` |
| 17 | iCloud Keychain default = Apple holds rotation key | **3** | `50-identity-auth-recovery.md:76` |
| 18 | Federation trust bootstrap chicken-and-egg for new operators | **3** | `12-abuse-handling.md:244` |
| 19 | `docs/CLAUDE.md` lookup table is stale / wrong | **3** | `docs/CLAUDE.md` |
