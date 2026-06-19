// actnet adminbot — the canonical first-party bot.
//
// v1 responsibilities (per docs/22-adminbot.md):
//   - Register a bot account at the reserved DID `did:local:adminbot`
//     (server-side default; override via ADMINBOT_DID on the server).
//   - Create the `#admins @ {hostname}` group, invite the DIDs listed in
//     ADMINBOT_INITIAL_ADMINS at bootstrap.
//   - Auto-invite every new human account (AccountJoinedEvent WS push) to
//     every group adminbot is currently an admin of — `#admins` and any
//     other group it's been added to as admin.
//   - Respond to `/whoami` and `/help` in `#admins`.
//
// Persistent state:
//   - SQLCipher DB at ADMINBOT_STATE_DIR/store.db — owned by app-core.
//   - JSON sidecar at ADMINBOT_STATE_DIR/state.json — adminbot's own
//     bookkeeping (group id, already-invited initial admins).

import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

import {
  AppCore,
  initLogging,
  type AdminEvent,
  type IncomingEvent,
  type SendTarget,
} from "@actnet/app-core";

// Reserved well-known suffix for the canonical adminbot account. This also
// matches the server's superuser Project slug (ADMINBOT_PROJECT_SLUG), so the
// bootstrap token below both registers the bot and links it into the superuser
// Project — granting admin authority (docs/24).
const ADMINBOT_DID_SUFFIX = "adminbot";
const ADMINBOT_DID = `did:local:${ADMINBOT_DID_SUFFIX}`;
const SUPERUSER_PROJECT_SLUG = "adminbot";

interface AdminbotState {
  adminsGroupId?: string;
  invitedInitialAdmins?: string[];
}

interface Env {
  serverUrl: string;
  stateDir: string;
  dbPath: string;
  statePath: string;
  dbKey: string;
  initialAdmins: string[];
  logLevel: string;
  sharedSecret?: string;
}

function readEnv(): Env {
  const serverUrl = process.env.ADMINBOT_SERVER_URL;
  if (!serverUrl) {
    throw new Error("ADMINBOT_SERVER_URL is required");
  }
  const stateDir = process.env.ADMINBOT_STATE_DIR ?? "./adminbot-state";
  mkdirSync(stateDir, { recursive: true });
  const initialAdmins =
    process.env.ADMINBOT_INITIAL_ADMINS?.split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0) ?? [];
  return {
    serverUrl,
    stateDir,
    dbPath: join(stateDir, "store.db"),
    statePath: join(stateDir, "state.json"),
    dbKey: process.env.ADMINBOT_DB_KEY ?? "",
    initialAdmins,
    logLevel: process.env.ADMINBOT_LOG ?? "info",
    // Bootstrap secret for closed-registration servers (docs/24). Required to
    // register against a closed server; unset/ignored on an open one.
    sharedSecret: process.env.REGISTRATION_SHARED_SECRET || undefined,
  };
}

function loadState(path: string): AdminbotState | null {
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, "utf8")) as AdminbotState;
}

function saveState(path: string, state: AdminbotState): void {
  writeFileSync(path, JSON.stringify(state, null, 2));
}

function adminsTitle(serverUrl: string): string {
  return `#admins @ ${new URL(serverUrl).hostname}`;
}

async function loginOrRegister(env: Env): Promise<AppCore> {
  // Register on first run, re-login thereafter. app-core decides which based
  // on whether the store already holds an account (including the empty-DB-from-
  // a-failed-registration case) — adminbot only supplies the reserved DID.
  // Bootstrap token naming the superuser Project: registers the bot (against a
  // closed server) and links it into the superuser Project, granting admin
  // authority. Only consulted on first-run registration; ignored on re-login.
  const inviteToken = env.sharedSecret
    ? AppCore.bootstrapToken(env.serverUrl, env.sharedSecret, SUPERUSER_PROJECT_SLUG)
    : undefined;
  const core = await AppCore.loginOrCreateBot(
    env.serverUrl,
    env.dbPath,
    env.dbKey,
    "Adminbot",
    ADMINBOT_DID_SUFFIX,
    inviteToken,
  );
  // Identity policy is ours, not the core's: the store must belong to the
  // reserved adminbot DID. A mismatch means this state dir was created by a
  // different bot, or the server handed back an unexpected DID.
  if (core.did() !== ADMINBOT_DID) {
    throw new Error(
      `local store DID (${core.did()}) is not the reserved adminbot DID ` +
        `(${ADMINBOT_DID}); this state dir belongs to a different bot`,
    );
  }
  return core;
}

async function withRetry<T>(label: string, fn: () => Promise<T>): Promise<T> {
  // Race against server startup in dev-all and against transient errors.
  let delayMs = 500;
  for (;;) {
    try {
      return await fn();
    } catch (e) {
      console.error(`adminbot: ${label} failed: ${(e as Error).message}; retrying in ${delayMs}ms`);
      await new Promise((r) => setTimeout(r, delayMs));
      delayMs = Math.min(delayMs * 2, 30_000);
    }
  }
}

async function ensureAdminsGroup(core: AppCore, env: Env, state: AdminbotState): Promise<string> {
  if (state.adminsGroupId) return state.adminsGroupId;

  const title = adminsTitle(env.serverUrl);
  console.log(`adminbot: creating group "${title}"`);
  const created = await core.createGroup(title, "Server administrators.", 0);
  state.adminsGroupId = created.groupId;
  saveState(env.statePath, state);
  return created.groupId;
}

async function inviteInitialAdmins(
  core: AppCore,
  env: Env,
  state: AdminbotState,
  groupId: string,
): Promise<void> {
  const already = new Set(state.invitedInitialAdmins ?? []);
  for (const did of env.initialAdmins) {
    if (already.has(did)) continue;
    if (did === ADMINBOT_DID) continue;
    try {
      console.log(`adminbot: inviting initial admin ${did}`);
      await core.inviteMember(groupId, did, "admin");
      already.add(did);
    } catch (e) {
      console.error(`adminbot: failed to invite ${did}: ${(e as Error).message}`);
      // continue — partial success is fine, operator can re-run
    }
  }
  state.invitedInitialAdmins = [...already];
  saveState(env.statePath, state);
}

async function handleMessage(
  core: AppCore,
  groupId: string,
  event: IncomingEvent,
): Promise<void> {
  // Being added to a group is interesting on its own: app-core auto-accepts
  // the invite (so we're already a full member by the time this fires), and
  // any group we're an admin of becomes an auto-invite target for new
  // server-joiners (see handleAdminEvent). Just log it — no accept needed.
  if (event.kind === "groupInvite") {
    const { groupId: gid, inviterDid } = event.groupInvite;
    console.log(`adminbot: added to group ${gid} by ${inviterDid}`);
    return;
  }
  if (event.kind !== "message") return;
  const msg = event.message;
  if (msg.senderDid === ADMINBOT_DID) return;
  // Slash commands are accepted in #admins and in 1:1 DMs with the bot.
  // Replies always go back through the same channel (group → group send,
  // DM → DM).
  const inAdminsGroup = msg.groupId === groupId;
  const isDm = msg.groupId == null;
  if (!inAdminsGroup && !isDm) return;
  await handleCommand(
    core,
    inAdminsGroup ? { kind: "group", groupId } : { kind: "dm", recipientDid: msg.senderDid },
    msg.senderDid,
    msg.body.trim(),
  );
}

async function handleAdminEvent(
  core: AppCore,
  event: AdminEvent,
): Promise<void> {
  if (event.kind !== "accountJoined") return;
  const { did } = event.accountJoined;
  if (did === ADMINBOT_DID) return;

  // Only humans get auto-invited. Every account registration fires this
  // event — including bots (e.g. testbot spins up a fresh bot account on each
  // "Text Me"). Inviting them would fill groups with bots and fan a Sender
  // Key out to every member on each invite, so skip any bot account.
  let isBot: boolean;
  try {
    isBot = (await core.getAccountInfo(did)).isBot;
  } catch (e) {
    console.error(`adminbot: getAccountInfo(${did}) failed: ${(e as Error).message}; skipping`);
    return;
  }
  if (isBot) {
    console.log(`adminbot: new account ${did} is a bot — not auto-inviting`);
    return;
  }

  await inviteToAdminGroups(core, did);

  // Send a 1:1 welcome DM. Goes over the same sealed-sender channel the
  // GroupContext invite opens, so it works regardless of whether the
  // recipient has accepted any group invite yet.
  try {
    await core.sendDm(
      did,
      "Welcome! You've been added to this server's groups. Type /help to see what I can do.",
    );
    console.log(`adminbot: sent welcome DM to ${did}`);
  } catch (e) {
    console.error(`adminbot: welcome DM to ${did} failed: ${(e as Error).message}`);
  }
}

// Invite a new server-joiner into every group adminbot is currently an admin
// of. The admin check is live (a group's invite policy defaults to admin-only,
// and the bot may only have been added as a plain member) — non-admin groups
// are skipped. #admins is just one such group: adminbot founded it, so it's
// always admin there. Per-group failures are logged and don't abort the rest.
async function inviteToAdminGroups(core: AppCore, did: string): Promise<void> {
  let groupIds: string[];
  try {
    groupIds = await core.listGroups();
  } catch (e) {
    console.error(`adminbot: listGroups failed: ${(e as Error).message}; skipping invites`);
    return;
  }
  for (const gid of groupIds) {
    let summary;
    try {
      summary = await core.fetchGroupState(gid);
    } catch (e) {
      console.error(`adminbot: fetchGroupState(${gid}) failed: ${(e as Error).message}; skipping`);
      continue;
    }
    const me = summary.members.find((m) => m.did === ADMINBOT_DID);
    if (me?.role !== "admin") continue; // not an admin here — leave it alone
    if (summary.members.some((m) => m.did === did)) continue; // already a member
    try {
      console.log(`adminbot: inviting ${did} to ${gid} ("${summary.title}")`);
      await core.inviteMember(gid, did, "member");
    } catch (e) {
      console.error(`adminbot: invite of ${did} to ${gid} failed: ${(e as Error).message}`);
    }
  }
}

async function handleCommand(
  core: AppCore,
  channel: SendTarget,
  senderDid: string,
  body: string,
): Promise<void> {
  if (!body.startsWith("/")) return;
  const [cmd] = body.split(/\s+/, 1);
  switch (cmd) {
    case "/whoami":
      await core.send(channel, `${senderDid} (admin)`);
      break;
    case "/help":
      await core.send(
        channel,
        ["Available commands:", "  /whoami    echo your DID", "  /help      show this help"].join("\n"),
      );
      break;
    default:
      await core.send(channel, `unknown command: ${cmd}. Try /help.`);
  }
}

async function run(): Promise<void> {
  const env = readEnv();
  initLogging(env.logLevel);

  const state: AdminbotState = loadState(env.statePath) ?? {};
  const core = await withRetry("login/register", () => loginOrRegister(env));
  console.log(`adminbot: started (did=${core.did()})`);

  const groupId = await withRetry("ensure #admins group", () =>
    ensureAdminsGroup(core, env, state),
  );
  await inviteInitialAdmins(core, env, state, groupId);

  console.log(`adminbot: listening for events on ${groupId}`);

  const messagesLoop = (async () => {
    for await (const event of core.events()) {
      handleMessage(core, groupId, event).catch((e) => {
        console.error(`adminbot: message handler error: ${(e as Error).message}`);
      });
    }
  })();

  const adminLoop = (async () => {
    for await (const event of core.adminEvents()) {
      handleAdminEvent(core, event).catch((e) => {
        console.error(`adminbot: admin handler error: ${(e as Error).message}`);
      });
    }
  })();

  await Promise.all([messagesLoop, adminLoop]);
}

run().catch((e) => {
  console.error(`adminbot: fatal: ${(e as Error).message}`);
  process.exit(1);
});
