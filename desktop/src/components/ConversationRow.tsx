import { useApp } from "../state/AppContext";
import type { Conversation } from "../models";
import { formatRelative } from "../lib/format";
import AccountAvatar from "./AccountAvatar";
import "./ConversationRow.css";

interface Props {
  conversation: Conversation;
  selected: boolean;
  onSelect: (id: string) => void;
}

export default function ConversationRow(props: Props) {
  const { unreadCount } = useApp();
  const n = unreadCount(props.conversation);
  const did =
    props.conversation.recipientDid ??
    props.conversation.groupId ??
    props.conversation.id;

  return (
    <div
      class={`conversation-row${props.selected ? " selected" : ""}`}
      onClick={() => props.onSelect(props.conversation.id)}
    >
      <AccountAvatar name={props.conversation.title} did={did} />
      <div class="conv-info">
        <div class="conv-title">{props.conversation.title}</div>
        {props.conversation.lastMessage && (
          <div class="conv-preview">{props.conversation.lastMessage}</div>
        )}
      </div>
      <div class="conv-meta">
        {props.conversation.lastMessageDate && (
          <span class="conv-timestamp">
            {formatRelative(props.conversation.lastMessageDate)}
          </span>
        )}
        {n > 0 && <span class="unread-badge">{n}</span>}
      </div>
    </div>
  );
}
