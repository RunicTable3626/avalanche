import { createSignal, type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { FiArrowLeft } from "solid-icons/fi";
import { useApp } from "../../state/AppContext";
import { ServiceMode } from "../../services/AvalancheService";
import "./DevSettingsView.css";

export default function DevSettingsView(): JSX.Element {
  const { store, switchMode, logout } = useApp();
  // useNavigate throws if rendered outside a Router — guard gracefully.
  let navigate: ReturnType<typeof useNavigate> | undefined;
  try {
    navigate = useNavigate();
  } catch {
    // rendered outside Router context (e.g. test), navigation is a no-op
  }
  const [saving, setSaving] = createSignal(false);

  function handleModeChange(mode: ServiceMode) {
    if (mode === store.serviceMode) return;
    setSaving(true);
    switchMode(mode);
    // Give the reset a tick to settle, then navigate home.
    // switchMode sets isOnboarding=true and clears accounts, so
    // the router will render OnboardingFlow automatically.
    setTimeout(() => {
      setSaving(false);
      navigate?.("/");
    }, 0);
  }

  function handleLogout() {
    logout();
    navigate?.("/");
  }

  return (
    <div class="dev-settings">
      <header class="dev-settings-header">
        <button class="back-btn" onClick={() => navigate?.(-1)}>
          <FiArrowLeft size={14} />Back
        </button>
        <h1>Developer Settings</h1>
      </header>

      <section class="dev-settings-section">
        <h2>Service Mode</h2>
        <p class="dev-settings-hint">
          Mock mode uses in-memory seeded data. DevServer mode connects to a
          local homeserver.
        </p>
        <div class="mode-selector">
          <label
            class={`mode-option${store.serviceMode === ServiceMode.Mock ? " selected" : ""}`}
          >
            <input
              type="radio"
              name="serviceMode"
              value={ServiceMode.Mock}
              checked={store.serviceMode === ServiceMode.Mock}
              onChange={() => handleModeChange(ServiceMode.Mock)}
              disabled={saving()}
            />
            <span class="mode-label">Mock</span>
            <span class="mode-desc">Seeded data, no server</span>
          </label>
          <label
            class={`mode-option${store.serviceMode === ServiceMode.DevServer ? " selected" : ""}`}
          >
            <input
              type="radio"
              name="serviceMode"
              value={ServiceMode.DevServer}
              checked={store.serviceMode === ServiceMode.DevServer}
              onChange={() => handleModeChange(ServiceMode.DevServer)}
              disabled={saving()}
            />
            <span class="mode-label">DevServer</span>
            <span class="mode-desc">Connect to local homeserver</span>
          </label>
        </div>
        {saving() && <span class="spinner" />}
      </section>

      <section class="dev-settings-section">
        <h2>Session</h2>
        <p class="dev-settings-hint">
          {store.accounts.length === 0
            ? "No accounts signed in."
            : `${store.accounts.length} account${store.accounts.length > 1 ? "s" : ""} signed in.`}
        </p>
        <button class="btn-secondary" onClick={handleLogout}>
          Sign Out
        </button>
      </section>
    </div>
  );
}
