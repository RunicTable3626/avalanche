# Screenshots

iOS simulator screenshots used as visual references when implementing Android
and Desktop equivalents. Optional — contributors on Windows or Linux skip this.

## How to add screenshots

1. `make ios` — build and launch the iOS simulator
2. Navigate to the screen
3. Take a screenshot (Cmd+S in Simulator, or File → Save Screen)
4. Save as `docs/screenshots/<screen-name>.png`

## Naming convention

| Screen | Filename |
|---|---|
| Splash / onboarding entry | `splash.png` |
| QR scanner | `qr-scanner.png` |
| Invite link entry | `invite-link-entry.png` |
| Identity picker | `identity-picker.png` |
| Joining server | `joining-server.png` |
| New account | `new-account.png` |
| Chats list | `chats-list.png` |
| Conversation | `conversation.png` |
| Message bubble (sent/delivered/read states) | `message-bubbles.png` |
| Compose new DM | `compose.png` |
| Recovery key banner | `recovery-key-banner.png` |
| Network tab | `network.png` |
| Project webview | `project-webview.png` |
| Calls tab | `calls.png` |
| Dev settings | `dev-settings.png` |

## When to update

Update a screenshot whenever the corresponding iOS screen changes visually.
Stale screenshots are worse than no screenshots — if you change an iOS screen
and can't update the screenshot (Windows/Linux), add a comment to the PR so a
macOS contributor can take a fresh one.
