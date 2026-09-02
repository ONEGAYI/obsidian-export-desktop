import { describe, expect, it } from "vitest";

import { fmt } from "./index";
import { en } from "./en";
import { zh } from "./zh";

describe("fmt", () => {
  it("replaces matched placeholders with params", () => {
    expect(fmt("a {x} b {y}", { x: 1, y: "z" })).toBe("a 1 b z");
  });

  it("keeps placeholders that have no matching param verbatim", () => {
    expect(fmt("{x} {y}", { x: "1" })).toBe("1 {y}");
  });

  it("ignores params without a matching placeholder", () => {
    expect(fmt("{x}", { x: "1", unused: "u" })).toBe("1");
  });

  it("returns a placeholder-free template unchanged", () => {
    expect(fmt("plain text", {})).toBe("plain text");
  });
});

/** Sorted placeholder names of a dictionary template, e.g. "{a} x {b}" → ["a", "b"]. */
function placeholders(template: string): string[] {
  return [...template.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

/** Collect every string leaf as `path → sorted placeholders`. */
function collectLeaves(
  value: unknown,
  path: string,
  out: Map<string, string[]>,
): void {
  if (typeof value === "string") {
    out.set(path, placeholders(value));
  } else if (value !== null && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      collectLeaves(child, path ? `${path}.${key}` : key, out);
    }
  }
}

// The two dictionaries are kept structurally identical by the `Dict` type
// (`Widen<typeof zh>`), but the type cannot see into template strings: a
// translated entry that drops or renames a `{name}` placeholder leaves `fmt`
// silently emitting the raw template. This test locks the placeholder sets
// of both dictionaries together, order-insensitively (translation may
// legitimately reorder placeholders).
describe("dictionary placeholder parity (zh ↔ en)", () => {
  it("covers the same leaf paths in both dictionaries", () => {
    const zhLeaves = new Map<string, string[]>();
    const enLeaves = new Map<string, string[]>();
    collectLeaves(zh, "", zhLeaves);
    collectLeaves(en, "", enLeaves);
    expect([...enLeaves.keys()].sort()).toEqual([...zhLeaves.keys()].sort());
  });

  it("uses the same placeholder set for every leaf entry", () => {
    const zhLeaves = new Map<string, string[]>();
    const enLeaves = new Map<string, string[]>();
    collectLeaves(zh, "", zhLeaves);
    collectLeaves(en, "", enLeaves);
    for (const [path, expected] of zhLeaves) {
      expect(enLeaves.get(path), `placeholder mismatch at ${path}`).toEqual(
        expected,
      );
    }
  });

  it("sanity-checks the extractor against known entries", () => {
    const zhLeaves = new Map<string, string[]>();
    collectLeaves(zh, "", zhLeaves);
    expect(zhLeaves.get("theme.toggleLabel")).toEqual(["current", "next"]);
    expect(zhLeaves.get("options.updateAvailable")).toEqual(["version"]);
  });
});
