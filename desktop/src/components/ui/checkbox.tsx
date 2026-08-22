import { CheckIcon, MinusIcon } from "lucide-react";

import { cn } from "@/lib/utils";

/**
 * Hand-rolled equivalent of the shadcn/ui checkbox (no radix dependency for
 * this single control). Supports controlled usage and an indeterminate look.
 */
function Checkbox({
  className,
  checked,
  indeterminate = false,
  onCheckedChange,
  ...props
}: Omit<React.ComponentProps<"button">, "onChange"> & {
  checked: boolean;
  indeterminate?: boolean;
  onCheckedChange?: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? "mixed" : checked}
      data-state={checked ? "checked" : "unchecked"}
      className={cn(
        "peer size-4 shrink-0 rounded-[4px] border border-[var(--background-modifier-border-hover)] transition-colors",
        "hover:bg-[var(--background-modifier-hover)]",
        "focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-[var(--interactive-accent)]",
        checked && "border-[var(--interactive-accent)] bg-[var(--interactive-accent)] text-white",
        className,
      )}
      onClick={() => onCheckedChange?.(!checked)}
      {...props}
    >
      {checked && !indeterminate && <CheckIcon className="size-3" strokeWidth={3} />}
      {indeterminate && <MinusIcon className="size-3" strokeWidth={3} />}
    </button>
  );
}

export { Checkbox };
