import {
  createContext,
  createSignal,
  onCleanup,
  useContext,
  type JSX,
} from "solid-js";
import { createStore } from "solid-js/store";
import { listen } from "@tauri-apps/api/event";
import type { Account, ServerInfo } from "../models";
import { displayHost } from "../lib/format";
import { ServiceMode } from "../services/AvalancheService";
import type { AppContextValue, AppStore, SessionGuards } from "./types";
import { createServices } from "./createServices";
import { createConversations } from "./createConversations";
import { createMessaging } from "./createMessaging";
import { createGroupsAndSafety } from "./createGroupsAndSafety";
import { createEventLoops } from "./createEventLoops";
import { createAccounts } from "./createAccounts";

const AppContext = createContext<AppContextValue | undefined>(undefined);

// ── Provider ──────────────────────────────────────────────────────────────────

export function AppProvider(props: { children: JSX.Element }) {
  const [store, setStore] = createStore<AppStore>({
    accounts: [],
    isOnboarding: true,
    isAddingAccount: false,
    serviceMode: ServiceMode.DevServer,
    selectedTab: "chats",
    conversations: [],
    messagesByConversation: {},
    reactionsByConversation: {},
    connectionStates: {},
    pendingInviteToken: null,
    serverUrl: "http://localhost:3000",
    // Default on: closing keeps the app alive in the tray so messages keep
    // arriving (matches the Rust-side default in close_to_tray_enabled).
    closeToTray: true,
  });

  // Per-account service resolution (see createServices.ts). Destructured so the
  // hot call sites (`serviceFor(...)`, `onboardingService()`) read unchanged.
  const services = createServices({ store });
  const { onboardingService, serviceFor, registerAccountService } = services;

  // Selected conversation — lifted into context so compose/group/join flows
  // can programmatically open a conversation. ChatsView mirrors this signal.
  const [selectedConversationId, setSelectedConversationId] = createSignal<string | null>(null);

  // Bumps each time a group's metadata changes (incoming GroupMetadataChanged),
  // carrying the affected groupId. ConversationView tracks this to re-check
  // membership for the open group without waiting for a conversation switch (T74).
  const [groupMetaChange, setGroupMetaChange] = createSignal<{ groupId: string; n: number }>({
    groupId: "",
    n: 0,
  });

  // Load-once / lifecycle guards shared across the state modules (see the
  // SessionGuards doc in ./types.ts).
  const guards: SessionGuards = {
    loadedConversations: { value: false },
    loadedMessages: new Set(),
    loadedReactions: new Set(),
    pendingConversations: new Set(),
  };

  // Conversation list, name/bot caches, and deep-link routing (see
  // createConversations.ts). Destructured so call sites read unchanged.
  const conversations = createConversations({
    store,
    setStore,
    serviceFor,
    guards,
    setSelectedConversationId,
  });
  const {
    loadConversationsFromStore,
    reloadConversations,
    findOrCreateDMConversation,
    displayName,
    isBot,
    isDeepLink,
    handleDeepLink,
    accountIdForConversation,
    getServerUrl,
    findOrCreateGroupConversation,
    cachedDisplayName,
    resetCaches,
  } = conversations;

  // Message timelines, optimistic send, read state, and message actions (see
  // createMessaging.ts). Destructured so call sites read unchanged.
  const messaging = createMessaging({
    store,
    setStore,
    serviceFor,
    onboardingService,
    guards,
    accountIdForConversation,
  });
  const {
    sendMessage,
    sendGroupMessage,
    sendMessageWithAttachments,
    uploadAttachment,
    downloadAttachment,
    fetchLinkPreview,
    openExternal,
    loadMessagesFromStore,
    markAllMessagesRead,
    unreadCount,
    reactionsFor,
    loadReactions,
    toggleReaction,
    editMessage,
    loadMessageRevisions,
    deleteMessage,
    retryMessage,
    reloadMessagesIfLoaded,
    clearReactionsForMessage,
  } = messaging;

  // Group create/join/leave and safety/timers (see createGroupsAndSafety.ts).
  const groupsAndSafety = createGroupsAndSafety({
    store,
    setStore,
    serviceFor,
    guards,
    reloadConversations,
    findOrCreateGroupConversation,
    selectedConversationId,
    setSelectedConversationId,
  });
  const {
    createGroupAndOpen,
    joinViaLink,
    leaveGroup,
    acceptRequest,
    deleteRequest,
    reportAndBlock,
    blockContact,
    unblockContact,
    listBlocked,
    getConversationTimer,
    setConversationTimer,
  } = groupsAndSafety;

  // Per-account event + connection loops, inbound-event handlers, native
  // notifications, and the aggregate connection state (see createEventLoops.ts).
  // Registers the loops' onCleanup; must be called synchronously here.
  const eventLoops = createEventLoops({
    store,
    setStore,
    serviceFor,
    guards,
    reloadConversations,
    getServerUrl,
    cachedDisplayName,
    reloadMessagesIfLoaded,
    clearReactionsForMessage,
    selectedConversationId,
    setGroupMetaChange,
  });
  const {
    startPollingFor,
    stopPollingFor,
    stopPolling,
    aggregateConnectionState,
    reconnectNow,
  } = eventLoops;

  // Account lifecycle, avalanche.json persistence, and settings (see
  // createAccounts.ts). Destructured so call sites read unchanged.
  const accounts = createAccounts({
    store,
    setStore,
    services,
    guards,
    loadConversationsFromStore,
    reloadConversations,
    resetCaches,
    startPollingFor,
    stopPollingFor,
    stopPolling,
    setSelectedConversationId,
  });
  const {
    createAccount,
    restoreAccounts,
    logout,
    setServerUrl,
    setCloseToTray,
    joinServer,
    setAccountDisplayName,
    leaveServer,
    deleteIdentity,
    hasRecovery,
    generateRecoveryPhrase,
    recoverFromPhrase,
    validateInvite,
    startAddAccount,
    cancelAddAccount,
    enterApp,
    addPersistedAccount,
  } = accounts;


  // ── Deep-link listener ────────────────────────────────────────────────────
  // Single consumer of `avalanche-deeplink` (emitted by the Rust deep-link
  // plugin, see src-tauri/src/lib.rs). OnboardingFlow's pendingInviteToken
  // effect still drives onboarding navigation for invite tokens.
  let deeplinkUnlisten: (() => void) | undefined;
  listen<string>("avalanche-deeplink", (ev) => handleDeepLink(ev.payload))
    .then((un) => { deeplinkUnlisten = un; })
    .catch(() => { /* Tauri event API unavailable (browser/test) */ });
  onCleanup(() => deeplinkUnlisten?.());


  // ── Device linking (T71) ────────────────────────────────────────────────────
  // Poll cadence mirrors iOS AppState (1s interval, 180s deadline). The TS layer
  // drives the loop so it stays cancellable, per docs/04 §4.2 (no long-lived,
  // uncancellable FFI call).
  const LINK_POLL_MS = 1000;
  const LINK_TIMEOUT_MS = 180_000;

  // New device, show mode: generate this device's pairing code to display.
  // Account-less (no account yet) → onboarding service.
  async function deviceLinkShowCode(): Promise<string> {
    return onboardingService().deviceLinkCreatePairing(null);
  }

  // New device, paste mode: accept the existing device's pairing code.
  async function deviceLinkEnterCode(code: string): Promise<void> {
    await onboardingService().deviceLinkAcceptPairing(code);
  }

  // New device: poll until the provisioning bundle arrives, then install the
  // linked account and enter the app — the same completion as createAccount
  // (account row + persisted record + enterApp). The home server is learned
  // from the bundle (homeServer()), not from user input.
  async function deviceLinkComplete(): Promise<void> {
    const dbPath = `account-${Math.random().toString(36).slice(2, 10)}.db`;
    const deadline = Date.now() + LINK_TIMEOUT_MS;
    for (;;) {
      const result = await onboardingService().deviceLinkAwaitStep(dbPath, "dev-placeholder-key");
      if (result) {
        // The backend has installed the linked core keyed by this DID; bind its
        // service so homeServer() (per-account) and the loops route correctly.
        registerAccountService(result.did);
        const serverUrl = await serviceFor(result.did).homeServer();
        const serverInfo: ServerInfo = {
          id: serverUrl,
          name: serverUrl,
          url: serverUrl,
          displayHost: displayHost(serverUrl, serverUrl),
        };
        const account: Account = {
          id: result.did,
          displayName: result.displayName,
          avatarData: null,
          servers: [serverInfo],
        };
        if (!store.accounts.some((a) => a.id === result.did)) {
          setStore("accounts", (prev) => [...prev, account]);
        }
        await addPersistedAccount({
          did: result.did,
          displayName: account.displayName,
          dbPath,
          servers: [{ id: serverUrl, name: serverUrl, url: serverUrl }],
        });
        enterApp();
        return;
      }
      if (Date.now() >= deadline) {
        await onboardingService().deviceLinkReset().catch(() => {});
        throw new Error("Device link timed out. Please try again.");
      }
      await new Promise((r) => setTimeout(r, LINK_POLL_MS));
    }
  }

  // New device: abandon an in-progress pairing (view teardown / cancel).
  async function deviceLinkCancel(): Promise<void> {
    await onboardingService().deviceLinkReset().catch(() => {});
  }

  // Existing device, show mode: generate this device's pairing code to display.
  // Per-account: the user is linking a new device to a specific identity.
  async function linkShowCode(accountId: string): Promise<string> {
    return serviceFor(accountId).linkCreatePairing(null);
  }

  // Existing device, paste mode: accept the new device's pairing code.
  async function linkEnterCode(accountId: string, code: string): Promise<void> {
    await serviceFor(accountId).linkAcceptPairing(code);
  }

  // Existing device: poll until the provisioning bundle has been sealed + sent.
  async function linkSendBundle(accountId: string): Promise<void> {
    const deadline = Date.now() + LINK_TIMEOUT_MS;
    for (;;) {
      const done = await serviceFor(accountId).linkSendBundleStep();
      if (done) return;
      if (Date.now() >= deadline) {
        throw new Error("Device link timed out. Please try again.");
      }
      await new Promise((r) => setTimeout(r, LINK_POLL_MS));
    }
  }


  const ctx: AppContextValue = {
    store,
    service: onboardingService,
    serviceFor,
    setSelectedTab: (tab) => setStore("selectedTab", tab),
    createAccount,
    restoreAccounts,
    logout,
    serverUrl: () => store.serverUrl,
    setServerUrl,
    closeToTray: () => store.closeToTray,
    setCloseToTray,
    reconnectNow,
    joinServer,
    sendMessage,
    sendGroupMessage,
    sendMessageWithAttachments,
    uploadAttachment,
    downloadAttachment,
    fetchLinkPreview,
    openExternal,
    loadConversationsFromStore,
    loadMessagesFromStore,
    markAllMessagesRead,
    findOrCreateDMConversation,
    aggregateConnectionState,
    unreadCount,
    displayName,
    isBot,
    setPendingInviteToken: (token) => setStore("pendingInviteToken", token),
    validateInvite,
    selectedConversationId,
    selectConversation: (id) => setSelectedConversationId(id),
    reloadConversations,
    groupMetaChange,
    reactionsFor,
    loadReactions,
    toggleReaction,
    editMessage,
    loadMessageRevisions,
    deleteMessage,
    retryMessage,
    createGroupAndOpen,
    joinViaLink,
    leaveGroup,
    acceptRequest,
    deleteRequest,
    reportAndBlock,
    blockContact,
    unblockContact,
    listBlocked,
    getConversationTimer,
    setConversationTimer,
    setAccountDisplayName,
    leaveServer,
    deleteIdentity,
    hasRecovery,
    generateRecoveryPhrase,
    recoverFromPhrase,
    startAddAccount,
    cancelAddAccount,
    deviceLinkShowCode,
    deviceLinkEnterCode,
    deviceLinkComplete,
    deviceLinkCancel,
    linkShowCode,
    linkEnterCode,
    linkSendBundle,
    isDeepLink,
    handleDeepLink,
  };

  return (
    <AppContext.Provider value={ctx}>
      {props.children}
    </AppContext.Provider>
  );
}

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used inside AppProvider");
  return ctx;
}
