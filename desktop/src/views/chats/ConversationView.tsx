import { createEffect, createMemo, onMount, For, Show } from "solid-js";
import { useApp } from "../../state/AppContext";
import type { Conversation } from "../../models";
import { initials } from "../../lib/format";
import MessageBubble from "../../components/MessageBubble";
import ComposeMessageView from "../../components/ComposeMessageView";
import "./ConversationView.css";

interface Props {
  conversation: Conversation;
}

export default function ConversationView(props: Props) {
  const { store, loadMessagesFromStore, markAllMessagesRead, displayName } = useApp();
  let messagesEnd: HTMLDivElement | undefined;

  // Re-runs whenever conversation changes, not just on first mount.
  createEffect(() => {
    loadMessagesFromStore(props.conversation.id, props.conversation.accountId);
  });

  // Mark all messages read when messages arrive (handles both initial async
  // load and new incoming messages).  Tracking messages().length ensures this
  // re-runs after the async fetch resolves — the conversation-id-only effect
  // would fire before messages arrived from disk.
  createEffect(() => {
    const msgs = messages();
    msgs.length; // track — re-run when messages actually arrive
    markAllMessagesRead(props.conversation.id, props.conversation.accountId);
  });

  onMount(() => {
    messagesEnd?.scrollIntoView();
  });

  // createMemo ensures the For list re-renders when the async store write lands.
  const messages = createMemo(() => store.messagesByConversation[props.conversation.id] ?? []);

  // Auto-scroll when message count changes (new messages arrive or are sent).
  createEffect(() => {
    messages().length; // track
    messagesEnd?.scrollIntoView({ behavior: "smooth" });
  });

  return (
    <div class="conv-view">
      <div class="conv-header">
        <div class="conv-header-avatar">{initials(props.conversation.title)}</div>
        {props.conversation.title}
      </div>
      <div class="messages-list scrollbar-thin">
        <Show
          when={messages().length > 0}
          fallback={<div class="empty-conv">No messages yet.</div>}
        >
          <For each={messages()}>
            {(msg) => (
              <MessageBubble
                message={msg}
                mine={msg.senderAccountId === props.conversation.accountId}
                isGroup={props.conversation.isGroup}
                senderName={displayName(msg.senderAccountId, props.conversation.accountId)}
              />
            )}
          </For>
        </Show>
        <div ref={messagesEnd} />
      </div>
      <ComposeMessageView conversation={props.conversation} />
    </div>
  );
}
