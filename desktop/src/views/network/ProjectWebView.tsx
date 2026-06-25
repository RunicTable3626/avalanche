import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { ProjectInfoFfi } from "../../services/AvalancheService";

/**
 * Open a project URL with an auth token in a Tauri WebviewWindow modal.
 * Returns true if the window was created successfully.
 */
export async function openProjectWindow(
  project: ProjectInfoFfi,
  token: string
): Promise<boolean> {
  const label = `project-${crypto.randomUUID().slice(0, 8)}`;
  // Pass the token via hash fragment rather than query string so it is
  // never sent in Referer headers or visible in server access logs.
  // The project page reads it from `window.location.hash`.
  const url = `${project.url}#token=${encodeURIComponent(token)}`;

  // TODO(Day 4): intercept navigation to go.theavalanche.net and close the
  // modal / emit a deeplink event.
  // TODO(security, Day 4): the webview inherits full Tauri IPC capabilities
  // because its ephemeral label can't be pre-configured in a capabilities
  // file. Restrict via a dedicated capability or CSP when hardening (T38).

  return new Promise((resolve) => {
    try {
      const webview = new WebviewWindow(label, {
        url,
        title: project.name,
        width: 900,
        height: 700,
        resizable: true,
      });

      webview.once("tauri://created", () => resolve(true));
      webview.once("tauri://error", (e) => {
        console.error("Project webview creation error:", e);
        resolve(false);
      });
    } catch (e) {
      console.error("Failed to open project webview:", e);
      resolve(false);
    }
  });
}
