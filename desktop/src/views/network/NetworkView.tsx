import { createMemo, createResource, For, Show, createSignal } from "solid-js";
import { useApp } from "../../state/AppContext";
import type { ProjectInfoFfi } from "../../services/AvalancheService";
import type { ServerInfo } from "../../models";
import { openProjectWindow } from "./ProjectWebView";
import "./NetworkView.css";

export default function NetworkView() {
  const { store, service } = useApp();

  // Deduplicate servers across all accounts.
  const allServers = createMemo(() => {
    const seen = new Map<string, ServerInfo>();
    for (const account of store.accounts) {
      for (const srv of account.servers) {
        seen.set(srv.id, srv);
      }
    }
    return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name));
  });

  // Fetch projects once per session.
  const [projects, { refetch }] = createResource<ProjectInfoFfi[]>(
    () => service().fetchProjects(),
    { initialValue: [] }
  );

  const [openingUrl, setOpeningUrl] = createSignal<string | null>(null);
  const [openError, setOpenError] = createSignal<string | null>(null);

  async function handleOpen(project: ProjectInfoFfi) {
    setOpenError(null);
    setOpeningUrl(project.url);
    try {
      const token = await service().requestProjectToken(project.url);
      const ok = await openProjectWindow(project, token);
      if (!ok) {
        setOpenError("Could not open project window. Check that Tauri is running.");
      }
    } catch (e) {
      console.error("Failed to open project:", e);
      setOpenError(`Failed to open project: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setOpeningUrl(null);
    }
  }

  return (
    <div class="network-view">
      <h2>Network</h2>
      <Show
        when={allServers().length > 0}
        fallback={
          <div class="empty-state">No servers. Join a server to get started.</div>
        }
      >
        <div class="server-section">
          <h3 class="server-name">Servers</h3>
          <For each={allServers()}>
            {(server) => (
              <div class="server-row">
                <span class="server-url">{server.url}</span>
                <span class="server-host">{server.displayHost}</span>
              </div>
            )}
          </For>
        </div>

        <div class="server-section">
          <h3 class="server-name">Projects</h3>
          <Show when={openError()}>
            <div class="error-state">{openError()}</div>
          </Show>
          <Show when={projects.error}>
            <div class="error-state">
              Failed to load projects.{" "}
              <button class="btn-link" onClick={() => refetch()}>
                Retry
              </button>
            </div>
          </Show>
          <Show
            when={!projects.loading}
            fallback={<div class="loading-state">Loading projects…</div>}
          >
            <Show
              when={projects().length > 0}
              fallback={<p class="no-projects">No projects</p>}
            >
              <div class="project-list">
                <For each={projects()}>
                  {(project) => (
                    <div class="project-card">
                      <div class="project-info">
                        <span class="project-name">{project.name}</span>
                        <span class="project-desc">{project.description}</span>
                      </div>
                      <button
                        class="btn-primary project-open-btn"
                        onClick={() => handleOpen(project)}
                        disabled={openingUrl() === project.url}
                      >
                        {openingUrl() === project.url ? (
                          <span class="spinner" />
                        ) : (
                          "Open"
                        )}
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </Show>
        </div>
      </Show>
    </div>
  );
}
