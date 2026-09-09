import { useSyncExternalStore } from "react";

/**
 * Independent viewport breakpoints (spec #37): width decides whether the
 * config summary column appears, height decides whether the keep-root/output
 * area is a single-line pill or the expanded block. Both are measured in CSS
 * pixels of the WebView viewport — display scaling is already folded in.
 */
export const WIDE_BREAKPOINT = 1000;
export const TALL_BREAKPOINT = 640;

export interface LayoutBreakpoints {
  wide: boolean;
  tall: boolean;
}

export function classifyLayout(width: number, height: number): LayoutBreakpoints {
  return {
    wide: width >= WIDE_BREAKPOINT,
    tall: height >= TALL_BREAKPOINT,
  };
}

let cached: LayoutBreakpoints | null = null;
const listeners = new Set<() => void>();

function getSnapshot(): LayoutBreakpoints {
  if (cached === null) {
    cached = classifyLayout(window.innerWidth, window.innerHeight);
  }
  return cached;
}

function subscribe(listener: () => void): () => void {
  if (listeners.size === 0) {
    window.addEventListener("resize", onResize);
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      window.removeEventListener("resize", onResize);
    }
  };
}

function onResize(): void {
  cached = null;
  for (const listener of listeners) listener();
}

/**
 * Live breakpoint pair. Conditionally rendered subtrees are removed from the
 * DOM (not CSS-hidden), so no invisible focusable controls linger when a
 * region is collapsed. Resize-driven re-renders change nothing else: the same
 * inputs feed the same state, nothing resets.
 */
export function useLayoutBreakpoints(): LayoutBreakpoints {
  return useSyncExternalStore(subscribe, getSnapshot);
}
