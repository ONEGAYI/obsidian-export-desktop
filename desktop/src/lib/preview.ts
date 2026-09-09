import { useEffect, useMemo, useSyncExternalStore } from "react";

import {
  previewExportDestination,
  type DestinationPreview,
} from "@/lib/sidecar";

/** The three inputs the resolved destination depends on. */
export interface PreviewInput {
  source: string;
  destination: string;
  keepRootFolder: boolean;
}

/** What the home view and the confirm dialog render for the landing path. */
export type PreviewState =
  /** Incomplete input (blank source or destination): waiting for a choice. */
  | { phase: "idle" }
  /** Complete input, debouncing or request in flight; no stale path shown. */
  | { phase: "pending" }
  /** Resolution succeeded. `sourceKind: "other"` means the source was not
   * found on disk — the caller must not present it as a valid export. */
  | { phase: "ready"; target: string; sourceKind: DestinationPreview["sourceKind"] }
  /** Resolution itself failed (e.g. unparsable path); technical message. */
  | { phase: "failed"; message: string };

export type PreviewFetcher = (input: PreviewInput) => Promise<DestinationPreview>;

/** The `ready` arm of [`PreviewState`]. */
export type ReadyPreview = Extract<PreviewState, { phase: "ready" }>;

/**
 * True when the preview resolved to a landing path the caller may present as
 * the export destination (ready, and the source actually exists on disk —
 * `sourceKind: "other"` must not be shown as a valid export). Shared by the
 * home output area and the confirm dialog so "usable" means one thing; as a
 * type predicate it also narrows to the ready arm for `target` access.
 */
export function isPreviewUsable(state: PreviewState): state is ReadyPreview {
  return state.phase === "ready" && state.sourceKind !== "other";
}

/**
 * True when the source resolved to a single file: the keep-root preference
 * never applies to it (both surfaces show the same explanatory note).
 */
export function isFileSource(
  state: PreviewState,
): state is ReadyPreview & { sourceKind: "file" } {
  return state.phase === "ready" && state.sourceKind === "file";
}

/** Debounce window the spec picked for merging rapid input edits (~150ms). */
const DEBOUNCE_MS = 150;

function isComplete(input: PreviewInput): boolean {
  return input.source.trim() !== "" && input.destination.trim() !== "";
}

function sameInput(a: PreviewInput, b: PreviewInput): boolean {
  return (
    a.source === b.source &&
    a.destination === b.destination &&
    a.keepRootFolder === b.keepRootFolder
  );
}

/**
 * Framework-free orchestrator for the destination preview, so the debounce /
 * stale-result rules are unit-testable without React:
 *
 * - blank input never issues a request;
 * - a complete input is debounced (~150ms) so bursts of edits merge;
 * - every input change (including a keep-root toggle) immediately invalidates
 *   any previous result — a late answer from an older generation is dropped,
 *   never allowed to masquerade as the current landing path;
 * - identical repeated inputs (e.g. re-render with unchanged values) neither
 *   re-request nor flash back to pending.
 */
export function createDestinationPreview(fetcher: PreviewFetcher) {
  let state: PreviewState = { phase: "idle" };
  let latest: PreviewInput | null = null;
  /** Generation counter: bumped on every accepted input change and on
   * dispose; a result only lands when its generation is still current. */
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const listeners = new Set<() => void>();

  const setState = (next: PreviewState) => {
    state = next;
    for (const listener of listeners) listener();
  };

  const fire = (input: PreviewInput) => {
    const myGeneration = generation;
    fetcher(input).then(
      (result) => {
        if (myGeneration === generation) {
          setState({
            phase: "ready",
            target: result.target,
            sourceKind: result.sourceKind,
          });
        }
      },
      (err: unknown) => {
        if (myGeneration === generation) {
          setState({ phase: "failed", message: String(err) });
        }
      },
    );
  };

  return {
    setInput(next: PreviewInput): void {
      if (latest && sameInput(latest, next)) {
        return;
      }
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      latest = next;
      // Invalidate every in-flight answer up front: results from the old
      // generation must not land even while the new debounce is pending.
      generation += 1;
      if (!isComplete(next)) {
        setState({ phase: "idle" });
        return;
      }
      setState({ phase: "pending" });
      timer = setTimeout(() => {
        timer = null;
        fire(next);
      }, DEBOUNCE_MS);
    },
    getState(): PreviewState {
      return state;
    },
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose(): void {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      generation += 1;
      // Reset the dedup memory: StrictMode's mount→unmount→remount cycle
      // replays setInput with identical values on the same controller, and a
      // stale `latest` would short-circuit that replay into never rescheduling
      // (the preview then stays on its initial state forever — smoke defect A).
      latest = null;
      listeners.clear();
    },
  };
}

/**
 * React binding for [`createDestinationPreview`]. The controller is a
 * framework-free external store read through `useSyncExternalStore`, so the
 * React state always matches the store (including the first `pending` fired
 * during mount, before any subscription callback could run). Re-renders with
 * unchanged source/destination/keepRootFolder (language or viewport switches
 * included) do not re-query; the controller lives for the app's lifetime.
 */
export function useDestinationPreview(input: PreviewInput): PreviewState {
  const controller = useMemo(
    () =>
      createDestinationPreview((request) =>
        previewExportDestination(
          request.source,
          request.destination,
          request.keepRootFolder,
        ),
      ),
    [],
  );
  useEffect(() => {
    controller.setInput(input);
  }, [controller, input.source, input.destination, input.keepRootFolder]);
  useEffect(() => () => controller.dispose(), [controller]);
  return useSyncExternalStore(controller.subscribe, controller.getState);
}
