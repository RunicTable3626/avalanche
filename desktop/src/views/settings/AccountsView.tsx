import { For } from "solid-js";
import { FiArrowLeft, FiChevronRight } from "solid-icons/fi";
import { useApp } from "../../state/AppContext";
import AccountAvatar from "../../components/AccountAvatar";
import type { Account, ServerInfo } from "../../models";
import "./AccountsView.css";

interface Props {
  onBack: () => void;
  onOpenIdentity: (account: Account) => void;
  onOpenServer: (account: Account, server: ServerInfo) => void;
}

/**
 * Accounts list: each identity with its servers. Mirrors iOS AccountsView,
 * minus "Scan Invite" (QR divergence) and "Add an account" (the multi-account
 * refactor is a dedicated branch, out of Day-5 scope).
 */
export default function AccountsView(props: Props) {
  const { store } = useApp();

  const isHome = (account: Account, server: ServerInfo) =>
    account.servers[0]?.id === server.id;

  return (
    <div class="accounts-view">
      <header class="settings-subheader">
        <button class="back-btn" onClick={props.onBack}>
          <FiArrowLeft size={14} />Back
        </button>
        <h1>Accounts</h1>
      </header>

      <div class="accounts-body scrollbar-thin">
        <For each={store.accounts}>
          {(account) => (
            <section class="accounts-card">
              <button class="accounts-identity-row" onClick={() => props.onOpenIdentity(account)}>
                <AccountAvatar name={account.displayName} did={account.id} />
                <span class="accounts-identity-name">{account.displayName}</span>
                <FiChevronRight size={18} class="accounts-chevron" />
              </button>

              <For each={[...account.servers].sort((a, b) => a.name.localeCompare(b.name))}>
                {(server) => (
                  <button class="accounts-server-row" onClick={() => props.onOpenServer(account, server)}>
                    <span class="accounts-server-name">{server.name}</span>
                    {isHome(account, server) && <span class="accounts-home-badge">home</span>}
                    <FiChevronRight size={16} class="accounts-chevron" />
                  </button>
                )}
              </For>
            </section>
          )}
        </For>
      </div>
    </div>
  );
}
