import * as React from "react";

import { cn } from "@/lib/utils";

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      className={cn(
        "flex h-8 w-full rounded-md border border-[var(--background-modifier-border)] bg-[var(--background-primary)] px-2.5 font-mono text-xs transition-colors",
        "placeholder:font-sans placeholder:text-[var(--text-faint)]",
        "focus-visible:border-[var(--interactive-accent)] focus-visible:outline-none",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
