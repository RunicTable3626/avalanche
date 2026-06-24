import { createSignal, onMount } from "solid-js";
import { useApp } from "../state/AppContext";
import type { Conversation } from "../models";
import "./ComposeMessageView.css";

interface Props {
  conversation: Conversation;
}

/** Collapsed max-height (~2-3 lines). */
const COLLAPSED_MAX = 72;
/** Expanded max-height for long messages. */
const EXPANDED_MAX = 212;

export default function ComposeMessageView(props: Props) {
  const { sendMessage, sendGroupMessage } = useApp();
  const [draft, setDraft] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [expanded, setExpanded] = createSignal(false);
  const [mounted, setMounted] = createSignal(false);
  let inputRef: HTMLTextAreaElement | undefined;

  onMount(() => {
    // Reveal the caret only after the editor has fully mounted, so there's
    // no flash of a blinking cursor before the rich-text engine initialises.
    setMounted(true);
    inputRef?.focus();
  });

  function resizeTextarea() {
    const el = inputRef;
    if (!el) return;
    el.style.height = "auto";
    // When expanded, the textarea has a minimum visible height so the
    // toggle is a visible change even when the box is empty.  Collapsed
    // mode only grows to fit content up to the collapsed cap.
    const h = expanded()
      ? Math.min(Math.max(el.scrollHeight, 120), EXPANDED_MAX)
      : Math.min(el.scrollHeight, COLLAPSED_MAX);
    el.style.height = `${h}px`;
  }

  function toggleExpand() {
    setExpanded((prev) => !prev);
    // Defer so the signal propagates before reading clientHeight.
    setTimeout(() => resizeTextarea(), 0);
  }

  async function handleSend() {
    const text = draft().trim();
    if (!text || sending()) return;

    if (!props.conversation.isGroup && !props.conversation.recipientDid) return;

    setDraft("");
    setSending(true);
    setExpanded(false);
    setTimeout(() => resizeTextarea(), 0);
    try {
      if (props.conversation.isGroup) {
        await sendGroupMessage(props.conversation, text);
      } else {
        await sendMessage(
          props.conversation.id,
          text,
          props.conversation.recipientDid!,
          props.conversation.accountId
        );
      }
    } catch {
      // optimistic update already shows failed state
    } finally {
      setSending(false);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  }

  return (
    <div class="compose-row">
      <div class="compose-input-wrap" classList={{ expanded: expanded() }}>
        <textarea
          ref={inputRef}
          class="compose-input scrollbar-thin"
          classList={{ mounted: mounted(), expanded: expanded() }}
          placeholder="Message"
          rows={1}
          value={draft()}
          onInput={(e) => {
            setDraft(e.currentTarget.value);
            resizeTextarea();
          }}
          onKeyDown={handleKeyDown}
          disabled={sending()}
        />
        {!sending() && (
          <button
            class="compose-expand-tab"
            onClick={toggleExpand}
            aria-label={expanded() ? "Collapse" : "Expand"}
          >
            {expanded() ? "▼" : "▲"}
          </button>
        )}
      </div>
      <button
        class="send-btn"
        disabled={!draft().trim() || sending()}
        onClick={handleSend}
      >
        ↑
      </button>
    </div>
  );
}
