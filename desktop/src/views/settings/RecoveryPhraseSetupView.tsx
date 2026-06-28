import { createSignal, For, Show, onMount } from "solid-js";
import { FiX } from "solid-icons/fi";
import { useApp } from "../../state/AppContext";
import "./RecoveryPhraseSetupView.css";

interface Props {
  onClose: () => void;
  onComplete?: () => void;
}

type Stage = "loading" | "display" | "verify" | "done";

// Pick three distinct 1-based word positions in ascending order.
function pickQuizPositions(count: number): number[] {
  if (count < 3) return Array.from({ length: Math.max(count, 1) }, (_, i) => i + 1);
  const chosen = new Set<number>();
  while (chosen.size < 3) chosen.add(1 + Math.floor(Math.random() * count));
  return [...chosen].sort((a, b) => a - b);
}

/**
 * Recovery-phrase setup (mirrors iOS RecoveryPhraseSetupView, but for *securing*
 * an already-created account rather than signup): generate a 12-word BIP39
 * phrase, have the user write it down and confirm three words, then derive the
 * seed and upload the recovery blob (updateRecoveryBlob). Desktop has no
 * passkey/PRF, so the phrase is the only recovery method (see desktop/CLAUDE.md).
 */
export default function RecoveryPhraseSetupView(props: Props) {
  const { generateRecoveryPhrase, setupRecoveryFromPhrase } = useApp();

  const [stage, setStage] = createSignal<Stage>("loading");
  const [words, setWords] = createSignal<string[]>([]);
  const [quizPositions, setQuizPositions] = createSignal<number[]>([]);
  const [answers, setAnswers] = createSignal<Record<number, string>>({});
  const [error, setError] = createSignal<string | null>(null);
  const [saving, setSaving] = createSignal(false);

  onMount(() => {
    void (async () => {
      try {
        const phrase = await generateRecoveryPhrase();
        const w = phrase.split(/\s+/).filter(Boolean);
        setWords(w);
        setQuizPositions(pickQuizPositions(w.length));
        setStage("display");
      } catch (e) {
        setError(e instanceof Error ? e.message : "Couldn't generate a recovery phrase");
      }
    })();
  });

  function allCorrect(): boolean {
    return quizPositions().every((pos) => {
      const expected = words()[pos - 1]?.toLowerCase() ?? "";
      const got = (answers()[pos] ?? "").trim().toLowerCase();
      return expected === got;
    });
  }

  async function verifyAndSave() {
    if (!allCorrect()) {
      setError("Those words don't match. Double-check what you wrote down.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await setupRecoveryFromPhrase(words().join(" "));
      setStage("done");
      props.onComplete?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't save your recovery key");
      setSaving(false);
    }
  }

  return (
    <div class="recovery-backdrop" onClick={props.onClose}>
      <div class="recovery-sheet" onClick={(e) => e.stopPropagation()}>
        <div class="recovery-header">
          <span>Recovery phrase</span>
          <button class="recovery-close" onClick={props.onClose} aria-label="Close">
            <FiX size={18} />
          </button>
        </div>

        <div class="recovery-body scrollbar-thin">
          <Show when={stage() === "loading"}>
            <div class="recovery-loading">Generating…</div>
          </Show>

          <Show when={stage() === "display"}>
            <h2>Write down your recovery phrase</h2>
            <p class="recovery-hint">
              These 12 words are the only way to recover this identity. Store them
              somewhere safe — anyone with them can access your account.
            </p>
            <ol class="recovery-words">
              <For each={words()}>
                {(word) => <li class="recovery-word">{word}</li>}
              </For>
            </ol>
            <button
              class="btn-primary recovery-action"
              onClick={() => {
                setAnswers({});
                setError(null);
                setStage("verify");
              }}
            >
              I've written it down
            </button>
          </Show>

          <Show when={stage() === "verify"}>
            <h2>Confirm your recovery phrase</h2>
            <p class="recovery-hint">Enter the following words from the phrase you wrote down.</p>
            <div class="recovery-quiz">
              <For each={quizPositions()}>
                {(pos) => (
                  <label class="recovery-quiz-row">
                    <span>Word #{pos}</span>
                    <input
                      class="text-input"
                      value={answers()[pos] ?? ""}
                      onInput={(e) => setAnswers({ ...answers(), [pos]: e.currentTarget.value })}
                      spellcheck={false}
                      autocomplete="off"
                    />
                  </label>
                )}
              </For>
            </div>
            <Show when={error()}>
              <p class="settings-error">{error()}</p>
            </Show>
            <div class="recovery-verify-actions">
              <button class="btn-secondary" onClick={() => setStage("display")} disabled={saving()}>
                Show phrase again
              </button>
              <button class="btn-primary" onClick={() => void verifyAndSave()} disabled={saving()}>
                {saving() ? "Saving…" : "Verify & Save"}
              </button>
            </div>
          </Show>

          <Show when={stage() === "done"}>
            <h2>You're all set</h2>
            <p class="recovery-hint">
              Your recovery key is saved. You can restore this identity on another
              device with these 12 words and your home server.
            </p>
            <button class="btn-primary recovery-action" onClick={props.onClose}>Done</button>
          </Show>

          <Show when={error() && stage() !== "verify"}>
            <p class="settings-error">{error()}</p>
          </Show>
        </div>
      </div>
    </div>
  );
}
