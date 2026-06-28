import { type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { FiArrowLeft } from "solid-icons/fi";
import { useApp } from "../../state/AppContext";
import "./DevSettingsView.css";

export default function DevSettingsView(): JSX.Element {
  const { store, logout } = useApp();
  // useNavigate throws if rendered outside a Router — guard gracefully.
  let navigate: ReturnType<typeof useNavigate> | undefined;
  try {
    navigate = useNavigate();
  } catch {
    // rendered outside Router context (e.g. test), navigation is a no-op
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
        <h1>Settings</h1>
      </header>

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
