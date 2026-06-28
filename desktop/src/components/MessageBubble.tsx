import { Switch, Match } from "solid-js";
import {
  TbOutlineClock,
  TbOutlineCheck,
  TbOutlineChecks,
  TbOutlineAlertTriangle,
} from "solid-icons/tb";
import type { Message } from "../models";
import { DeliveryStatus } from "../models/Message";
import { formatTime } from "../lib/format";
import "./MessageBubble.css";

const DELIVERY_ICON_SIZE = 14;

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
        <span class="delivery sending"><TbOutlineClock size={DELIVERY_ICON_SIZE} /></span>
      </Match>
      <Match when={props.status === DeliveryStatus.sent}>
        <span class="delivery"><TbOutlineCheck size={DELIVERY_ICON_SIZE} /></span>
      </Match>
      <Match when={props.status === DeliveryStatus.delivered}>
        <span class="delivery"><TbOutlineChecks size={DELIVERY_ICON_SIZE} /></span>
      </Match>
      <Match when={props.status === DeliveryStatus.read}>
        <span class="delivery read"><TbOutlineChecks size={DELIVERY_ICON_SIZE} /></span>
      </Match>
      <Match when={props.status === DeliveryStatus.failed}>
        <span class="delivery failed">
          {/* TODO: wire retry handler — Day 3 (T23) */}
          <TbOutlineAlertTriangle size={DELIVERY_ICON_SIZE} />
          <span class="retry-hint">Tap to retry</span>
        </span>
      </Match>
    </Switch>
  );
}
