// Wraps the NSIS installer in a zip ready to upload to OverTake as is.
//
// OverTake does not accept a bare `.exe`, so each release used to mean zipping
// the installer by hand and renaming the archive. The name is the one GitHub
// gives the release asset — it turns the space of "Pit Box" into a dot — with
// `.zip` instead of `.exe`: `Pit.Box_0.8.0_x64-setup.zip`. The installer keeps
// its own name inside the archive.
//
// The installer is looked up by the version being built, not taken as "the
// .exe in the folder": `bundle/nsis/` keeps every installer ever built on this
// machine, and zipping a stale one would publish the wrong version under the
// right name.
//
// Run after `tauri build` (`npm run bundle` chains both). When GITHUB_OUTPUT is
// set, the zip path is written there as `zip` for the next workflow step.

import { readFileSync, readdirSync, existsSync, appendFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const NSIS_DIR = join(ROOT, "src-tauri", "target", "release", "bundle", "nsis");

const conf = JSON.parse(readFileSync(join(ROOT, "src-tauri", "tauri.conf.json"), "utf8"));
const prefix = `${conf.productName}_${conf.version}_`;

const candidates = existsSync(NSIS_DIR)
  ? readdirSync(NSIS_DIR).filter((f) => f.startsWith(prefix) && f.endsWith("-setup.exe"))
  : [];
if (candidates.length !== 1) {
  console.error(
    `[zip] expected exactly one "${prefix}*-setup.exe" in ${NSIS_DIR}, found ${candidates.length}` +
      (candidates.length ? `: ${candidates.join(", ")}` : " — run `npm run tauri build` first"),
  );
  process.exit(1);
}

const exe = join(NSIS_DIR, candidates[0]);
const zip = join(NSIS_DIR, candidates[0].replaceAll(" ", ".").replace(/\.exe$/, ".zip"));

// Compress-Archive rather than `tar -a`: under Git Bash, `tar` resolves to GNU
// tar, which does not write zip. Paths travel through the environment so that
// the space in the product name never meets PowerShell's quoting.
execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "$ErrorActionPreference = 'Stop'; Compress-Archive -LiteralPath $env:PB_EXE -DestinationPath $env:PB_ZIP -Force",
  ],
  { stdio: "inherit", env: { ...process.env, PB_EXE: exe, PB_ZIP: zip } },
);

console.log(`[zip] ${zip}`);
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `zip=${zip}\n`);
