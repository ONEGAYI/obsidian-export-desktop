// Build the CLI and copy it to the Tauri sidecar location
// (desktop/src-tauri/binaries/obsidian-export-<target-triple>[.exe]).
//
// Runs through plain Node on the host (no POSIX shell involved), so it works
// the same from PowerShell, cmd, or any terminal where cargo is on PATH.
import { execSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..", "..");
const exe = process.platform === "win32" ? ".exe" : "";

const rustcVv = execSync("rustc -vV", { cwd: root }).toString();
const triple = rustcVv
  .split("\n")
  .find((line) => line.startsWith("host:"))
  ?.slice("host:".length)
  .trim();
if (!triple) {
  throw new Error(`could not read host triple from rustc -vV:\n${rustcVv}`);
}

execSync("cargo build --release --bin obsidian-export", {
  cwd: root,
  stdio: "inherit",
});

const destDir = resolve(root, "desktop", "src-tauri", "binaries");
mkdirSync(destDir, { recursive: true });
const dest = resolve(destDir, `obsidian-export-${triple}${exe}`);
copyFileSync(resolve(root, `target/release/obsidian-export${exe}`), dest);

console.log(`Sidecar synced -> desktop/src-tauri/binaries/obsidian-export-${triple}${exe}`);
