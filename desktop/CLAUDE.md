# Desktop CLAUDE.md

## Platform Parity Rule

**Any feature added or changed on iOS must be implemented on Desktop (Tauri)
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
desktop/                         # Tauri app (this directory)
├── src/                         # Solid frontend
└── src-tauri/                   # Rust backend (Tauri commands, app config)
```

The Tauri Rust backend (`src-tauri/`) is the Desktop equivalent of UniFFI
bindings in `core/crates/app-core/` — a bridge layer, not an app. It links
directly against the `app-core` crate and exposes its methods as Tauri commands.

---

## Tauri Architecture

There is no main/renderer process split. The app has two layers:

**Rust backend** (`src-tauri/src/lib.rs`):
- Links against `app-core` directly — no Node.js intermediary
- Exposes methods as Tauri commands (`#[tauri::command]`)
- Manages WebSocket loops, metadata persistence via `tauri-plugin-store`

**Solid frontend** (`src/`, runs in a WebView sandbox):
- Runs Solid — no direct native access
- Calls Rust via `invoke('command_name', args)`
- Receives push events via Tauri event system

All Rust core calls flow: `Solid → invoke() → src-tauri/src/lib.rs → app-core → Rust`.

---

## Desktop Workflow

```bash
cd desktop && cargo tauri dev    # dev mode with hot reload
cd desktop && cargo tauri build  # package for current platform
```

FFI constraints:
- Tauri commands are async-capable — use `async fn` for Rust calls that block
- WebSocket loops run in the Rust backend via Tauri's async runtime
- Push events to the frontend via `app_handle.emit('event-name', payload)`

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
- [ ] Desktop Solid component created/updated in `desktop/src/views/`
- [ ] Tauri command added to `desktop/src-tauri/src/lib.rs` if new Rust calls needed
- [ ] AppContext updated to match AppState changes
- [ ] New model fields added to both `.swift` and `.ts` types
- [ ] `docs/61-desktop-implementation.md` parity table updated
- [ ] *(macOS only)* Screenshot taken and saved to `docs/screenshots/<screen-name>.png`

## Adding a New FFI Method (checklist)

1. Add Rust method to `core/crates/app-core/src/lib.rs` (`#[uniffi::export]`, sync)
2. `make bindings` — regenerates Swift + Kotlin UniFFI glue
3. Add to `AppCoreProtocol` in `ActnetService.swift` and stub in `MockActnetService.swift`
4. Add to `ActnetService` interface in Kotlin and stub in `MockActnetService.kt`
5. Call from `AppState.swift` via `Task.detached`
6. Call from `AppViewModel.kt` via `withContext(Dispatchers.IO)`
7. Add Tauri command in `desktop/src-tauri/src/lib.rs`, typed wrapper in `DevServerActnetService.ts`, stub in `MockActnetService.ts`

---

## Security constraints

The shell is the only WebView with Tauri command access. Keep these invariants:

- `npm ci` only — never `npm install` in production or CI
- Tauri CSP in `tauri.conf.json`: `default-src 'self'`, no `unsafe-inline`, no `eval`
- `Object.freeze(Object.prototype)` at app startup in `src/index.tsx`
- Strict TypeScript (`strict: true` in `tsconfig.json`, no `any`)
- All message content received as typed data from Tauri commands — never parse raw bytes in the frontend
- Minimal Tauri command surface: only declare commands the shell legitimately needs in `tauri.conf.json` capabilities
