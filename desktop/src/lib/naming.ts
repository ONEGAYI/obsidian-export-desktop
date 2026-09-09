/**
 * Reduce a full path to its final component for compact log lines and
 * summaries. Accepts both separators; a trailing separator yields an empty
 * string (callers that must not show blank trim it first, like
 * [`displayName`]). Lives here — with the other path-to-name helpers —
 * rather than in the sidecar invoke layer.
 */
export function baseName(path: string): string {
  const normalized = path.split("\\").join("/");
  const idx = normalized.lastIndexOf("/");
  return idx === -1 ? normalized : normalized.slice(idx + 1);
}

/**
 * Display name for a path input on the home card: the final component after
 * trimming trailing separators. Returns `null` for blank input (the caller
 * shows a placeholder instead of inventing a vault). Paths without a usable
 * name (e.g. a drive root like `C:`) fall back to the trimmed input itself.
 */
export function displayName(path: string): string | null {
  const trimmed = path.trim().replace(/[\\/]+$/, "");
  if (trimmed === "") {
    return null;
  }
  return baseName(trimmed) || trimmed;
}
