import { describe, expect, it } from "vitest";

import { EMPTY_PROGRESS, foldEvent } from "./App";

describe("foldEvent", () => {
  it("returns the state unchanged for schema events", () => {
    expect(foldEvent(EMPTY_PROGRESS, { type: "schema", version: 1 }, "w")).toBe(
      EMPTY_PROGRESS,
    );
  });

  it("records the file total on start", () => {
    const next = foldEvent(EMPTY_PROGRESS, { type: "start", total: 42 }, "w");
    expect(next.total).toBe(42);
  });

  it("counts done files and appends a log line with the file name", () => {
    const next = foldEvent(EMPTY_PROGRESS, { type: "file-done", path: "a/b.md" }, "w");
    expect(next.done).toBe(1);
    expect(next.lines).toEqual([{ kind: "done", text: "b.md" }]);
  });

  it("counts skipped files separately from done", () => {
    const next = foldEvent(
      EMPTY_PROGRESS,
      { type: "file-skipped", path: "a/b.md" },
      "w",
    );
    expect(next.skipped).toBe(1);
    expect(next.done).toBe(0);
  });

  it("records failures with the full path plus a base-name log line", () => {
    const next = foldEvent(
      EMPTY_PROGRESS,
      { type: "file-failed", path: "a/b.md", message: "boom" },
      "w",
    );
    expect(next.failures).toEqual([{ path: "a/b.md", message: "boom" }]);
    expect(next.lines).toEqual([
      { kind: "failed", text: "b.md", detail: "boom" },
    ]);
  });

  it("labels pathless warnings with the warning label", () => {
    const next = foldEvent(
      EMPTY_PROGRESS,
      { type: "warning", path: null, message: "careful" },
      "警告",
    );
    expect(next.warnings).toEqual([{ path: null, message: "careful" }]);
    expect(next.lines).toEqual([
      { kind: "warning", text: "警告", detail: "careful" },
    ]);
  });

  it("labels pathful warnings with the path's base name instead", () => {
    const next = foldEvent(
      EMPTY_PROGRESS,
      { type: "warning", path: "a/b.md", message: "careful" },
      "警告",
    );
    expect(next.lines).toEqual([
      { kind: "warning", text: "b.md", detail: "careful" },
    ]);
  });

  it("tracks the active diagram rendering slot", () => {
    const next = foldEvent(
      EMPTY_PROGRESS,
      { type: "diagram-render", language: "mermaid", index: 2, total: 5 },
      "w",
    );
    expect(next.diagram).toEqual({ index: 2, total: 5, language: "mermaid" });
  });

  it("marks the end seen and clears the diagram slot on end", () => {
    const rendering = foldEvent(
      EMPTY_PROGRESS,
      { type: "diagram-render", language: "mermaid", index: 1, total: 1 },
      "w",
    );
    const next = foldEvent(rendering, { type: "end", failed: [] }, "w");
    expect(next.endSeen).toBe(true);
    expect(next.diagram).toBeNull();
  });
});
