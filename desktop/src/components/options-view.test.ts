import { describe, expect, it } from "vitest";

import { nextTabIndex } from "./OptionsView";

describe("nextTabIndex", () => {
  it("moves to the next tab and wraps at the end", () => {
    expect(nextTabIndex(0, "ArrowRight", 6)).toBe(1);
    expect(nextTabIndex(5, "ArrowRight", 6)).toBe(0);
  });

  it("moves to the previous tab and wraps at the start", () => {
    expect(nextTabIndex(1, "ArrowLeft", 6)).toBe(0);
    expect(nextTabIndex(0, "ArrowLeft", 6)).toBe(5);
  });

  it("treats the vertical arrow keys as the same moves", () => {
    expect(nextTabIndex(0, "ArrowDown", 6)).toBe(1);
    expect(nextTabIndex(0, "ArrowUp", 6)).toBe(5);
  });

  it("jumps to the ends with Home and End", () => {
    expect(nextTabIndex(3, "Home", 6)).toBe(0);
    expect(nextTabIndex(3, "End", 6)).toBe(5);
  });

  it("keeps the focus for unrelated keys", () => {
    expect(nextTabIndex(2, "Enter", 6)).toBeNull();
    expect(nextTabIndex(2, "Tab", 6)).toBeNull();
    expect(nextTabIndex(2, "a", 6)).toBeNull();
  });

  it("returns null for an empty tab list", () => {
    expect(nextTabIndex(0, "ArrowRight", 0)).toBeNull();
  });
});
