# Desktop Implementation Plan

iOS is the reference implementation. This document tracks what needs to be built
to reach functional parity on Desktop (Windows, macOS, Linux) and how to maintain
it going forward.

See `desktop/CLAUDE.md` for the parity rule and Electron workflow.

---

## Tech Stack

| Concern | Desktop | iOS equivalent |
|---|---|---|
| Language | TypeScript | Swift |
| UI framework | React + Electron | SwiftUI |
| State management | React Context + useReducer (or Zustand) | ObservableObject + @Published |
| Navigation | React Router | NavigationStack |
| Async | async/await + Promises | async/await + Task |
| Camera (QR) | Electron `desktopCapturer` or native dialog | AVFoundation + VisionKit |
| WebView | Electron `<webview>` tag | WKWebView |
| Rust bridge | napi-rs Node.js bindings (in `core/crates/app-core-node/`) | UniFFI Swift bindings |
| Persistence (metadata) | `electron-store` (JSON file) | UserDefaults (JSON) |
| Local crypto DB | SQLCipher via napi-rs Rust core | SQLCipher via UniFFI Rust core |

---

## Project Structure

```
desktop/
├── src/
│   ├── main/                    # Electron main process (Node.js)
│   │   ├── index.ts             # app entry, BrowserWindow setup
│   │   ├── ipc.ts               # IPC handlers — bridge between renderer and Rust
│   │   └── store.ts             # electron-store for metadata persistence
│   ├── renderer/                # Electron renderer process (React)
│   │   ├── index.tsx
│   │   ├── App.tsx
│   │   ├── models/
│   │   │   ├── Account.ts
│   │   │   ├── Conversation.ts
│   │   │   ├── Message.ts
│   │   │   ├── InviteToken.ts
│   │   │   └── ProjectInfo.ts
│   │   ├── state/
│   │   │   └── AppContext.tsx    # mirrors iOS AppState
│   │   ├── services/
│   │   │   ├── ActnetService.ts
│   │   │   ├── MockActnetService.ts
│   │   │   └── DevServerActnetService.ts
│   │   └── views/
│   │       ├── onboarding/
│   │       │   ├── SplashView.tsx
│   │       │   ├── QRScannerView.tsx
│   │       │   ├── InviteLinkEntryView.tsx
│   │       │   ├── IdentityPickerView.tsx
│   │       │   ├── JoiningServerView.tsx
│   │       │   └── NewAccountView.tsx
│   │       ├── chats/
│   │       │   ├── ChatsView.tsx
│   │       │   ├── ConversationRow.tsx
│   │       │   ├── ConversationView.tsx
│   │       │   ├── MessageBubble.tsx
│   │       │   ├── ComposeMessageView.tsx
│   │       │   └── RecoveryKeyBanner.tsx
│   │       ├── calls/
│   │       │   └── CallsView.tsx
│   │       ├── network/
│   │       │   ├── NetworkView.tsx
│   │       │   └── ProjectWebView.tsx
│   │       └── common/
│   │           ├── AccountAvatar.tsx
│   │           ├── DevSettingsView.tsx
│   │           └── MainLayout.tsx   # sidebar nav (desktop adapts tabs → sidebar)
├── package.json
├── tsconfig.json
├── electron-builder.config.ts   # packaging for Win/Mac/Linux
└── vite.config.ts               # bundler for renderer
```

### Main process vs. renderer

Electron has two JavaScript contexts:

- **Main process** (Node.js): can call native code. This is where napi-rs Rust bindings are called. It exposes results to the renderer via IPC.
- **Renderer process** (browser): runs React. Cannot call native code directly — communicates with main via `ipcRenderer.invoke()`.

All Rust core calls go: `React component → ipcRenderer.invoke() → main process ipc.ts → napi-rs → Rust`.

---

## Desktop UX Adaptation

The iOS app uses a bottom tab bar (Calls / Chats / Network). On desktop, tabs become a **left sidebar** — the standard pattern for desktop messaging apps (Signal Desktop, Slack, Discord all do this). The three sections are identical; only the navigation chrome differs.

Everything else — conversation list, message bubbles, delivery indicators, network view, project webview — maps 1:1 from iOS with no conceptual change.

---

## Parity Map

### App Shell

| iOS | Desktop | Status |
|---|---|---|
| `ActnetApp.swift` | `main/index.ts` + `renderer/App.tsx` | `[ ]` |
| `RootView.swift` | Root router in `App.tsx` | `[ ]` |
| `AppState.swift` | `AppContext.tsx` | `[ ]` |

### Models

| iOS | Desktop | Status |
|---|---|---|
| `Account.swift` | `Account.ts` | `[ ]` |
| `Conversation.swift` | `Conversation.ts` | `[ ]` |
| `Message.swift` | `Message.ts` | `[ ]` |
| `InviteToken.swift` | `InviteToken.ts` | `[ ]` |
| `ProjectInfo.swift` | `ProjectInfo.ts` | `[ ]` |

### Services

| iOS | Desktop | Status |
|---|---|---|
| `ActnetService.swift` protocol | `ActnetService.ts` interface | `[ ]` |
| `MockActnetService.swift` | `MockActnetService.ts` | `[ ]` |
| `DevServerActnetService.swift` | `DevServerActnetService.ts` | `[ ]` |
| UniFFI `AppCore` | napi-rs `AppCore` (from `core/crates/app-core-node/`) via IPC | `[ ]` |

### Onboarding

| iOS | Desktop | Status |
|---|---|---|
| `SplashView.swift` | `SplashView.tsx` | `[ ]` |
| `QRScannerView.swift` | `QRScannerView.tsx` (file upload or camera capture) | `[ ]` |
| `InviteLinkEntryView.swift` | `InviteLinkEntryView.tsx` | `[ ]` |
| `IdentityPickerView.swift` | `IdentityPickerView.tsx` | `[ ]` |
| `JoiningServerView.swift` | `JoiningServerView.tsx` | `[ ]` |
| `NewAccountView.swift` | `NewAccountView.tsx` | `[ ]` |

### Navigation

| iOS | Desktop | Status |
|---|---|---|
| `MainTabView.swift` (bottom tabs) | `MainLayout.tsx` (left sidebar) | `[ ]` |

### Chats

| iOS | Desktop | Status |
|---|---|---|
| `ChatsView.swift` | `ChatsView.tsx` | `[ ]` |
| `ConversationRow.swift` | `ConversationRow.tsx` | `[ ]` |
| `ConversationView.swift` | `ConversationView.tsx` | `[ ]` |
| `MessageBubble.swift` | `MessageBubble.tsx` | `[ ]` |
| `ComposeMessageView.swift` | `ComposeMessageView.tsx` | `[ ]` |
| `RecoveryKeyBanner.swift` | `RecoveryKeyBanner.tsx` | `[ ]` |

### Calls

| iOS | Desktop | Status |
|---|---|---|
| `CallsView.swift` | `CallsView.tsx` | `[ ]` |

### Network

| iOS | Desktop | Status |
|---|---|---|
| `NetworkView.swift` | `NetworkView.tsx` | `[ ]` |
| `ProjectWebView.swift` | `ProjectWebView.tsx` | `[ ]` |

### Common

| iOS | Desktop | Status |
|---|---|---|
| `AccountAvatar.swift` | `AccountAvatar.tsx` | `[ ]` |
| `DevSettingsView.swift` | `DevSettingsView.tsx` | `[ ]` |

### State Behaviors (AppContext mirrors AppState)

| Behavior | Status |
|---|---|
| Account restoration on launch | `[ ]` |
| `createAccount(serverUrl, serverName, displayName)` | `[ ]` |
| `joinServer(serverUrl, serverName, existingAccountId)` | `[ ]` |
| `switchMode(mode)` | `[ ]` |
| `sendMessage(...)` — optimistic + core via IPC | `[ ]` |
| `markAllMessagesRead(conversationId, accountId)` | `[ ]` |
| `loadMessagesFromStore(conversationId, accountId)` | `[ ]` |
| `findOrCreateDMConversation(recipientDid, accountId)` | `[ ]` |
| WebSocket loop per account (Node.js, reconnect on error) | `[ ]` |
| `handleIncomingMessage()` | `[ ]` |
| `applyDeliveryStatusUpdates()` | `[ ]` |
| `fetchProjects(serverUrl)` | `[ ]` |
| `requestProjectToken(accountId, projectUrl)` | `[ ]` |
| Conversation persistence (electron-store) | `[ ]` |
| `unreadCount(for:)` | `[ ]` |

---

## Implementation Phases

### Phase 1 — Electron project + Rust bridge

- Create `desktop/` with Electron + Vite + React + TypeScript
- Wire napi-rs bindings from `core/crates/app-core-node/` into the main process
- IPC skeleton: `ipc.ts` with handlers for each Rust core method
- `electron-builder` config for Win/Mac/Linux targets
- `make desktop` Makefile target

**Done when:** Electron window opens; can call a Rust method from the renderer via IPC and get a result.

### Phase 2 — Models + AppContext

- TypeScript interfaces for all models
- `AppContext.tsx` with all state and methods
- `MockActnetService.ts` with seeded conversations
- `DevServerActnetService.ts` calling Rust via IPC

**Done when:** mock mode works end-to-end in the renderer with no Rust calls.

### Phase 3 — Navigation skeleton

- React Router setup
- `MainLayout.tsx`: left sidebar with Calls / Chats / Network links
- Root routing: onboarding flow vs. main layout

### Phase 4 — Onboarding screens

- `SplashView.tsx`: logo, scan QR (file input fallback on desktop), enter link, dev settings
- `InviteLinkEntryView.tsx`: text field, parse `actnet://` or `https://…/invite/…`
- `IdentityPickerView.tsx`, `JoiningServerView.tsx`, `NewAccountView.tsx`
- QR scanner note: desktop doesn't have a camera in the same sense; accept pasted link or image file upload as the primary path, with optional webcam capture as secondary

### Phase 5 — Chats

- Full chats tab matching iOS: conversation list, conversation view, message bubbles, compose, delivery indicators, unread counts, mark read

### Phase 6 — Network + Calls tabs

- `NetworkView.tsx`: server/project list
- `ProjectWebView.tsx`: Electron `<webview>` with auth token
- `CallsView.tsx`: placeholder

### Phase 7 — Dev settings + polish

- `DevSettingsView.tsx`: mode selector, server URL, counts
- System tray integration (optional: keep app running in background)
- Native notifications via Electron `Notification` API

---

## Open Questions

1. **napi-rs bindings status.** The napi-rs bindings crate `core/crates/app-core-node/` needs to be created alongside the Electron app.
2. **QR scanning on desktop.** Primary path is paste-a-link. Webcam QR scanning is a nice-to-have.
3. **WebSocket in main process.** The WS loop should run in the main process (Node.js) and push events to the renderer via IPC — same as how native mobile runs it off the main thread.
