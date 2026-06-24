import { createSignal, createMemo, For, Show } from "solid-js";
import { useApp } from "../../state/AppContext";
import ConversationRow from "../../components/ConversationRow";
import RecoveryKeyBanner from "../../components/RecoveryKeyBanner";
import OfflineBanner from "../../components/OfflineBanner";
import ConversationView from "./ConversationView";
import "./ChatsView.css";

export default function ChatsView() {
  const { store, loadMessagesFromStore, unreadCount } = useApp();
  const [selectedId, setSelectedId] = createSignal<string | null>(null);

  const selected = () =>
    store.conversations.find((c) => c.id === selectedId()) ?? null;

  const totalUnread = createMemo(() =>
    store.conversations.reduce((sum, c) => sum + unreadCount(c), 0)
  );

  return (
    <div class="chats-split">
      <div class="chats-list-panel">
        <div class="chats-header">
          Chats
          {totalUnread() > 0 && (
            <span class="chats-unread-badge">{totalUnread()}</span>
          )}
        </div>
        <RecoveryKeyBanner />
        <OfflineBanner />
        <div class="conversation-list scrollbar-thin">
          <For
            each={store.conversations}
            fallback={
              <div class="empty-state">
                No conversations yet. Join a server to get started.
              </div>
            }
          >
            {(conv) => (
              <ConversationRow
                conversation={conv}
                selected={selectedId() === conv.id}
                onSelect={(id) => {
                  setSelectedId(id);
                  loadMessagesFromStore(id, conv.accountId);
                }}
              />
            )}
          </For>
        </div>
      </div>
      <div class="detail-panel">
        <Show
          when={selected()}
          fallback={<div class="no-selection">Select a conversation</div>}
        >
          {(conv) => <ConversationView conversation={conv()} />}
        </Show>
      </div>
    </div>
  );
}
