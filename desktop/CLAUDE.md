# Desktop CLAUDE.md

## Platform Parity Rule

**Any feature added or changed on iOS must be implemented on Desktop (Electron)
in the same session. Any feature added or changed on Desktop must be implemented
on iOS.**

iOS is the reference implementation. The Desktop app must match it
feature-for-feature. When in doubt about behavior, check the iOS source.

The same rule applies across all three platforms — see `mobile/CLAUDE.md` for
iOS/Android and the root `CLAUDE.md` for the overall parity rule.

Use `docs/61-desktop-implementation.md` as the parity tracking document — update
the `[ ]` / `[x]` checkboxes as each component is completed.

---

## Repository layout

```
desktop/                         # Electron app (this directory)
core/crates/app-core-node/       # napi-rs Rust bindings for Node.js/Electron
projects/adminbot/               # Admin bot (separate Project, uses same bindings)
```

The napi-rs bindings (`core/crates/app-core-node/`) are the Desktop equivalent
of the UniFFI bindings in `core/crates/app-core/` — a bridge layer, not an app.
The desktop app depends on them the same way the mobile apps depend on UniFFI.

---

## Electron Architecture

Electron has two JavaScript contexts:

**Main process** (`src/main/`, runs in Node.js):
- Calls napi-rs Rust bindings directly
- Manages BrowserWindow, IPC handlers, WebSocket loops
- Persists metadata via `electron-store`

**Renderer process** (`src/renderer/`, runs in a browser sandbox):
- Runs React — no direct native access
- Calls Rust indirectly: `ipcRenderer.invoke('method-name', ...args)`
- Main process handles the call and returns the result

All Rust core calls flow: `React → ipcRenderer.invoke() → ipc.ts → napi-rs → Rust`.

---

## Desktop Workflow

```bash
make desktop-bindings   # build napi-rs .node file from core/crates/app-core-node/
cd desktop && npm run dev        # start Electron in dev mode (hot reload)
cd desktop && npm run build      # package for current platform
cd desktop && npm run build:all  # package for Win + Mac + Linux
```

FFI constraints:
- napi-rs calls must run in the **main process** (Node.js), never in the renderer
- WebSocket loops run in the main process and push events to the renderer via
  `mainWindow.webContents.send()`
- All blocking Rust calls should use async napi-rs exports to avoid blocking
  the main process event loop

---

## UX Adaptation: Tabs → Sidebar

iOS uses a bottom tab bar (Calls / Chats / Network). Desktop uses a **left
sidebar** — the standard for desktop messaging apps (Signal Desktop, Slack,
Discord). The three sections are identical in content; only the navigation
chrome differs. This is the one intentional UX divergence from iOS.

---

## Visual Reference: Screenshots

`docs/screenshots/` contains iOS simulator screenshots organized by screen name
(e.g. `splash.png`, `chats-list.png`, `conversation.png`). When implementing a
screen on Desktop, use the matching screenshot as a visual reference if it
exists. If it doesn't exist, derive the layout from the iOS source alone —
screenshots are optional, not required.

Screenshots are only capturable on macOS with the iOS simulator. Contributors on
Windows or Linux skip this step entirely.

---

## Adding a New Screen (checklist)

Before closing any branch that adds or changes Desktop UI:

- [ ] iOS SwiftUI view created/updated in `mobile/ios/`
- [ ] Desktop React component created/updated in `desktop/src/renderer/views/`
- [ ] IPC handler added to `desktop/src/main/ipc.ts` if new Rust calls needed
- [ ] AppContext updated to match AppState changes
- [ ] New model fields added to both `.swift` and `.ts` types
- [ ] `docs/61-desktop-implementation.md` parity table updated
- [ ] *(macOS only)* Screenshot taken and saved to `docs/screenshots/<screen-name>.png`

## Adding a New FFI Method (checklist)

1. Add Rust method to `core/crates/app-core/src/lib.rs` (`#[uniffi::export]`, sync)
2. Add napi-rs export to `core/crates/app-core-node/src/lib.rs`
3. `make bindings` — regenerates Swift + Kotlin UniFFI glue
4. `make desktop-bindings` — rebuilds napi-rs `.node` file
5. Add IPC handler in `desktop/src/main/ipc.ts`
6. Add typed wrapper in `desktop/src/renderer/services/DevServerActnetService.ts`
7. Stub in `MockActnetService.ts`
8. Update iOS and Android simultaneously (see `mobile/CLAUDE.md`)
