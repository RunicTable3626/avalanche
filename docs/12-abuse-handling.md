# User-Generated Abuse Handling

How blocking, spam reporting, and account-level enforcement work in an E2E-encrypted, federated system where the server cannot read message content.

## Goals

1. Give users meaningful tools to control their own inbox (block, mute, refuse first contact).
2. Give homeserver operators a way to act against abusive accounts (rate-limit, suspend, ban) using only metadata and account-level signals.
3. Satisfy App Store Guideline 1.2 (UGC) without compromising the privacy promise: **the server never sees plaintext message content, and reports never contain content.**
4. Stay close to Signal's model — it's well-understood by users, has cleared App Review repeatedly, and the abuse properties are battle-tested.

## Non-goals

- Global content moderation. The server cannot read messages and we will not build client-side reporting that uploads plaintext to a moderation queue.
- Network-wide truth. Each homeserver decides what to act on. There is no global ban list and no central authority.
- Perfect anti-abuse. A determined attacker with multiple identities can always re-contact a target. The goal is to raise the cost enough that casual abuse is unattractive.

## Threat model

Adversaries we care about:

- **Spammer** — automates account creation on a permissive homeserver and sends unsolicited DMs to harvested DIDs. Wants reach.
- **Harasser** — targets a specific user, possibly across multiple identities. Wants attention from the target.
- **Stalker / doxxer** — wants to learn who the target is communicating with, or where they are. Defeated primarily by the E2E layer, not this design.
- **Hostile homeserver operator** — runs a server that ignores abuse reports about its own users. Federation peers can defederate.

Out of scope: state-level adversaries, lawful-intercept demands (covered elsewhere), compromised client devices.

## Mechanisms

The design has four layered mechanisms, in increasing severity:

1. **Message requests** — first contact from an unknown DID is gated behind an explicit accept/decline.
2. **Block** — client-side suppression of a specific DID, with server-side relay refusal as defense in depth.
3. **Spam report** — account-level signal sent to the *reportee's* homeserver. Contains no message content.
4. **Account-level enforcement** — homeserver-side rate limiting, suspension, ban based on report patterns.

Each is described below.

## 1. Message requests

The primary filter. A message from a DID the recipient has never accepted appears in the main conversation list, but opening it presents an Accept / Delete / Report Spam and Block gate before the user can reply or interact further. This matches Signal's UX.

### Recipient UX

The conversation list shows the thread with the sender's claimed display name and a preview of the first message — same as any other conversation. A subtle indicator ("Message request" label or similar) distinguishes it.

When the recipient opens the conversation, instead of the normal compose UI they see:
- Sender's DID, claimed display name, and profile avatar (all client-trusted; see `35-profiles.md`)
- The message thread, read-only
- Three actions at the bottom: **Accept**, **Delete**, **Report Spam and Block**

Until accepted:
- No read receipts are sent (opening to evaluate isn't acknowledgement)
- No typing indicators are sent
- The recipient cannot reply
- Delivery receipts may still be sent (consistent with Signal — the sender learns the message was received, not that it was read)

### Sender-side

The sender cannot tell whether the recipient has accepted, declined, or not yet seen the request. From the sender's perspective the message was delivered.

### When is a sender "known"?

A DID is treated as known (skips the request gate) if any of:
- The recipient has previously sent a message to that DID
- The DID was added via the recipient's contact list / QR scan / invite link flow, even if no message was sent
- The DID is a bot trusted by the homeserver

### Why this works as "filtering" for App Store 1.2

Apple's UGC rule requires "a method for filtering objectionable material." Message Requests *is* that method: unsolicited content is hidden by default and only surfaced when the user opts in. This is the same posture Signal ships and has been accepted by review repeatedly.

## 2. Block

Block is a unilateral, local action that any user can take against any DID at any time. It is the primary remedy *after* a conversation has been accepted.

### Storage

Block list lives in the client's SQLCipher DB as a `blocked_dids` table (`account_id`, `blocked_did`, `blocked_at`). Synced across the user's own devices via existing multi-device sync (sync message body variant: `BlockListUpdate`).

### Client-side effects

When DID X is blocked:
- Incoming messages from X are dropped before rendering (still decrypted to advance the Double Ratchet, but never displayed and never trigger notifications)
- Outgoing messages to X are refused by the local client with a UI explanation ("You blocked this contact")
- X's profile fetches are not performed
- X cannot see typing indicators, read receipts, or delivery receipts from the blocker
- Existing conversation history with X is preserved but the conversation is moved to an "Archived / Blocked" section

### Server-side enforcement (considered, deferred)

We considered letting the client push the block list to its homeserver so the server could refuse to enqueue messages from blocked senders. This would defend against queue-flooding attacks where a blocked sender bombs the recipient's message queue.

**Decision: not in v1.** Signal does not do this — block lists are purely client-side, and the Signal server has no knowledge of who has blocked whom. Pushing block lists to the server is a metadata leak (the server learns each user's full social cut-off list) that diverges from the Signal privacy model we're trying to inherit. The queue-flooding threat is real but narrow, and we don't have evidence it matters in practice.

If we see queue-flooding abuse in the wild, revisit. Possible designs at that point: server-side rate limiting on per-(sender, recipient) message volume (which doesn't require disclosing block lists), or an opt-in server-enforced block list for users under active harassment.

### Unblock

Unblock is symmetric and immediately reverses all of the above. Old messages that were dropped while blocked are not recovered (they were dropped, not held).

## 3. Spam report

The only mechanism that crosses the network. Signal's design, adapted for federation.

### When can a user report?

Following Signal's model: **Report Spam is exposed only in the Message Request UI**, not in established conversations. The reasoning:

- A message request is, by definition, unsolicited contact. Reporting it is the highest-value abuse signal — it's the case where homeserver operators want to act.
- Once a user has accepted a conversation, they made an affirmative choice to engage. The appropriate remedy at that point is block, not report. Block is local; report is a network action with consequences.
- Restricting reports to first contact keeps the abuse-report signal high-quality and reduces the value of weaponized reporting (you can't report someone you've been talking to for a year because you had a fight).

Group invites from unknown senders get the same treatment as DM requests: Accept / Decline / Report.

### What is reported

A spam report contains exactly:

```
{
  reported_did: <DID of the alleged spammer>,
  reporter_homeserver: <URL — for the receiving server to validate the report and rate-limit reporters>,
  reporter_homeserver_signature: <signature over the payload by the reporter's homeserver>,
  reported_at: <timestamp>,
  reason: <enum: spam | harassment | impersonation | other>,
}
```

What is **not** reported:
- The reporter's DID. The reporter's homeserver mediates and vouches for the report's authenticity; the reportee's homeserver only learns "one of your users was reported by some user of homeserver X."
- The message content or any hash of it.
- The conversation history.
- The reporter's IP address or device info.

### Submission flow

1. User taps **Report Spam and Block** in the Message Request UI.
2. Client signs and sends the report to the reporter's *own* homeserver: `POST /v1/abuse/report` with `{reported_did, reason}`.
3. Reporter's homeserver authenticates the request (existing auth token), rate-limits by reporter account (e.g. 20 reports/day), signs the payload, and forwards to the reportee's homeserver: `POST {reportee_homeserver}/v1/abuse/incoming-report`.
4. Reportee's homeserver validates the forwarding-homeserver signature, persists the report, and acks.
5. Client confirms "Reported" and applies a local block.

The reporter's homeserver acts as a privacy shield: the reportee's homeserver learns that homeserver X reported one of its users, but not which user of X. This is similar to email abuse-reporting via ARF: the receiving postmaster learns *that* one of their users was reported, by a peer postmaster they trust to some degree, but not which specific subscriber complained.

### Why a homeserver-mediated report?

Alternatives considered:

- **Direct client-to-reportee-server report**: leaks the reporter's identity (auth token or IP) to the adversary's server. Rejected.
- **Anonymous unsigned report**: trivially forgeable; the reportee's server has no way to rate-limit attackers from drowning a target in fake reports. Rejected.
- **Homeserver-mediated, signed by reporter's homeserver** (chosen): the reportee's server can rate-limit per-reporting-homeserver and apply trust weights to known-good peers, while the individual reporter remains pseudonymous behind their homeserver.

This shifts trust to the reporter's homeserver, which the reporter chose to use. The reporter's homeserver sees who reported whom. That's an accepted privacy tradeoff: your homeserver already knows you're sending DMs to that DID.

## 4. Account-level enforcement

What the reportee's homeserver does with incoming reports. This is operator policy, not protocol, but the design enables it.

### Signals available to the homeserver

For each local account, the homeserver tracks:
- Incoming abuse reports (count, distinct reporting homeservers, recency, reason distribution)
- Send-rate metrics (messages/hour, distinct recipients/day, % of messages to DIDs that never reply)
- Account age and prekey rotation patterns (botnet accounts churn through prekeys fast)
- Registration metadata (IP, time-of-day, captcha challenge if used)

None of these involve message content.

### Enforcement ladder

Operators define their own thresholds. Suggested defaults:

| Trigger | Action |
|---|---|
| 5+ distinct reporters in 24h | Rate-limit: throttle outgoing messages to 1/min for 24h |
| 20+ distinct reporters in 7d, or 50+ total | Suspend: account cannot send messages, can receive (so they can learn they're suspended). Owner can appeal via support contact. |
| Confirmed abuse after operator review, or 100+ reports | Ban: account terminated, DID document deleted (tombstone — see account deletion design), prekeys revoked. |

"Distinct reporters" is counted at the homeserver level (one homeserver = one reporter, regardless of how many users on that homeserver reported). This prevents a single hostile homeserver from manufacturing reports.

### Operator transparency

Suspended/banned users receive a system message (sent via a control-message envelope from the homeserver's reserved DID) explaining the action and how to appeal. The homeserver publishes aggregate enforcement stats periodically (e.g. "1,247 accounts suspended this quarter for spam") for transparency.

### Cross-server defederation

If a homeserver consistently ignores reports about its users — i.e., known bad actors keep operating from it — peer homeservers can defederate: refuse to accept messages from accounts on that server. This is a heavy hammer (cuts off all that server's users, not just abusers) and is operator policy, not in-protocol.

A trust score per peer-homeserver (computed from how often their accounts get reported, how responsively they act on forwarded reports) gives operators data to make defederation decisions without making them in the dark.

## 5. Profile-level abuse

Display names, avatars, and bios are user-supplied profile fields visible to anyone who can fetch the profile (see `35-profiles.md`). These are abuse vectors distinct from message content because they're broadcast, not addressed.

Mitigations:
- Client-side display-name profanity filter (configurable, on by default). Filtered names render as "[name hidden]" with a tap-to-reveal.
- Profile reports use the same mechanism as message-request reports, but reason `impersonation` or `objectionable_profile`. The reporting client includes a *signed snapshot* of the offending profile so the reportee's homeserver can verify the report (since profile contents change).
- Operator action on profile reports: force a profile reset (clearing display name, avatar, bio to defaults) before allowing further sends. Repeat offenses → suspend.

## 6. UI surfaces summary

| Surface | Affordance |
|---|---|
| Message Request inbox | Accept / Delete / Report Spam and Block |
| Conversation menu (accepted) | Block / Mute / Disappearing messages settings |
| Profile view | Block / Report Profile (for impersonation/objectionable name or avatar) |
| Settings → Privacy → Blocked | Block list management (unblock) |
| Settings → Privacy → Server-enforced blocking | Opt-in toggle for relaying block list to homeserver |
| Group invite | Accept / Decline / Report group invite |

## 7. What we explicitly do not build

To preserve the privacy promise and avoid encouraging weaponized reporting:

- No content reporting. The server never receives plaintext or hashes of plaintext.
- No "report" button in established conversations. Use block.
- No global ban list. Each homeserver decides independently.
- No client-side ML moderation of incoming messages.
- No proactive content scanning (CSAM-hash scanning, etc.) on-device. If legally compelled in some jurisdiction, this would be a hard design conflict; defer to legal review.
- No reverse search of report history ("show me everyone who has ever reported me"). Reports are operator data, not user data.

## 8. App Store 1.2 mapping

For Apple review, the four requirements map cleanly:

| Apple requirement | Our implementation |
|---|---|
| Method for filtering objectionable material | Message Requests (gates first contact); client-side profile name filter |
| Mechanism to report offensive content + timely response | Report Spam in Message Request UI → homeserver-mediated report → operator action ladder |
| Ability to block abusive users | Per-account block list, client-enforced |
| Published contact info | Support email in app and App Store listing |

In review notes, explicitly call out: "This is an end-to-end encrypted messaging app. The server cannot read message content. Abuse handling is account-level, following the model used by Signal, WhatsApp, and other E2E messengers."

## 9. Open questions / future work

- **Cross-homeserver report aggregation.** When the same DID is reported by users on many homeservers, no single homeserver sees the full picture. A privacy-preserving aggregation protocol (e.g. PSI-style intersection of report sets) would help. Out of scope for v1.
- **Group abuse**. Group spam (mass-add-to-group) needs its own design. Probably: group invites land in a request queue similar to DMs; admins can rate-limit invites; reporting a group reports the admin who added you.
- **Project abuse** (per `20-project-security.md`). When third-party Projects can interact with users, reports against the *Project* (not the user) need a different routing path — probably to the Project developer's homeserver and to the platform.
- **Recovery after false report**. If a user is suspended based on coordinated false reporting, what's the appeal flow? At minimum: a support contact and operator-side ability to review and clear the suspension. Standardize across reference homeserver.
- **Federation trust scoring**. How do homeservers build reputation for each other? Could be as simple as a manually-curated trust list at first.
