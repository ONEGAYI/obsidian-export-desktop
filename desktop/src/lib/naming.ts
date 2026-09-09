import { baseName } from "@/lib/sidecar";

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
