# Desktop Implementation Plan

iOS is the reference implementation. This document tracks what needs to be built
to reach functional parity on Desktop (Windows, macOS, Linux) and how to maintain
it going forward.

See `desktop/CLAUDE.md` for the parity rule and Tauri workflow.

---

## Tech Stack Decision

Tauri was chosen over Electron. The decision was not obvious — Electron has real advantages — so the full reasoning is recorded here.

### Why Tauri

**Security patch ownership.** Electron bundles a specific Chromium version. When a Chromium CVE drops, protection requires: (1) Electron releases an update with the patched Chromium, and (2) users install it. Tauri uses the OS system webview — WebView2 on Windows, WebKit on macOS, webkit2gtk on Linux — which is patched automatically by the OS vendor independent of the app. For a security-sensitive app targeting activist groups who may not keep software updated, offloading the rendering engine patch cycle to the OS is a meaningful reduction in ongoing maintenance burden and a better security posture in practice.

**Architecture consistency.** Tauri's Rust backend with no Node.js intermediary matches the iOS/Android shell pattern exactly: the same `app-core` crate, bridged by a thin platform layer, with the frontend calling into it. Electron would require a separate napi-rs bindings crate, a Node.js main process, and an IPC layer between the renderer and native code — an extra moving part that doesn't exist on any other platform.

**Resource footprint.** Tauri's memory baseline is ~30MB vs ~150–200MB for Electron; binary size is ~5MB vs ~150MB. This matters for two reasons. First, activist groups realistically run on donated or cheap hardware — old laptops, Raspberry Pis. Second, cheap replaceable hardware is itself a security property: a $35 device you can destroy or abandon lowers the cost of physical seizure. (Note: the homeserver's Postgres requirement is the harder constraint for Pi-class hardware; SQLite homeserver would need to land before the desktop shell choice becomes the binding constraint.)

### Why not Electron

The main argument for Electron is that Signal Desktop uses it — a directly copyable reference implementation with a well-hardened configuration (context isolation on, Node integration off in renderer, strict CSP, sandboxed webview). That configuration closes most of Electron's structural security gaps and is genuinely production-grade.

However, hardened Electron and Tauri are comparable in practice on security — the advantage Tauri has is structural (deny-by-default capability system, no Node intermediary) while hardened Electron achieves similar results through discipline. The security patch ownership difference remains regardless of hardening.

### ProjectWebView

The main concern raised about Tauri was `ProjectWebView` — embedding a project's web UI inside the app shell, which requires auth token injection and navigation interception. This turned out to be a non-issue for two reasons.

First, the iOS reference (`mobile/ios/Actnet/Sources/Views/Network/ProjectWebView.swift`) is a plain modal sheet with no JavaScript bridge to native code — no `WKScriptMessageHandler`, no injected scripts, no auth token injection. It loads a project URL in a sandboxed `WKWebView` and intercepts navigations to `go.theavalanche.net` as deep links. This maps directly to a Tauri `WebviewWindow` modal with a `navigation_handler`. No embedded-webview concern applies.

Second, the Tauri APIs that were blockers at the time of initial evaluation have since shipped: `Webview::set_cookie()` landed in Tauri 2.8.0 (mid-2025, cross-platform), and navigation interception via `navigation_handler` was already available. The concern about embedding a webview as an inline positioned element within the React layout (tauri #13311, closed "not planned") is irrelevant because our implementation is a modal window, not an inline element.

### WebKit inconsistency

Tauri uses three different rendering engines across platforms, which raises a cross-platform consistency concern. In practice, for a messaging app UI — flexbox layouts, scrolling message lists, input fields, CSS animations — the engines are consistent enough that the QA burden is minor. Inconsistencies are at the margins (Wayland compositing, transparency, iframe event routing) and are unlikely to affect core messaging UI.

The more concrete Linux concern is webkit2gtk stability: a Tauri maintainer has flagged quality issues in webkit2gtk (discussion #8524). The escape hatch is `tauri-apps/cef-rs` — an experimental Chromium renderer for Linux that is actively maintained — which would restore uniform rendering on Linux at the cost of a larger binary. Linux requires webkit2gtk-4.1 (Ubuntu 22.04+, Debian 12+, Pi OS Bookworm); older distros are unsupported.

---

## Tech Stack

| Concern | Desktop | iOS equivalent |
|---|---|---|
| Language | TypeScript | Swift |
| UI framework | React + Tauri | SwiftUI |
| State management | React Context + useReducer (or Zustand) | ObservableObject + @Published |
| Navigation | React Router | NavigationStack |
| Async | async/await + Promises | async/await + Task |
| Camera (QR) | Tauri plugin or native dialog | AVFoundation + VisionKit |
| WebView | Tauri `WebviewWindow` (modal) | WKWebView |
| Rust bridge | Tauri commands (Rust backend in `src-tauri/`) | UniFFI Swift bindings |
| Persistence (metadata) | `tauri-plugin-store` (JSON file) | UserDefaults (JSON) |
| Local crypto DB | SQLCipher via Tauri Rust core | SQLCipher via UniFFI Rust core |

---

## Project Structure

```
desktop/
├── src/                             # React frontend
│   ├── index.tsx
│   ├── App.tsx
│   ├── models/
│   │   ├── Account.ts
│   │   ├── Conversation.ts
│   │   ├── Message.ts
│   │   ├── InviteToken.ts
│   │   └── ProjectInfo.ts
│   ├── state/
│   │   └── AppContext.tsx           # mirrors iOS AppState
│   ├── services/
│   │   ├── ActnetService.ts
│   │   ├── MockActnetService.ts
│   │   └── DevServerActnetService.ts
│   └── views/
│       ├── onboarding/
│       │   ├── SplashView.tsx
│       │   ├── QRScannerView.tsx
│       │   ├── InviteLinkEntryView.tsx
│       │   ├── IdentityPickerView.tsx
│       │   ├── JoiningServerView.tsx
│       │   └── NewAccountView.tsx
│       ├── chats/
│       │   ├── ChatsView.tsx
│       │   ├── ConversationRow.tsx
│       │   ├── ConversationView.tsx
│       │   ├── MessageBubble.tsx
│       │   ├── ComposeMessageView.tsx
│       │   └── RecoveryKeyBanner.tsx
│       ├── calls/
│       │   └── CallsView.tsx
│       ├── network/
│       │   ├── NetworkView.tsx
│       │   └── ProjectWebView.tsx
│       └── common/
│           ├── AccountAvatar.tsx
│           ├── DevSettingsView.tsx
│           └── MainLayout.tsx       # sidebar nav (desktop adapts tabs → sidebar)
├── src-tauri/
│   ├── src/
│   │   └── lib.rs                   # Tauri commands — bridge between React and Rust core
│   ├── Cargo.toml
│   └── tauri.conf.json              # app config, window setup, capability declarations
├── package.json
├── tsconfig.json
└── vite.config.ts                   # bundler for frontend
```

### Tauri architecture

There is no main/renderer process split. The app has two layers:

- **Rust backend** (`src-tauri/src/lib.rs`): links against `app-core` directly — no Node.js intermediary. Exposes methods as Tauri commands (`#[tauri::command]`). Manages WebSocket loops and metadata persistence.
- **React frontend** (`src/`): runs in a WebView sandbox. Calls Rust via `invoke('command_name', args)`. Receives push events via the Tauri event system.

All Rust core calls go: `React component → invoke() → src-tauri/src/lib.rs → app-core → Rust`.

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
| UniFFI `AppCore` | Tauri commands (from `src-tauri/src/lib.rs`) via `invoke()` | `[ ]` |

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
| Conversation persistence (tauri-plugin-store) | `[ ]` |
| `unreadCount(for:)` | `[ ]` |

---

## Implementation Phases

### Phase 1 — Tauri project + Rust bridge

- Create `desktop/` with Tauri + Vite + React + TypeScript
- Wire `app-core` crate into `src-tauri/src/lib.rs` as Tauri commands
- `tauri.conf.json` config for Win/Mac/Linux targets
- `make desktop` Makefile target

**Done when:** Tauri window opens; can call a Rust command from the frontend via `invoke()` and get a result.

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
- `ProjectWebView.tsx`: opens project URL in a Tauri `WebviewWindow` modal; intercepts deep link navigations back to the app (mirrors `ProjectWebView.swift`)
- `CallsView.tsx`: placeholder

### Phase 7 — Dev settings + polish

- `DevSettingsView.tsx`: mode selector, server URL, counts
- System tray integration (optional: keep app running in background)
- Native notifications via `tauri-plugin-notification`

---

## Open Questions

1. **Tauri commands structure.** `src-tauri/src/lib.rs` will grow as commands are added — may want to split into submodules by domain (auth, messages, projects) once it reaches meaningful size.
2. **QR scanning on desktop.** Primary path is paste-a-link. Webcam QR scanning is a nice-to-have.
3. **WebSocket in Rust backend.** The WS loop runs in `src-tauri` and pushes events to the frontend via `app_handle.emit()` — same pattern as native mobile running it off the main thread.
4. **webkit2gtk on constrained Linux.** Requires webkit2gtk-4.1 (Ubuntu 22.04+, Debian 12+, Pi OS Bookworm). Older distros unsupported. If webkit2gtk stability becomes a blocker, `tauri-apps/cef-rs` (experimental Chromium renderer for Linux) is the fallback.
