import { createSignal, onMount, Show } from "solid-js";
import { FiShield, FiX } from "solid-icons/fi";
import { useApp } from "../state/AppContext";
import RecoveryPhraseSetupView from "../views/settings/RecoveryPhraseSetupView";
import "./RecoveryKeyBanner.css";

/**
 * "Secure your account" prompt shown when the signed-in account has no recovery
 * blob yet (mirrors iOS RecoveryKeyBanner). Checks hasRecovery() on mount;
 * "Set up" opens the recovery-phrase setup flow. Hidden once recovery exists or
 * the user dismisses it for the session.
 */
export default function RecoveryKeyBanner() {
  const { store, hasRecovery } = useApp();
  const [visible, setVisible] = createSignal(false);
  const [showSetup, setShowSetup] = createSignal(false);

  async function refresh() {
    // No account → nothing to secure. Otherwise show only when recovery is unset.
    if (store.accounts.length === 0) {
      setVisible(false);
      return;
    }
    setVisible(!(await hasRecovery()));
  }

  onMount(() => {
    void refresh();
  });

  return (
    <>
      <Show when={visible()}>
        <div class="recovery-banner">
          <FiShield size={16} />
          <span class="recovery-banner-text">Secure your account</span>
          <button class="recovery-banner-setup" onClick={() => setShowSetup(true)}>
            Set up
          </button>
          <button
            class="recovery-banner-dismiss"
            onClick={() => setVisible(false)}
            aria-label="Dismiss"
          >
            <FiX size={14} />
          </button>
        </div>
      </Show>

      <Show when={showSetup()}>
        <RecoveryPhraseSetupView
          onClose={() => setShowSetup(false)}
          onComplete={() => {
            // Recovery now exists — re-check so the banner clears.
            void refresh();
          }}
        />
      </Show>
    </>
  );
}
