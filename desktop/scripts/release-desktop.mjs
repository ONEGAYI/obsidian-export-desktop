// Collect the built desktop installers, rename them to the release asset
// convention (spaces → dots), and upload them to an existing GitHub release.
//
// Usage: node scripts/release-desktop.mjs vX.Y.Z [--dry-run]
//
// Deliberately does NOT build: a dry run against an existing bundle must be
// possible without a rebuild. `just desktop-release vX.Y.Z` chains the build
// and this script. Runs through plain Node (no POSIX shell involved), same
// discipline as sync-sidecar.mjs — only the final `gh` call goes through a
// shell because that is simplest across platforms.
import { spawnSync } from "node:child_process";
import { readdirSync, renameSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const REPO = "ONEGAYI/obsidian-export-desktop";
// gh resolves to upstream when run without -R inside this checkout.
const root = resolve(import.meta.dirname, "..", "..");

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const tag = args.find((a) => !a.startsWith("--"));
if (!tag || !/^v\d+\.\d+\.\d+$/.test(tag)) {
  console.error("usage: node scripts/release-desktop.mjs vX.Y.Z [--dry-run]");
  process.exit(1);
}
const version = tag.slice(1);

// Tauri writes both installers under target/release/bundle:
//   msi/  "Obsidian Export_<version>_x64_en-US.msi"
//   nsis/ "Obsidian Export_<version>_x64-setup.exe"
const bundles = [
  ["msi", ".msi"],
  ["nsis", ".exe"],
];

const collected = [];
for (const [dir, ext] of bundles) {
  const dirPath = resolve(root, "desktop/src-tauri/target/release/bundle", dir);
  let names;
  try {
    names = readdirSync(dirPath);
  } catch {
    console.error(`missing bundle directory: ${dirPath}`);
    console.error("run `just desktop-build` first");
    process.exit(1);
  }
  // Old bundles accumulate across releases; only installers carrying the
  // tag's version are candidates. A missing candidate usually means the
  // bundle predates the version bump (v26.8.2 shipped mismatched names
  // once), which rebuilding after `just set-version` fixes.
  const candidates = names.filter(
    (name) => name.endsWith(ext) && name.includes(version),
  );
  if (candidates.length === 0) {
    console.error(
      `no ${ext} installer for ${version} in ${dirPath} — ` +
        "run `just desktop-build` after `just set-version`",
    );
    process.exit(1);
  }
  if (candidates.length > 1) {
    console.error(
      `ambiguous ${ext} installers for ${version}: ${candidates.join(", ")} — ` +
        "clean desktop/src-tauri/target/release/bundle and rebuild",
    );
    process.exit(1);
  }
  collected.push(join(dirPath, candidates[0]));
}

// Spaces in asset names are awkward on the command line and in download
// URLs; the release convention replaces them with dots (since v26.8.3).
// Idempotent: an already-dotted name maps to itself.
const assets = collected.map((file) => {
  const name = basename(file);
  const assetName = name.replaceAll(" ", ".");
  if (assetName === name) {
    return file;
  }
  const target = join(dirname(file), assetName);
  if (dryRun) {
    console.log(`[dry-run] would rename ${name} -> ${assetName}`);
  } else {
    renameSync(file, target);
    console.log(`renamed  ${name} -> ${assetName}`);
  }
  return target;
});

if (dryRun) {
  for (const asset of assets) {
    console.log(`[dry-run] would upload ${basename(asset)} to ${tag} (${REPO})`);
  }
  process.exit(0);
}

// shell: true — on Windows gh is a .cmd and cannot be spawned directly; the
// quoted paths keep spaces intact when the array is joined into a command
// line on either platform.
const result = spawnSync(
  "gh",
  [
    "release",
    "upload",
    tag,
    ...assets.map((asset) => `"${asset}"`),
    "--clobber",
    "-R",
    REPO,
  ],
  { stdio: "inherit", shell: true },
);
process.exit(result.status ?? 1);
