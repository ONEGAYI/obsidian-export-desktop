import { describe, expect, it } from "vitest";

import type { CheckEvent, CheckStatus } from "@/lib/sidecar";

import {
  EMPTY_LINK_CHECK,
  applyCheckEvents,
  applyCheckExit,
  isBroken,
} from "./LinkCheckPanel";

function report(status: CheckStatus): CheckEvent {
  return {
    type: "link-report",
    source: "note.md",
    line: 3,
    raw: "[[note]]",
    kind: "wiki-link",
    status,
  };
}

describe("applyCheckExit", () => {
  const exit = { code: null, stderr: "" };

  it("folds a cancelled run into the cancelled verdict", () => {
    const running = {
      ...EMPTY_LINK_CHECK,
      phase: "running" as const,
      cancelled: true,
    };
    const next = applyCheckExit(running, exit);
    expect(next.phase).toBe("cancelled");
    expect(next.exit).toBe(exit);
  });

  it("prefers the cancelled verdict even when the end summary arrived", () => {
    const running = {
      ...EMPTY_LINK_CHECK,
      phase: "running" as const,
      cancelled: true,
      end: { filesChecked: 1, totalLinks: 2, broken: 0, skipped: 0 },
    };
    expect(applyCheckExit(running, exit).phase).toBe("cancelled");
  });

  it("folds a finished run into done when the end summary arrived", () => {
    const running = {
      ...EMPTY_LINK_CHECK,
      phase: "running" as const,
      end: { filesChecked: 1, totalLinks: 2, broken: 0, skipped: 0 },
    };
    expect(applyCheckExit(running, { ...exit, code: 1 }).phase).toBe("done");
  });

  it("folds an aborted run without an end summary into failed", () => {
    const running = { ...EMPTY_LINK_CHECK, phase: "running" as const };
    expect(applyCheckExit(running, exit).phase).toBe("failed");
  });

  it("leaves a non-running state untouched", () => {
    const done = { ...EMPTY_LINK_CHECK, phase: "done" as const };
    expect(applyCheckExit(done, exit)).toBe(done);
  });
});

describe("isBroken", () => {
  it("treats ok and external-skipped as healthy, everything else as broken", () => {
    expect(isBroken({ type: "ok" })).toBe(false);
    expect(isBroken({ type: "external-skipped", url: "https://x" })).toBe(false);
    expect(isBroken({ type: "missing-file", target: "t" })).toBe(true);
    expect(isBroken({ type: "out-of-bounds", target: "t" })).toBe(true);
    expect(isBroken({ type: "missing-section", target: "t", section: "s" })).toBe(true);
    expect(isBroken({ type: "missing-block", target: "t", block: "b" })).toBe(true);
    expect(isBroken({ type: "file-unreadable", message: "m" })).toBe(true);
    expect(isBroken({ type: "unknown" })).toBe(true);
  });
});

describe("applyCheckEvents", () => {
  it("returns the state untouched for an empty batch", () => {
    expect(applyCheckEvents(EMPTY_LINK_CHECK, [])).toBe(EMPTY_LINK_CHECK);
  });

  it("ignores schema and check-start noise", () => {
    const next = applyCheckEvents(EMPTY_LINK_CHECK, [
      { type: "schema", version: 1 },
      { type: "check-start", files: 10 },
    ]);
    expect(next).toEqual(EMPTY_LINK_CHECK);
  });

  it("appends reports and counts broken/skipped incrementally", () => {
    const next = applyCheckEvents(EMPTY_LINK_CHECK, [
      report({ type: "ok" }),
      report({ type: "missing-file", target: "gone" }),
      report({ type: "external-skipped", url: "https://x" }),
    ]);
    expect(next.reports).toHaveLength(3);
    expect(next.brokenCount).toBe(1);
    expect(next.skippedCount).toBe(1);
    expect(next.end).toBeNull();
  });

  it("accumulates counters across batches", () => {
    const first = applyCheckEvents(EMPTY_LINK_CHECK, [
      report({ type: "missing-file", target: "a" }),
    ]);
    const second = applyCheckEvents(first, [
      report({ type: "missing-file", target: "b" }),
      report({ type: "external-skipped", url: "https://x" }),
    ]);
    expect(second.reports).toHaveLength(3);
    expect(second.brokenCount).toBe(2);
    expect(second.skippedCount).toBe(1);
  });

  it("records the authoritative summary on check-end", () => {
    const next = applyCheckEvents(EMPTY_LINK_CHECK, [
      {
        type: "check-end",
        filesChecked: 12,
        totalLinks: 100,
        broken: 7,
        skipped: 3,
      },
    ]);
    expect(next.end).toEqual({
      filesChecked: 12,
      totalLinks: 100,
      broken: 7,
      skipped: 3,
    });
  });
});
