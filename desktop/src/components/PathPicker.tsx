import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon } from "lucide-react";

import { Button } from "@/components/ui/button";

interface PathPickerProps {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
  /** Hint shown below the input, e.g. for the manual-input escape hatch. */
  hint?: string;
}

export function PathPicker({ label, placeholder, value, onChange, hint }: PathPickerProps) {
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

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-muted-foreground text-xs">{label}</span>
      <div className="flex gap-2">
        <input
          value={value}
          placeholder={placeholder}
          spellCheck={false}
          onChange={(e) => onChange(e.target.value)}
          className="h-8 flex-1 rounded-md border bg-[var(--background-primary)] px-2.5 font-mono text-xs outline-none placeholder:font-sans placeholder:text-[var(--text-faint)] focus-visible:ring-2 focus-visible:ring-ring/60"
        />
        <Button variant="secondary" onClick={pick}>
          <FolderOpenIcon className="size-4" />
          浏览
        </Button>
      </div>
      {hint && <span className="text-[var(--text-faint)] text-[11px]">{hint}</span>}
    </div>
  );
}
