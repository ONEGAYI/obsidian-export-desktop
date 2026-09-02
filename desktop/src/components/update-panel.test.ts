import { afterEach, describe, expect, it, vi } from "vitest";

import type { UpdateEvent, UpdateExit } from "@/lib/sidecar";

import {
  EMPTY_UPDATE,
  applyUpdateEvents,
  applyUpdateExit,
  dueUpdateCheck,
  markUpdateChecked,
} from "./UpdatePanel";

const exit = (code: number | null): UpdateExit => ({ code, stderr: "" });

const availableResult: UpdateEvent = {
  type: "update-result",
  outcome: "available",
  version: "26.9.0",
  htmlUrl: "https://example.com/released",
  notes: "release notes",
  assetName: "Obsidian.Export_26.9.0_x64-setup.exe",
  assetSize: 8_000_000,
};

describe("applyUpdateEvents", () => {
  it("returns the state untouched for schema events", () => {
    expect(
      applyUpdateEvents(EMPTY_UPDATE, [{ type: "schema", version: 1 }]),
    ).toBe(EMPTY_UPDATE);
  });

  it("folds a verdict into result and clears previous-run leftovers", () => {
    const dirty = {
      ...EMPTY_UPDATE,
      exit: exit(1),
      streamErrors: ["parse error"],
      invokeError: "boom",
    };
    const next = applyUpdateEvents(dirty, [availableResult]);
    expect(next.phase).toBe("result");
    expect(next.version).toBe("26.9.0");
    expect(next.exit).toBeNull();
    expect(next.streamErrors).toEqual([]);
    expect(next.invokeError).toBeNull();
    expect(next.downloadPath).toBeNull();
  });

  it("keeps downloading when a mid-download re-check still finds the asset", () => {
    const downloading = {
      ...EMPTY_UPDATE,
      phase: "downloading" as const,
      downloadedBytes: 100,
      totalBytes: 200,
      bytesPerSecond: 5,
    };
    const next = applyUpdateEvents(downloading, [availableResult]);
    expect(next.phase).toBe("downloading");
    expect(next.downloadedBytes).toBe(100);
    expect(next.totalBytes).toBe(200);
  });

  it("folds back to result when a mid-download re-check loses the asset", () => {
    const downloading = {
      ...EMPTY_UPDATE,
      phase: "downloading" as const,
      downloadedBytes: 100,
      totalBytes: 200,
      bytesPerSecond: 5,
    };
    const gone: UpdateEvent = {
      type: "update-result",
      outcome: "up-to-date",
      version: null,
      htmlUrl: null,
      notes: null,
      assetName: null,
      assetSize: null,
    };
    const next = applyUpdateEvents(downloading, [gone]);
    expect(next.phase).toBe("result");
    expect(next.downloadedBytes).toBe(0);
    expect(next.totalBytes).toBeNull();
  });

  it("tracks download progress frames", () => {
    const next = applyUpdateEvents(EMPTY_UPDATE, [
      { type: "download-start", total: 200 },
      {
        type: "download-progress",
        downloaded: 120,
        total: 240,
        bytesPerSecond: 30,
      },
    ]);
    expect(next.phase).toBe("downloading");
    expect(next.downloadedBytes).toBe(120);
    expect(next.totalBytes).toBe(240);
    expect(next.bytesPerSecond).toBe(30);
  });

  it("moves to ready with the saved path on download-end", () => {
    const next = applyUpdateEvents(EMPTY_UPDATE, [
      { type: "download-end", path: "C:/Downloads/installer.exe" },
    ]);
    expect(next.phase).toBe("ready");
    expect(next.downloadPath).toBe("C:/Downloads/installer.exe");
  });
});

describe("applyUpdateExit", () => {
  it("folds a transitional phase to failed on exit", () => {
    for (const phase of ["checking", "downloading"] as const) {
      const state = { ...EMPTY_UPDATE, phase };
      const next = applyUpdateExit(state, exit(1));
      expect(next.phase, `phase ${phase}`).toBe("failed");
      expect(next.exit?.code).toBe(1);
    }
  });

  it("folds a cancelled transitional phase into cancelled instead of failed", () => {
    const cancelled = {
      ...EMPTY_UPDATE,
      phase: "downloading" as const,
      cancelled: true,
    };
    const next = applyUpdateExit(cancelled, exit(1));
    expect(next.phase).toBe("cancelled");
    expect(next.exit?.code).toBe(1);
  });

  it("records the exit without disturbing a settled verdict", () => {
    const ready = applyUpdateEvents(EMPTY_UPDATE, [
      { type: "download-end", path: "C:/x.exe" },
    ]);
    const next = applyUpdateExit(ready, exit(0));
    expect(next.phase).toBe("ready");
    expect(next.exit?.code).toBe(0);
    expect(next.downloadPath).toBe("C:/x.exe");
  });
});

// The launch throttle lives on localStorage; tests stub it with a Map-backed
// shim (the node environment has no DOM storage).
function stubStorage(initial: Record<string, string> = {}) {
  const store = new Map(Object.entries(initial));
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
  });
  return store;
}

describe("update check throttle", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("is due when never checked", () => {
    stubStorage();
    expect(dueUpdateCheck(1_000)).toBe(true);
  });

  it("is not due within 24h of the last check, due after", () => {
    stubStorage();
    const day = 24 * 60 * 60 * 1000;
    markUpdateChecked(1_000);
    expect(dueUpdateCheck(1_000 + day - 1)).toBe(false);
    expect(dueUpdateCheck(1_000 + day)).toBe(true);
  });

  it("is due again on a corrupted stored payload", () => {
    stubStorage({ "obsidian-export-update-state": "not json" });
    expect(dueUpdateCheck(1_000)).toBe(true);
  });

  it("is due on a payload without a numeric lastCheck", () => {
    stubStorage({ "obsidian-export-update-state": '{"lastCheck":"yesterday"}' });
    expect(dueUpdateCheck(1_000)).toBe(true);
  });

  it("persists the check timestamp through markUpdateChecked", () => {
    const store = stubStorage();
    markUpdateChecked(1_000);
    expect(store.get("obsidian-export-update-state")).toBe(
      '{"lastCheck":1000}',
    );
  });
});
