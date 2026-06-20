# Supergroups: large-scale broadcast channels

Status: **design-only, deferred.** This is a sketch of how very large groups
(200+ members, scaling to thousands and beyond) should work. It is **not** scheduled
for any near-term stage — it is recorded now so the shape is known and so that
`docs/03-groups.md` (the normal-group design) doesn't get stretched to cover a problem
it isn't built for. Sections are mostly **PROPOSED** / **OPEN** rather than **DECIDED**;
this doc exists to capture the reasoning, the tradeoffs, and the one genuinely hard
unsolved sub-problem (anonymous reaction de-duplication, §7), not to commit an
implementation.

Background reading:

- `docs/03-groups.md` — normal (action-bound and casual) group design. Supergroups are a
  separate primitive; this doc assumes familiarity with §2 (zkgroup membership), §3.7
  (push fan-out), §3.9 (membership-opacity discipline), and §3.11 (sealed-sender sends).
- `docs/32-threading.md` — threaded replies. Supergroups deliberately diverge from its
  "every reply is a thread in the same conversation" model (§4.2 below).
- `docs/33-reactions.md` — per-member client-tallied reactions. Supergroups diverge to a
  server-counted model (§4.3 below).
- Chase, Perrin, Zaverucha 2019, *The Signal Private Group System*; MLS (RFC 9420) as the
  principled long-run answer to large dynamic E2E groups.

## 1. What a supergroup is, and why it's a separate primitive

A **supergroup** is a large, mostly one-directional broadcast channel: a small admin set
posts announcements; a large membership (hundreds to many thousands) receives them.
Members can react and can discuss, but discussion does **not** flow back into the
broadcast channel as peer-to-everyone messages (§4.2).

A supergroup is **not** the same thing as a normal group with the `announcement_only`
policy flag set. That flag stays exactly as designed in `docs/03-groups.md`: any normal
group can be announcement-only, and that's the right tool up to ~150–200 members. A
supergroup is a distinct primitive that trades some of the normal-group guarantees (§3)
for the ability to scale an order of magnitude or more past where the normal-group send
path falls over.

**Design principle: a supergroup should feel like an announcement-only group, not a new
thing to learn.** *Mechanically* it is a distinct primitive (§3 onward); *experientially*
it should be as close to indistinguishable from a small announcement-only group as
possible — same compose/read/react/reply surfaces, same mental model. The divergences exist
only to make it scale, and should stay **below the UX surface**: single-blob storage, pull
delivery, server-counted reactions, admin-only sending, and the pseudonymous send path are
all mechanism, not UI. Where full transparency isn't achievable at scale, the visible
difference should be minimized and degrade gracefully (e.g. reaction *counts* rather than
per-reactor faces — §4.3). This principle is what makes promotion (below) feel seamless,
and it is the tie-breaker whenever a scaling choice would otherwise leak into the
interface: prefer the option that keeps the announcement-only-group experience intact.

**The recommended way to become a supergroup is promotion from a normal group, not
selection at creation (§5).** A group is born normal and is promoted when it actually
grows large; direct create-as-supergroup stays available for the rare case of standing up
a channel you already know will be huge.

**Size gate (PROPOSED): the supergroup tradeoffs apply only at 200+ members.** Below that,
normal groups (optionally announcement-only) are strictly better — they keep full sender
anonymity and the uniform threading model. So the gate is a **promotion trigger, not a
creation fork**: crossing ~200 members is what prompts (or, past a hard ceiling, requires)
promotion. See §5 for the lifecycle.

### Why one primitive can't do both

The normal-group send path (`03` §3.11) encrypts each message **once** under a Sender Key
and wraps it in a `sealed_sender_multi_recipient_encrypt` envelope with one slot per
recipient device. That's already good — the *payload* is not re-encrypted per recipient.
But three costs remain linear in membership N, and they're the wall:

1. **Per-recipient envelope sealing** — O(N·devices) key-wraps by the sender, purely to
   preserve sender-among-members anonymity.
2. **Per-recipient storage** — today the server stores the full concatenated
   `received_message` once per recipient (`crypto::sealed_sender::parse_sent_message`
   flattens libsignal's shared-body/per-recipient-header split with `.concat()`, and
   `db::group_messages::enqueue` writes one row per recipient). The shared body is
   duplicated N times.
3. **Sender-Key distribution** — when a member joins, *they* broadcast their own Sender Key
   to all existing members (O(N) per join, O(N²) over a group's life), so that any member
   can later send to everyone.

For a small group all three are free. For a 10k-member channel they are fatal — and cost
(3) plus "any member can reply to everyone" also turns the channel into a spam megaphone.

**The honest framing.** Large-scale broadcast with dynamic membership and E2E is close to
the open problem MLS was built for. The mass-market products this primitive resembles —
WhatsApp Channels, Telegram channels — are **not** end-to-end encrypted, and that is not an
accident. A supergroup therefore makes a **deliberate, documented confidentiality
tradeoff** that normal groups do not (§3). The alternative to making that tradeoff
explicit is silently stretching the normal-group crypto until it breaks at scale.

## 2. What carries over unchanged

- **Membership opacity (`03` §3.9).** The server still holds no `(did → groups)` or
  `(encrypted_member_id → did)` mapping. Members are `encrypted_member_id`s under the
  group key (`03` §2.3). Everything below preserves this; where a mechanism weakens
  *sender* opacity it is called out explicitly.
- **Sender Keys for content (`03` §6).** Content is encrypted once. Supergroups keep this;
  what changes is who holds a sending key and how the single ciphertext is stored and
  delivered.
- **Relay push (`03` §3.7).** Content-free wakeups by `group_push_pseudonym` via the relay
  are reused as-is. Push fan-out stays O(N), but a wakeup ping is the cheap part — it
  carries no content and triggers a pull.
- **MLS as a future swap.** `03` §6 keeps the group encryption scheme behind an
  `encrypt`/`decrypt` interface. If supergroups are ever built for real, MLS is the
  principled candidate for the broadcast key schedule and should be evaluated before the
  shared-key approach in §4.1 is committed.

## 3. The confidentiality tradeoff (what supergroups give up)

Two relaxations versus normal groups, both **scoped to the broadcast path only**:

1. **Admin sends are pseudonymous, not identified. DECIDED (tier 2).** The server can verify
   the poster is *an admin* — enough to enforce announcement-only — but does **not** learn
   *which* admin's DID. This reuses normal-group machinery rather than inventing a send
   path; §4.1 records all three tiers (identified / pseudonymous / fully-unlinkable) and why
   pseudonymous is chosen.
   - **No admin-roster carve-out — `03` §3.9 stays fully intact.** The admin set is stored
     exactly like normal-group `member_credentials`: opaque `encrypted_member_id`s carrying
     `role = Admin`, with **no DID column**. Each send carries a zkgroup `AuthCredential`
     presentation the server verifies against that admin set (`03` §2 / §3.11), learning "a
     valid admin posted" without learning who. A seized server yields no organizer list —
     for a channel of thousands, the single most important property to preserve.
   - **Residual linkage — this is the actual relaxation.** The server sees a *stable opaque
     admin id* per send, so it can link one admin's posts to each other, but never to a DID
     or to any other server identifier (`03` §3.9 rules 2–4 still hold). This is weaker than
     normal-group message sends, which are fully unlinkable via sealed sender + endorsements
     (that's tier 3, rejected in §4.1 as not worth it for a small admin set).
   - **Scope.** Readers and reactors are already opaque (`encrypted_member_id`, no DID) and
     reactions are posted pseudonymously (§4.3); nothing here changes that.
2. **Forward secrecy on broadcast content is weaker.** A small, long-lived admin sending
   key (or a channel content key) doesn't ratchet per-recipient the way pairwise sessions
   do. Mitigated by periodic key rotation / epochs; accepted as a tradeoff for O(1) sends.

What is **not** given up: **membership opacity is fully intact** (`03` §3.9) — the server
enumerates neither members nor admins; all are opaque `encrypted_member_id`s with no DID
column. Also kept: content confidentiality against the server (it relays ciphertext it
can't read) and member-side anonymity (a member who only reads/reacts is never identified).
The *only* relaxation versus normal groups is that an admin's broadcasts are linkable to
each other by a stable opaque id (relaxation 1) and weaker FS on content (relaxation 2).

## 4. Architecture: three separate mechanisms

The key aspect of this design is to split the three things members do (**receive**, **reply**, and **react**), instead of forcing all three through one
group-message path:

| Signal | Requirement | Mechanism |
|---|---|---|
| Announcement (admin → all) | one-directional, readable by all | §4.1 single shared ciphertext + pull |
| Reply (member → thread) | readable by all, read on demand | §4.2 per-announcement threads, pulled |
| Reaction (member → channel) | counted and visible, not individually read | §4.3 server-counted opaque tokens |

The unifying principle: **the cost is in _delivery_, not _readability_.** Anything that has
to be *pushed* to all N members is the broadcast wall; anything that can be *pulled* on
demand (announcements, reply threads) or *counted* server-side (reactions, reply counts)
sidesteps it. Supergroups push nothing but content-free wakeups.

### 4.1 Announcement broadcast (one-directional content)

**PROPOSED.**

- **Only admins send.** Sending is gated to the admin set. Consequently, **only admins
  seed and distribute Sender Keys**; a non-admin member never creates or broadcasts a
  sending key. This alone removes the O(N²) SKDM cost — a new member only *receives* the
  admins' keys.
- **Content encrypted once, stored once.** The admin encrypts the announcement a single
  time. The server stores **one** ciphertext per message keyed by the supergroup, not one
  copy per recipient. Content *and the sender certificate* ride the shared supergroup read
  key (§4.2), so the server can read neither — it holds a **single opaque blob with no
  per-recipient header**. (Concretely, versus `03`'s send path: stop producing libsignal's
  per-recipient `[version, recipient_key_material, shared_bytes]` fan-out at all; there's
  one body, decryptable by any member holding the read key.)
- **Delivery = content-free wakeup + pull.** Members are woken by the existing relay push
  (`03` §3.7) and **pull** the single stored ciphertext via a membership-gated fetch (the
  404-for-non-members rule of `03` §3.4 applies, so the fetch leaks nothing about
  membership). No per-recipient queue rows for the body.
- **Server-enforced announcement-only.** Each send carries a zkgroup `AuthCredential`
  presentation; the server verifies it against the admin `member_credentials` set and
  *rejects* a non-admin at the endpoint — closing the gap that `announcement_only` on normal
  groups is **client-enforced only** (`03` §3.11 / `03-groups.md:421`): there the server
  can't check who sent under full sealed sender. Here it can check *that an admin sent*
  without learning which (§3.1).

**Sender anonymity: pseudonymous among admins (tier 2). DECIDED (§3.1).** The guiding rule
is *harmonize with the existing normal-group infrastructure, diverge only where it doesn't
scale.* So the send path **reuses** `03`'s zkgroup presentation machinery, and diverges only
on storage/delivery:

- **Auth — reused verbatim.** A zkgroup `AuthCredential` presentation (`03` §2 / §3.11),
  verified against the opaque admin `member_credentials` set. No new send credential, no
  DID, no admin roster.
- **Storage/delivery — diverges, because this is the part that doesn't scale.** Single
  opaque blob under the shared read key + pull, instead of `03`'s per-recipient sealed-sender
  fan-out (§1's O(N) wall).

Three tiers were considered, by how much the server learns about the sender:

1. **Identified** — session-bearer auth; server learns the admin DID and needs a
   `(group → admin DIDs)` roster (a `03` §3.9 carve-out). Cheapest, but hands a seized server
   the organizer list. **Rejected.**
2. **Pseudonymous (chosen)** — `AuthCredential` presentation; server learns "an admin" and a
   stable opaque id, never a DID; `03` §3.9 fully intact. Reuses existing machinery; cost is
   one presentation verify per send (a few ms of Ristretto ops — negligible at announcement
   frequency, which is low).
3. **Fully unlinkable** — `GroupSendEndorsement` + sealed sender (`03` §3.11); the server
   can't even link an admin's posts to each other. For a small admin set this buys little
   over tier 2 and costs the entire anonymous-send pipeline. **Rejected as not worth it.**

**Accountability cost (tiers 2–3).** No clean per-admin attribution server-side, so abuse
controls fall back to per-IP / per-group rate limits (`03` §3.11) and a compromised admin
device can't be pinned to a DID by the server. Accepted: for a small admin set this is mild,
and not handing a seized server the organizer list outweighs it. (Members still see *which*
admin authored each post — the sender cert is inside the read-key blob, so authorship and
in-app moderation are unaffected; only the *server's* view is pseudonymous.)

**Residual cost.** Push wakeups stay O(N) (one cheap ping per member device), and the
membership/credential bookkeeping is still per-member. The expensive parts — content
crypto and content storage — become O(1).

### 4.2 Replies → pull-based per-announcement threads

**PROPOSED.** Replies hang off the announcement they answer, like comments under a post:
the channel shows "5 replies" on the announcement, and any member can click in to read
them. The thing that makes this cheap at scale is that **a reply is _readable_ by everyone
but _delivered_ to no one** — it is pulled on demand, never pushed.

The earlier instinct to route replies into a separate group came from conflating *readable
by all* with *delivered to all*. They are different costs:

- **Push** (deliver each reply to all N members) → O(N) fan-out + notifications + spam
  megaphone. This is the wall, and supergroups avoid it.
- **Pull** (store the reply anchored to its announcement; members who open the thread fetch
  it) → no fan-out, no notifications, not a megaphone. This is what "see 5 replies, click to
  read" actually is.

So replies are in-channel threads, handled as pulls:

- **Count is free.** A reply increments a server-side counter per announcement — the exact
  mechanism reactions use (§4.3 / §7). The channel renders "5 replies" with no content
  delivery.
- **Reading is a pull.** Click a thread → fetch that announcement's stored replies
  (membership-gated, `03` §3.4's 404-for-non-members rule applies) → lazily fetch the Sender
  Keys of any repliers you don't already hold → decrypt. Cost is bounded by *how many people
  replied in the thread you opened*, never by channel size.
- **Writing is O(1).** A replier encrypts once under their own Sender Key and stores the
  reply once, anchored to the parent `message_id`. No fan-out, no eager O(N²) key push.
- **Authenticated, not a megaphone.** Replies are authored under the replier's signed Sender
  Key, so authorship is verifiable. Because they are never pushed and never notify
  (threading replies are quiet-by-default, `32`), an abusive reply is a comment buried under
  a post that only interested members open — bounded harm, handled with reply rate limits +
  admin moderation/removal, not by forbidding replies.

**New requirement: a supergroup read key.** Lazy reply-key fetch needs any member to be able
to unwrap any replier's SKDM. The clean way is a **supergroup-wide symmetric read key held
by all members** (rotated per epoch — the same weaker-FS tradeoff already accepted in §3.2),
used to wrap reply SKDMs so a reader can unwrap on demand. This generalizes §4.1's "members
hold the keys needed to read broadcasts" from admin-only senders to member repliers; the
per-author Sender Key still provides authorship, the shared read key only carries
distribution.

**Consistency with `docs/32-threading.md`.** This *keeps* `32`'s model rather than
overturning it: replies are in-channel threads in the same encrypted conversation (so
`32:28`'s "no subset threads" holds — the earlier separate-group design was the thing that
broke it), quiet-by-default (`32:52`), and **surfacing a reply to the channel feed remains
the admin-gated post-to-channel capability** (`32:81`). The only supergroup-specific change
is delivery: replies are pulled, not fanned out. Normal groups are unchanged.

**Secondary affordance (optional).** For sustained back-and-forth that wants its own space,
a thread can still spin off into a real linked discussion group (an ordinary `03` group with
opt-in membership). That's a *user choice for a side-conversation*, not the primary reply
mechanism — comments-under-a-post is the default.

### 4.3 Reactions → server-counted opaque tokens

**PROPOSED.** Reactions must be **live and visible to everyone**, and must **not depend on
any particular member (e.g. the author) being online** to tally them. That rules out
author-side aggregation. The server can aggregate — not by reading reactions, but by
**counting opaque tokens it cannot interpret**:

- A reactor derives an opaque token for their emoji under the group key (the server cannot
  tell 👍 from 🎉) and posts it pseudonymously (sealed-sender-style) with a per-message
  authorization (see §7).
- The server keeps a count per `(message, token)` — **no DID, no identity**, just "message
  M received 47 of token T." This adds no `did`-linked table, so `03` §3.9 still holds.
- Members hold the group key, so they **pull** the counts (riding the same fetch as the
  announcement), map tokens back to emoji locally, and render "👍 1.2k 🎉 340."
- **Liveness:** the server is always online, so counts update as reactions arrive and are
  visible to everyone who pulls — no online-author dependency, satisfying the "visible to
  others" requirement (a shared, live count is exactly what "1.2k 👍" is everywhere).

This **diverges from `docs/33-reactions.md`**, whose per-member client-tally requires
delivering every individual reaction to every member — which at supergroup scale is the
same O(N²) broadcast wall. `33` stays correct for normal groups; supergroups substitute
server-counted aggregates.

**Costs, stated plainly:**

1. **Popularity metadata.** The server learns *how many* reactions a post got (not who, not
   which emoji). Minor — roughly inferable from send/read volume anyway.
2. **Anonymous anti-stuffing** — the genuinely hard part; see §7.

What's lost vs `33`: per-reactor visibility for everyone (you see the count, not the faces,
in a 10k channel). That matches every large platform and is the expected UX at this size.

## 5. Lifecycle: created normal, promoted to supergroup (recommended)

**PROPOSED — promotion is the primary path; create-time selection is the exception.**

Rationale:

- At creation you rarely know whether a group stays small or grows huge. Forcing the
  choice up front either burdens a 12-person group with confidentiality tradeoffs (§3) it
  doesn't need, or boxes a normal group in when it hits the wall at scale.
- Normal groups are **strictly better below the gate** (full sender anonymity, uniform
  threading per `32`, per-member reactions per `33`). A group should stay normal as long as
  it can and take on the supergroup tradeoffs only when scale forces them.
- So every group is **born normal**. When it approaches/crosses the ~200 gate, admins are
  offered (or, past a hard ceiling, required) to **promote** it. Direct create-as-supergroup
  stays available only for the rare known-large case.

This is consistent with §4.2's "switch on the channel flag, not a size threshold":
**promotion is precisely the act of setting that flag.** The flag stays the single visible
line; crossing 200 only *prompts* the action — it never silently flips behavior.

**What promotion changes** (all forward-going; `group_id` and membership persist):

- **Send path:** per-recipient sealed-sender fan-out (`03` §3.11) → single shared
  ciphertext + pull (§4.1).
- **Sender keys:** non-admins stop being senders; their sending keys are retired. Only
  admins keep and seed sending keys.
- **Replies:** in-channel threads stay in-channel threads, but delivery switches from
  fan-out to pull-per-announcement (§4.2). The UX (reply, see "N replies", click to read)
  is unchanged.
- **Reactions:** per-member client-tally (`33`) → server-counted aggregates (§4.3).
- **Confidentiality:** the group takes on §3's relaxed sender anonymity / weaker forward
  secrecy from this point on.

Because promotion **weakens a security property**, it must be:

- **Explicit and admin-driven** — never automatic or silent.
- **Member-visible** — a "this group became a channel" state-change event, so members
  understand the property change *before* they next post (analogous to `32`'s surfacing
  being a visible action).
- **Effectively one-way.** Demotion is not a goal: it would have to re-establish per-member
  sending keys across a now-huge membership, and it can't retroactively restore
  anonymity/FS to messages already sent under the broadcast model. Treat promotion as a
  one-way door (matching `32`'s no-demote stance). If a smaller private space is wanted
  later, start a new group.

**History.** Messages sent *before* promotion keep the properties they were sent under
(per-recipient, fully sender-anonymous); the broadcast store (§4.1 / §6) begins at
promotion. Because replies remain in-channel threads (just pulled instead of fanned out),
pre-promotion threads carry over as-is — there is no separate-conversation migration to do.
New replies after promotion land in the pull-based reply store (§6).

## 6. Wire / storage sketch (illustrative, not committed)

```
// Broadcast store: one ciphertext per message, not per recipient.
supergroup_messages(
  group_id     bytea,   // public supergroup id
  message_id   bigint,
  ciphertext   bytea,   // single shared body (admin sender key / channel key)
  posted_at    timestamptz,
  expires_at   timestamptz
)

// Reaction tally: opaque tokens, no identity column (preserves 03 §3.9).
supergroup_reactions(
  group_id     bytea,
  message_id   bigint,
  token        bytea,   // opaque per-emoji token under the group key
  count        bigint   // incremented by verified, de-duplicated reaction posts
)

// Per-announcement reply threads: stored once, pulled on demand (§4.2).
supergroup_replies(
  group_id     bytea,
  parent_id    bigint,  // the announcement this reply hangs off
  reply_id     bigint,
  ciphertext   bytea,   // reply body under the replier's Sender Key; SKDM wrapped
                        //   under the supergroup read key for lazy fetch (§4.2)
  posted_at    timestamptz,
  expires_at   timestamptz
)
// "N replies" is a COUNT(*) over parent_id (or a denormalized counter on
// supergroup_messages), tallied like reactions — content is never delivered, only pulled.
```

Endpoints (sketch): an admin-authenticated `POST /v1/supergroups/{id}/announce` (server
checks admin eligibility and stores one row); a membership-gated
`GET /v1/supergroups/{id}/messages` (404 for non-members, pull); a pseudonymous
`POST /v1/supergroups/{id}/react` (verifies the per-message authorization of §7, increments
the opaque tally); a membership-gated `GET /v1/supergroups/{id}/reactions?message_id=…`;
a member-authenticated `POST /v1/supergroups/{id}/messages/{parent_id}/replies` (stores one
reply row, bumps the parent's reply count, rate-limited) and a membership-gated
`GET /v1/supergroups/{id}/messages/{parent_id}/replies` (404 for non-members, the
click-to-read pull). Push wakeups reuse the `03` §3.7 relay path.

## 7. The hard open problem: anonymous reaction de-duplication

Counting opaque tokens means a single member could inflate a count by reacting many times,
and under membership opacity the server **cannot** tell "this is the same member again." A
correct fix needs a per-`(member, message)` **nullifier**: a deterministic token that is
identical if the same member reacts twice to the same message but reveals nothing about who
they are (the anonymous-voting / nullifier family — e.g. `PRF(member_secret, message_id)`
proved well-formed in zero knowledge against the member's group credential).

This is real cryptographic machinery and is **the** piece that cannot be hand-waved. Two
positions:

- **Do it right:** a nullifier scheme over the zkgroup membership credential. Most work,
  cleanest guarantee (exactly-once per member, fully anonymous).
- **Approximate it:** rely on per-IP rate limits + the existing `GroupSendFullToken`-style
  per-member-per-day endorsement budget (`03` §3.11), and accept that supergroup reaction
  counts are *approximate*. Big-channel counts are approximate everywhere; this is likely
  acceptable for a first cut and defers the nullifier scheme.

Recommendation: if/when supergroups are built, start with the approximate path and treat
the nullifier scheme as a follow-on, gated on whether count-stuffing is actually observed.

## 8. Relationship to existing docs (what each change touches)

- `docs/03-groups.md` — supergroups are a **new sibling primitive**, not a modification.
  The `announcement_only` flag and its client-side enforcement stay as-is for normal
  groups. Supergroups reuse `03`'s membership opacity **fully intact** (§3.9), zkgroup
  member IDs (§2.3), the `AuthCredential` presentation path for admin-send auth (§2 /
  §3.11), and relay push (§3.7). They diverge only where `03` doesn't scale: the
  per-recipient sealed-sender send/store/fan-out path (§3.11) is replaced by the single-blob
  + pull model (§4.1–§4.2).
- `docs/32-threading.md` — supergroups **stay consistent** with `32`: replies are in-channel
  threads in the same conversation (`32:28`), quiet-by-default (`32:52`), and surfacing to
  the channel feed is the admin-gated post-to-channel capability (`32:81`). The only
  supergroup-specific change is *delivery* — replies are pulled per-announcement, not fanned
  out (§4.2). `32` is unchanged for normal groups.
- `docs/33-reactions.md` — supergroups **diverge** to server-counted opaque tokens (§4.3),
  and reply *counts* use the same tally mechanism. `33`'s per-member client-tally is
  unchanged for normal groups.

## 9. Forward compatibility: receiving on un-updated clients (recommendations)

Nothing here is decided — this section records *recommendations* for keeping the door open,
so that a future supergroup build (and especially **promotion** of an existing group, §5)
doesn't strand members who haven't updated their app. The asymmetry that motivates it: we
can reasonably require **admins** to update before they create or post to a supergroup, but
we can't expect the large **receiving** membership to update in order to keep reading a
channel they're already in. So the receive path is the one to protect.

The encouraging starting point is that most of the scaling work is **not receiver-visible**:
storage de-duplication is server-side and transparent (a client still pulls "its" message as
today); push wakeups are already O(N) content-free pings; per-recipient sealing is *sender*
(admin) CPU; admin-only Sender-Key distribution is sender/membership-side. The
receiver-visible new machinery — the shared read key, pull endpoints, server-counted
reactions, pull reply threads — is a **feature layer**: a client that can't do it can still
receive the announcement itself and simply not render the "N replies" affordance or live
reaction counts. Graceful degradation, not breakage.

Recommendations, roughly in order of value-for-cost:

- **Keep a legacy receive representation in mind.** If a supergroup announcement can also be
  delivered as an ordinary Sender-Key group message on today's receive path, then any client
  that can receive a group today can receive a promoted channel's announcements with no
  update, and promotion is transparent to old members (same `group_id`, same delivery
  channel). Note the server can't manufacture this — it has no keys — so it would be the
  *updated admin client* that emits the legacy envelopes for non-updated members (i.e. the
  existing `03` §3.11 fan-out, used as a fallback). This is a recommendation to preserve the
  option, not a committed wire format.
- **A per-device capability/version signal would have lead-time value.** We don't have one
  today. It isn't needed for *correctness* (a legacy representation covers that), but it's
  what would let a sender send the efficient single-blob to updated clients while falling
  back to legacy only for the laggards — i.e. it's the difference between "legacy-to-everyone,
  zero scaling during the transition" and "scaling that grows as the fleet updates." Because
  it only helps clients that update *after* it ships, planting a minimal per-device protocol
  version early (it's `03` §3.9-safe — per-device, no group link) maximizes the
  known-capable population by the time supergroups actually land. Worth considering well
  ahead of building supergroups; not urgent.
- **Graceful handling of unknown message types is general hygiene.** The whole story rests on
  un-updated clients **skipping `ContentMessage` body variants they don't understand** rather
  than erroring. That protects *all* future protocol evolution, not just supergroups, and it
  can only be fixed in clients that update — so the sooner the deployed fleet handles unknowns
  gracefully, the safer every later addition is. Worth verifying independently of this doc.

Honest caveat: a legacy fallback buys **correctness** for laggards, not **efficiency**. Until
a capability signal exists and the fleet has largely updated, a promoted supergroup would see
little scaling benefit (the admin still fans out legacy envelopes to old/unknown members). The
trade is the right way round, though — correctness is the part that can't be fixed after the
fact, and efficiency improves on its own as clients update.

## 10. Open questions / deferred

1. **Broadcast key schedule** — single long-lived admin sending key + rotation, a dedicated
   channel content key, or MLS. Evaluate MLS before committing (§2, §3).
2. **Anonymous reaction de-duplication** — nullifier scheme vs approximate rate-limiting
   (§7).
3. **Reply abuse controls** — rate limits and admin moderation/removal for pull-based
   replies (§4.2). Bounded harm (never pushed/notified), but still needs a moderation story.
4. **Membership management at scale** — joins/leaves, supergroup-read-key rekeying cadence
   (§4.2), and how `03` §3.9-compatible push-pseudonym bookkeeping scales to tens of
   thousands of rows.
5. **Optional spin-off discussion groups** — UX for promoting a thread into a standalone
   `03` group for sustained back-and-forth (§4.2 secondary affordance).

These are recorded for when the primitive is picked up; none is a blocker now, because the
whole doc is deferred.
