import { createSignal, Show, type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { FiArrowLeft } from "solid-icons/fi";
import { useApp } from "../../state/AppContext";
import BlockedContactsView from "./BlockedContactsView";
import "./DevSettingsView.css";

interface Props {
  // When embedded inside the SettingsView hub, the hub supplies its own back
  // handler. When routed standalone, falls back to router navigation.
  onBack?: () => void;
}

export default function DevSettingsView(props: Props = {}): JSX.Element {
  const { store, logout } = useApp();
  const [showBlocked, setShowBlocked] = createSignal(false);
  // useNavigate throws if rendered outside a Router — guard gracefully.
  let navigate: ReturnType<typeof useNavigate> | undefined;
  try {
    navigate = useNavigate();
  } catch {
    // rendered outside Router context (e.g. test), navigation is a no-op
  }

  function handleBack() {
    if (props.onBack) props.onBack();
    else navigate?.(-1);
  }

  function handleLogout() {
    logout();
    navigate?.("/");
  }

  return (
    <div class="dev-settings">
      <header class="dev-settings-header">
        <button class="back-btn" onClick={handleBack}>
          <FiArrowLeft size={14} />Back
        </button>
        <h1>Developer</h1>
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

      <section class="dev-settings-section">
        <h2>Safety</h2>
        <p class="dev-settings-hint">
          Contacts you have blocked. Unblock to allow their messages again.
        </p>
        <button class="btn-secondary" onClick={() => setShowBlocked(true)}>
          Blocked Contacts
        </button>
      </section>

      <Show when={showBlocked()}>
        <BlockedContactsView onClose={() => setShowBlocked(false)} />
      </Show>
    </div>
  );
}
