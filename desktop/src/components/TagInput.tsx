import { useRef, useState } from "react";
import { XIcon } from "lucide-react";

interface TagInputProps {
  value: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
}

/**
 * Multi-value tag editor for the `--skip-tags` / `--only-tags` flags. Enter
 * or a comma commits the draft (a leading `#` is stripped since the CLI
 * expects bare tag names), Backspace on an empty draft removes the last
 * chip, and each chip has an × button.
 */
export function TagInput({ value, onChange, placeholder }: TagInputProps) {
  const [draft, setDraft] = useState("");
  // True between compositionstart/compositionend. If focus is lost while an
  // IME composition is still open (blur fires before compositionend on some
  // IMEs), the draft holds raw pinyin and must be discarded, not committed.
  const composingRef = useRef(false);

  const commitDraft = () => {
    if (composingRef.current) {
      setDraft("");
      return;
    }
    const tag = draft.trim().replace(/^#/, "");
    setDraft("");
    if (tag === "" || value.includes(tag)) {
      return;
    }
    onChange([...value, tag]);
  };

  return (
    <div className="flex min-h-8 flex-wrap items-center gap-1.5 rounded-md border border-[var(--background-modifier-border)] bg-[var(--background-primary)] px-2 py-1 transition-colors focus-within:border-[var(--interactive-accent)]">
      {value.map((tag) => (
        <span
          key={tag}
          className="flex items-center gap-1 rounded bg-[var(--background-modifier-hover)] px-1.5 py-0.5 font-mono text-xs"
        >
          {tag}
          <button
            type="button"
            className="text-[var(--text-muted)] transition-colors hover:text-[var(--text-normal)]"
            onClick={() => onChange(value.filter((t) => t !== tag))}
            aria-label={`移除标签 ${tag}`}
          >
            <XIcon className="size-3" />
          </button>
        </span>
      ))}
      <input
        className="min-w-24 flex-1 bg-transparent text-sm outline-none placeholder:text-[var(--text-faint)]"
        value={draft}
        placeholder={placeholder ?? "输入后回车添加"}
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
        onCompositionStart={() => {
          composingRef.current = true;
        }}
        onCompositionEnd={() => {
          composingRef.current = false;
        }}
        onKeyDown={(e) => {
          // While an IME is composing (e.g. pinyin candidates), Enter
          // confirms the candidate — it must not commit the raw draft.
          if (e.nativeEvent.isComposing) return;
          if (e.key === "Enter" || e.key === "," || e.key === "，") {
            e.preventDefault();
            commitDraft();
          } else if (
            e.key === "Backspace" &&
            draft === "" &&
            value.length > 0
          ) {
            onChange(value.slice(0, -1));
          }
        }}
        onBlur={commitDraft}
      />
    </div>
  );
}
