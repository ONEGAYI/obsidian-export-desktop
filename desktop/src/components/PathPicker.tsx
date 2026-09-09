import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n";
import { displayName } from "@/lib/naming";

/**
 * Discriminated on `variant`: the card variant requires its plinth `icon`
 * (source vs destination) and may take a `namePlaceholder`; the default
 * variant (settings view) accepts neither — the compiler rejects the silent
 * fall-back that used to happen when a card was requested without an icon.
 */
export type PathPickerProps = {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
  /** Hint shown below the input, e.g. for the manual-input escape hatch. */
  hint?: string;
} & (
  | { variant?: "default"; icon?: never; namePlaceholder?: never }
  | {
      /** Home-card variant: icon plinth plus a derived-name row above the
       * input row (spec #37 §4.2). */
      variant: "card";
      icon: LucideIcon;
      /** Name shown for blank input (a placeholder — never a made-up name). */
      namePlaceholder?: string;
    }
);

export function PathPicker({
  label,
  placeholder,
  value,
  onChange,
  hint,
  variant = "default",
  icon: Icon,
  namePlaceholder,
}: PathPickerProps) {
  const { t } = useI18n();
  const pick = async () => {
    const picked = await open({
      directory: true,
      multiple: false,
      title: label,
    });
    if (typeof picked === "string") {
      onChange(picked);
    }
  };

  const inputRow = (
    <div className="flex gap-2">
      <input
        value={value}
        placeholder={placeholder}
        aria-label={label}
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 min-w-0 flex-1 rounded-md border bg-[var(--background-primary)] px-2.5 font-mono text-xs outline-none placeholder:font-sans placeholder:text-[var(--text-faint)] focus-visible:ring-2 focus-visible:ring-ring/60"
      />
      <Button variant="secondary" onClick={pick} className="shrink-0">
        <FolderOpenIcon className="size-4" />
        {t.common.browse}
      </Button>
    </div>
  );

  if (variant === "card" && Icon) {
    const name = displayName(value);
    return (
      <div className="flex items-center gap-3">
        <span
          aria-hidden
          className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-[var(--pill-background)] text-[var(--interactive-accent)]"
        >
          <Icon className="size-5" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <span className="flex min-w-0 items-baseline gap-2">
            <span className="shrink-0 text-xs text-muted-foreground">
              {label}
            </span>
            <span
              className={`truncate text-sm font-medium ${
                name === null ? "text-[var(--text-faint)] font-normal" : ""
              }`}
              title={name ?? undefined}
            >
              {name ?? namePlaceholder ?? placeholder}
            </span>
          </span>
          {inputRow}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-muted-foreground text-xs">{label}</span>
      {inputRow}
      {hint && <span className="text-[var(--text-faint)] text-[11px]">{hint}</span>}
    </div>
  );
}
