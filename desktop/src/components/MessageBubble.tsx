import { Switch, Match } from "solid-js";
import type { Message } from "../models";
import { DeliveryStatus } from "../models/Message";
import { formatTime } from "../lib/format";
import "./MessageBubble.css";

interface Props {
  message: Message;
  mine: boolean;
  isGroup: boolean;
  senderName?: string;
}

export default function MessageBubble(props: Props) {
  const deleted = props.message.isDeleted;

  return (
    <div class={`message-row ${props.mine ? "mine" : "theirs"}`}>
      {props.isGroup && !props.mine && props.senderName && (
        <span class="sender-name">{props.senderName}</span>
      )}
      {deleted ? (
        <div class="deleted-tombstone">This message was deleted</div>
      ) : (
        <div class="bubble">{props.message.body}</div>
      )}
      {!deleted && (
        <div class="message-meta">
          <span class="timestamp">
            {formatTime(props.message.sentAtMs)}
            {props.message.editCount > 0 && " (edited)"}
          </span>
          {props.mine && <DeliveryIndicator status={props.message.deliveryStatus} />}
        </div>
      )}
    </div>
  );
}

function DeliveryIndicator(props: { status: DeliveryStatus }) {
  return (
    <Switch>
      <Match when={props.status === DeliveryStatus.sending}>
        <span class="delivery sending">⏱</span>
      </Match>
      <Match when={props.status === DeliveryStatus.sent}>
        <span class="delivery">✓</span>
      </Match>
      <Match when={props.status === DeliveryStatus.delivered}>
        <span class="delivery">✓✓</span>
      </Match>
      <Match when={props.status === DeliveryStatus.read}>
        <span class="delivery read">✓✓</span>
      </Match>
      <Match when={props.status === DeliveryStatus.failed}>
        <span class="delivery failed">
          ⚠ {/* TODO: wire retry handler — Day 3 (T23) */}
          <span class="retry-hint">Tap to retry</span>
        </span>
      </Match>
    </Switch>
  );
}
