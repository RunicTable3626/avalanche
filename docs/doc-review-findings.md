# Documentation Review Findings

Compiled from a full read of all docs as of branch `docs/android-desktop-implementation`.
Items are grouped by concern type, then ranked within each group by priority.

---

## Group 1 — Documentation Integrity

These issues make the docs contradict each other or cite things that don't exist,
actively misleading new contributors.

### 1.1 Security review cites documents that don't exist in this repo

`docs/security-review-findings.md` cites 10 documents absent from this repo:
`docs/03-groups.md`, `docs/04-multi-device.md`, `docs/05-device-data-sync.md`,
`docs/06-identity-device-store-split.md`, `docs/12-abuse-handling.md`,
`docs/13-federation.md`, `docs/22-adminbot.md`, `docs/24-vetted-onboarding-project.md`,
`docs/50-identity-auth-recovery.md`, and `docs/CLAUDE.md`. The review appears to be
from a more advanced internal/upstream version of the codebase. Features discussed
(adminbot, multi-device, passkey recovery, multi-homing) appear nowhere else in the
current docs.

**Risk:** A contributor reading this review cannot verify any cited claim, and will
incorrectly believe the current system has a passkey-based recovery architecture, an
adminbot, multi-device support, and other features that haven't been built yet.

**Fix:** Add a preamble to the file stating which upstream commit it was authored
against, that several referenced docs exist only upstream, and that the findings
apply to a future state of the system, not the current branch.

---

### 1.2 ONBOARDING.md stage table contradicts the deferred todos list

`ONBOARDING.md` marks stages 1–4 as "✅ Complete." But `docs/02-todos-deferred.md`
lists the following as unchecked `[ ]` or explicitly deferred:
- Push notifications (entire multi-section checklist)
- Recovery key UI ("banner currently shows, hardcoded false")
- Scroll-position-based read marking
- Database encryption key from Secure Enclave (currently hardcoded)

**Risk:** New contributors believe the app is production-ready and skip critical
security features. The dog-fooding recommendation in `docs/01` compounds this.

**Fix:** Either update the stage table to reflect actual completion state, or add
a "known gaps" callout in ONBOARDING.md that lists what Stage 4 still requires.

---

### 1.3 Stage numbering diverges between ONBOARDING.md and docs/01

`ONBOARDING.md` defines four stages: 1=crypto core, 2=homeserver, 3=mobile+identity,
4=ship it. `docs/01-technical-implementation.md` defines ten stages: 1=crypto core,
2=homeserver, 3=mobile DMs, 4=action-bound groups, 5=push notifications, 6=Projects,
etc. A contributor reading both gets a fundamentally different picture of where the
project stands.

**Risk:** "Stage 5" means something different depending on which document you're
reading. This causes confusion on PRs, planning, and the deferred todos list.

**Fix:** Reconcile the two stage schemas. The ten-stage plan in `docs/01` is more
detailed and should be authoritative; ONBOARDING.md should reference it rather than
defining its own numbering.

---

### 1.4 README documentation map is stale

`README.md` links to 7 docs. The repo contains 13+ docs including
`android-implementation.md`, `desktop-implementation.md`, `security-review-findings.md`,
`31-read-tracking.md`, `34-push-relay-bcd-impl.md`, and others.

**Risk:** New contributors won't discover the additional docs unless they browse
`docs/` manually.

**Fix:** Update README to link all docs, or point to the documentation map in
`docs/00-design.md` as the canonical index.

---

### 1.5 Doc numbering scheme is inconsistently applied

`docs/00-design.md` defines a numbered naming scheme (0x = design, 1x = server,
2x = projects, 3x = mobile). But several docs don't follow it:
`security-review-findings.md`, `android-implementation.md`, and
`desktop-implementation.md` have no numeric prefix. `docs/31-read-tracking.md`
and `docs/34-push-relay-bcd-impl.md` exist but aren't mentioned in the
documentation map table.

**Fix:** Either number all docs consistently and add them to the map, or drop the
numbering convention and use plain names.

---

## Group 2 — Security Gaps

Issues where the documented design has a known security hole, a missing gate, or a
feature advertised as complete that isn't.

### 2.1 Hardcoded database encryption key with no gate before shipping to real users

`docs/11-core-api-sketch.md` and `mobile/CLAUDE.md` acknowledge the SQLCipher key
is `"dev-placeholder-key"` pending Secure Enclave integration. `docs/01` says
Secure Enclave integration is Stage 3 work. ONBOARDING.md says Stage 3 is complete
and the team dog-foots internally. If the app is used for actual communications,
all local message databases are encrypted with a known constant key — effectively
unencrypted at rest.

**Risk:** Any seized device yields all local message history regardless of E2E
encryption. This contradicts the stated threat model.

**Fix:** Add an explicit gate: "Do not onboard real users until Secure Enclave
integration lands." Track this as a launch blocker, not a deferred item.

---

### 2.2 Project token scoping not server-side enforced, not tracked in todos

`docs/20-project-security.md` defers server-side enforcement of project URL scoping
("Items deferred: Token scoping enforcement on verify endpoint (v2)").
`docs/security-review-findings.md` calls this Tier 1 (launch-blocking).
`docs/02-todos-deferred.md` does not include it.

The current design: the verify endpoint returns `{did, project_url}` and it is
the Project's responsibility to check that `project_url` matches its own origin.
A third-party developer who omits that check allows any token issued for any
Project on the homeserver to authenticate against it — cross-Project privilege
escalation with access to the user's DID and whatever that Project's bots can see.

**Risk:** Every third-party Project developer is a footgun away from a significant
auth bypass.

**Fix:** Add server-side enforcement (reject verify requests where the caller's
origin doesn't match `project_url` stored in the token). Track this in
`docs/02-todos-deferred.md` as a blocker for the Project framework launch.

---

### 2.3 Stub `did:plc` DIDs are not valid — Bluesky compatibility claim is false

`docs/10-server-implementation.md` describes generating DIDs by hashing the identity
key + server URL + timestamp, prefixing with `did:plc:`, and storing them locally
without any PLC directory interaction until Stage 9. These DIDs will fail resolution
by any standard `did:plc` resolver — they are locally-generated stubs.

ONBOARDING.md and `docs/00` both advertise Bluesky DID compatibility as a key
differentiator. A user whose DID is a local stub cannot migrate their identity,
cannot be resolved by external systems, and has no actual Bluesky compatibility.

**Risk:** Users joining during the current phase may share their DID outside the
app and find it doesn't work. If DIDs are ever published (e.g., a profile link),
the claim of Bluesky compatibility is false.

**Fix:** Prominently document that current DIDs are server-local stubs and
Bluesky compatibility is not available until Stage 9. Remove or qualify the
Bluesky compatibility claim from ONBOARDING.md until it's true.

---

### 2.4 Session token appears in WebSocket URL — logged by servers

`docs/10` defines the WebSocket connection as `GET /v1/ws?token=<session_token>`.
Session tokens in URL query strings appear in server access logs, proxy logs, TLS
terminator logs, and OS network history. The standard for sensitive bearer tokens
is HTTP headers (`Authorization: Bearer`).

**Risk:** A seized server's access logs contain valid session tokens for all
connected clients (until token expiry). This contradicts the seizure resistance goal.

**Fix:** Pass the session token in the `Sec-WebSocket-Protocol` header or as an
initial WebSocket frame payload, not in the URL.

---

### 2.5 Recovery key design docs are absent from this repo

`docs/30-mobile-ux.md` and ONBOARDING.md both mention a recovery key setup flow.
`docs/02` has a deferred todo for the recovery key UI. But there is no design
document in this repo for: what the recovery key is, how it's derived, what the
recovery flow looks like, or what the security properties are. The
`docs/security-review-findings.md` discusses a passkey-based recovery architecture
in detail (items 5, 10, 15, 16, 17) but cites `docs/50-identity-auth-recovery.md`
which doesn't exist here.

**Risk:** The recovery key is a foundational security feature. Building it without
a design doc leads to ad-hoc implementation decisions in security-critical code.

**Fix:** Write a recovery key design document before implementing the recovery UI.
The security review findings can serve as a starting point for concerns to address.

---

## Group 3 — Architectural Gaps

Issues where the design is underspecified or has a known scaling / correctness
problem that isn't acknowledged.

### 3.1 Multi-instance deployment breaks real-time delivery silently

`docs/10-server-implementation.md` describes the WebSocket connection map as an
in-memory `Arc<RwLock<HashMap<DevicePk, Sender>>>`. In a load-balanced multi-instance
deployment, if Alice's WebSocket is on instance A but a message is sent via instance
B, instance B finds no connection in its local map and silently falls back to
poll-on-reconnect. `docs/01` states the server "is designed to be horizontally
scalable" without acknowledging that this breaks real-time delivery.

Rate limiting has the same issue: in-process rate limit state isn't shared across
instances, so a user can exceed the limit by N× where N is the instance count.

**Risk:** Operators who follow the "horizontally scalable" guidance will get silently
degraded real-time messaging with no error surfaced.

**Fix:** Document that real-time delivery requires sticky sessions (or a pub/sub
backend like Redis Pub/Sub or PostgreSQL LISTEN/NOTIFY) and that the no-Redis stance
applies to single-instance deployments only.

---

### 3.2 Fan-out for large action-bound groups is unaddressed

`docs/01` specifies that each message is encrypted once per recipient device. For a
50-member action-bound group where each member has 2 devices, one group message
requires 100 separate encrypt operations client-side and 100 `message_queue` rows
server-side. `docs/01` explicitly reserves Sender Keys (the efficient "encrypt once,
fan out" scheme) for cross-server casual groups only. Action-bound groups use zkgroup,
which doesn't have a Sender Key equivalent described anywhere.

**Risk:** Large action-bound groups — the primary use case for activist organizing —
will be unusably slow for clients and generate significant server storage at scale.

**Fix:** Design a Sender Key equivalent for action-bound groups, or explicitly
document the practical size limit and performance characteristics.

---

### 3.3 Cross-server communication is unavailable until Stage 9

Users on different homeservers cannot DM each other, cannot form groups, and cannot
see each other until federation lands in Stage 9 — the final stage. This is
architecturally correct but not well-emphasized in the product-facing docs.
ONBOARDING.md doesn't mention it. `docs/00` discusses federation as if it's a
present capability.

**Risk:** Organizations deploying separate homeservers during the build-out period
will discover their users are siloed with no warning.

**Fix:** Add a prominent callout in ONBOARDING.md and `docs/00`: "Cross-server
communication requires federation (Stage 9). Until then, users on different
servers cannot interact."

---

### 3.4 Orphaned bot accounts accumulate with no cleanup

`docs/21-chatbot-project.md` notes that when the chatbot service restarts, orphaned
bot accounts remain on the homeserver and are "harmless." Each bot creates a `devices`
row, prekey rows, and potentially `push_pseudonyms` rows. With repeated service
restarts and user interaction, these accumulate unboundedly. The background cleanup
tasks only handle expired messages and session tokens, not orphaned accounts.

**Risk:** At scale or after months of operation this is a data hygiene problem and
a storage cost that grows without bound.

**Fix:** Either add a bot account cleanup task (delete device rows for bots with no
active session and an in-progress flag) or document the trade-off explicitly and
add it to the deferred todos.

---

### 3.5 Chatbot in-memory state inadequately warned as dev-only

`docs/21` says bot conversation history lives in-memory and is lost on restart, and
this is "fine for a dev tool." But ONBOARDING.md says the team dog-foots internally.
If real users interact with the chatbot and the service restarts, their conversation
history is silently gone and they're talking to a fresh bot with no prior context.

**Risk:** Internal users may use the chatbot expecting persistence; the doc doesn't
make the ephemeral nature sufficiently prominent.

**Fix:** Add a bold "Dev-only — not for production use" callout at the top of
`docs/21`. If this is to become a real feature, persistence needs to be designed.

---

## Group 4 — Platform Parity Debt

Issues related to the three-platform parity rule and its current enforcement.

### 4.1 Platform parity rule was violated for all existing iOS code

`CLAUDE.md` establishes: "Any feature added or changed on one platform must be
implemented on all three in the same session." But `docs/android-implementation.md`
and `docs/desktop-implementation.md` (added on this branch) show every parity item
as `[ ]`. iOS (stages 1–4) was built before Android or Desktop existed as targets.

**Risk:** The parity tracking tables imply the work is planned but don't quantify
the debt or provide a timeline. A contributor following the rule on new work won't
know they also need to backfill.

**Fix:** Add a section to each parity doc: "Backfill required — the following iOS
features predate the parity rule and need to be implemented on this platform before
it ships." Make the debt visible rather than implicit.

---

### 4.2 `node/` directory doesn't exist but is referenced as present

`node/CLAUDE.md` says "`node/` contains two things: napi-rs Rust bindings and the
Desktop Electron app." `docs/desktop-implementation.md` says: "`node/` is referenced
in CLAUDE.md but the directory doesn't exist yet." The CLAUDE.md at root also
references `node/CLAUDE.md` as if the directory structure is established.

**Risk:** Confusion about whether napi-rs bindings exist. The desktop FFI bridge is
a prerequisite for all Desktop work; if it doesn't exist, Phase 1 of the Desktop
plan is the actual first step, not a refinement.

**Fix:** Update `node/CLAUDE.md` and root `CLAUDE.md` to clearly state that
`node/` is a planned directory, and that `make desktop-bindings` does not yet exist.

---

## Group 5 — Design Decisions Lacking Explicit Documentation

Known trade-offs that should be written down so future contributors don't relitigate
them or silently undo them.

### 5.1 No-Redis stance needs a "single-instance only" qualifier

`docs/01` states "No Redis. The homeserver dependency is a single binary plus
PostgreSQL" as a simplification benefit. This works correctly for single-instance
deployments. For multi-instance deployments, in-memory rate limiting and WebSocket
delivery lose correctness without some form of shared state. The docs don't qualify
this.

**Fix:** Add a note: "The no-Redis constraint applies to the typical single-instance
deployment. Multi-instance deployments require a coordination mechanism for rate
limiting and real-time delivery."

---

### 5.2 The `make dev-all` co-location caveat should be surface-level

`docs/security-review-findings.md` (item 6) notes that running the adminbot
co-located with the homeserver defeats the adminbot's seizure protection. The design
relies on the adminbot running on a separate machine. The simplest documented
workflow (`make dev-all`) produces the insecure co-located configuration and an
operator following the getting-started guide won't know it. (The adminbot doesn't
exist yet in this repo, but when it does, this should be a prominent callout, not
buried in a mid-document note.)

---

### 5.3 No documented abuse handling strategy

`docs/01` mentions rate limiting as the primary defense against abuse. `docs/20`
mentions bots are rate-limited like human accounts. But there's no document covering:
what happens when a bot account is used to spam users, how the homeserver admin
suspends accounts, what signals trigger moderation, or how to handle a compromised
Project bot that exfiltrates message content.

**Fix:** Either add an abuse handling section to `docs/10` or create a new doc
covering admin tooling, account suspension, and incident response.

---

## Summary Table

| # | Issue | Group | Priority |
|---|-------|-------|----------|
| 1.1 | Security review cites nonexistent docs | Integrity | Critical |
| 2.1 | Hardcoded DB key, no gate before real users | Security | Critical |
| 2.2 | Project token scoping not enforced, not in todos | Security | Critical |
| 1.2 | Stage table contradicts deferred todos | Integrity | High |
| 1.3 | Stage numbering diverges between ONBOARDING and docs/01 | Integrity | High |
| 2.3 | Stub `did:plc` DIDs + false Bluesky compat claim | Security | High |
| 2.4 | Session token in WebSocket URL → access logs | Security | High |
| 2.5 | Recovery key design docs missing | Security | High |
| 3.1 | Multi-instance breaks real-time delivery silently | Architecture | High |
| 4.1 | Platform parity rule violated for all existing iOS code | Parity | High |
| 3.2 | Fan-out for large action-bound groups unaddressed | Architecture | Medium |
| 3.3 | Cross-server comms unavailable until Stage 9, not emphasized | Architecture | Medium |
| 3.4 | Orphaned bot accounts accumulate without cleanup | Architecture | Medium |
| 3.5 | Chatbot in-memory state inadequately warned as dev-only | Architecture | Medium |
| 4.2 | `node/` directory doesn't exist but referenced as present | Parity | Medium |
| 5.1 | No-Redis stance needs "single-instance only" qualifier | Design | Low |
| 5.2 | `make dev-all` co-location caveat not surface-level | Design | Low |
| 5.3 | No documented abuse handling strategy | Design | Low |
| 1.4 | README doc map is stale | Integrity | Low |
| 1.5 | Doc numbering scheme inconsistently applied | Integrity | Low |
