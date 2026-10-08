// Measures Pit Box's startup in release, several times, and compares with the
// previous measure: `npm run bench:startup` (docs/CHANTIERS.md, "Performances").
//
//   npm run bench:startup                     build, then 5 runs
//   npm run bench:startup -- --no-build       runs on the last build
//   npm run bench:startup -- --runs 1         one run (a cold measure: right
//                                             after restarting Windows)
//
// The app writes its own timings when `PITBOX_TIMING` names a file
// (`src-tauri/src/timing.rs`, `src/lib/timing.ts`); this script only launches
// it, waits for the cards, closes it and sums up. Results are kept, per
// machine, in `.bench/` (not versioned), and each run is compared with the
// last one kept.
//
// **It runs the real app on the real library**, so two precautions:
// - it refuses to start while Pit Box is open - two instances on one database
//   is how `overlay.sqlite` got corrupted twice;
// - every launch makes a startup backup, and only the last 7 are kept: five
//   runs would push five real snapshots out (`backup.rs`, the very trap seven
//   `tauri dev` restarts fell into one morning). The backups folder is copied
//   aside first, and put back exactly as it was afterwards.
//
// The measure is end to end on purpose - the cards on screen - never a
// command's own duration alone: commands share the base lock, and one that no
// longer blocks the others can look slower while the screen is faster.

import { readFileSync, readdirSync, existsSync, mkdirSync, writeFileSync, rmSync, cpSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { spawn, spawnSync, execFileSync } from "node:child_process";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const EXE = join(ROOT, "src-tauri", "target", "release", "pitbox.exe");
const RESULTS = join(ROOT, ".bench");

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? Number(args[i + 1]) : fallback;
};
const RUNS = option("--runs", 5);
const TIMEOUT_MS = option("--timeout", 30) * 1000;
const BUILD = !args.includes("--no-build");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function fail(message) {
  console.error(`[bench] ${message}`);
  process.exit(1);
}

if (process.platform !== "win32") fail("Windows uniquement.");

const running = execFileSync("tasklist", ["/FI", "IMAGENAME eq pitbox.exe", "/NH"], { encoding: "utf8" });
if (running.toLowerCase().includes("pitbox.exe")) {
  fail("Pit Box est ouvert — le fermer d'abord (deux instances sur la même base l'ont déjà abîmée).");
}

if (BUILD) {
  console.log("[bench] compilation release (sans installateur)…");
  const built = spawnSync("npx", ["tauri", "build", "--no-bundle"], { cwd: ROOT, stdio: "inherit", shell: true });
  if (built.status !== 0) fail("la compilation a échoué.");
}
if (!existsSync(EXE)) fail(`${EXE} introuvable — lancer sans --no-build.`);

const identifier = JSON.parse(readFileSync(join(ROOT, "src-tauri", "tauri.conf.json"), "utf8")).identifier;
const BACKUPS = join(process.env.APPDATA ?? "", identifier, "backups");

// --- The backups, set aside and put back as they were ---

function setBackupsAside() {
  if (!existsSync(BACKUPS)) return null;
  const copy = join(tmpdir(), `pitbox-bench-backups-${Date.now()}`);
  cpSync(BACKUPS, copy, { recursive: true });
  return { copy, before: new Set(readdirSync(BACKUPS)) };
}

function putBackupsBack(saved) {
  if (!saved || !existsSync(BACKUPS)) return;
  const now = new Set(readdirSync(BACKUPS));
  // Only what the runs made goes: a snapshot whose name was not there before.
  for (const name of now) if (!saved.before.has(name)) rmSync(join(BACKUPS, name), { recursive: true, force: true });
  // And what their rotation pushed out comes back.
  for (const name of saved.before) {
    if (!now.has(name)) cpSync(join(saved.copy, name), join(BACKUPS, name), { recursive: true });
  }
  rmSync(saved.copy, { recursive: true, force: true });
}

// --- One run ---

function readLines(file) {
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim())
    .flatMap((l) => {
      try {
        return [JSON.parse(l)];
      } catch {
        return [];
      }
    });
}

const cardsOf = (lines) => lines.find((l) => l.label.startsWith("front.cards."));

async function run(i) {
  const file = join(tmpdir(), `pitbox-bench-${process.pid}-${i}.jsonl`);
  rmSync(file, { force: true });
  const app = spawn(EXE, [], { env: { ...process.env, PITBOX_TIMING: file }, stdio: "ignore" });
  const started = Date.now();
  let lines = [];
  while (Date.now() - started < TIMEOUT_MS) {
    await sleep(100);
    lines = readLines(file);
    if (cardsOf(lines) && lines.some((l) => l.label === "front.logos")) break;
  }
  // The last commands of the startup still write: a moment more, then closed.
  await sleep(500);
  lines = readLines(file);
  try {
    execFileSync("taskkill", ["/PID", String(app.pid), "/T", "/F"], { stdio: "ignore" });
  } catch {
    // Already gone.
  }
  rmSync(file, { force: true });
  await sleep(1500);
  return lines;
}

// --- From lines to numbers ---

/** One run's measures, every time on the process's clock (ms since start).
 * A page mark carries the page's own clock: the offset between the two is
 * taken from the mark delivered the soonest, the IPC being busy at startup. */
function measuresOf(lines) {
  const front = lines.filter((l) => l.webview != null);
  const offset = front.length ? Math.min(...front.map((l) => l.t - l.webview)) : 0;
  const first = (label) => lines.find((l) => l.label === label);
  const at = (l) => (l ? (l.webview != null ? l.webview + offset : l.t) : null);
  const setup = {};
  for (const l of lines) if (l.label.startsWith("setup.") && l.ms != null) setup[l.label.slice(6)] = l.ms;
  return {
    headline: {
      "préparation terminée": at(first("setup.end")),
      "écran de chargement levé": at(first("front.boot_dismissed")),
      "cartes à l'écran": at(cardsOf(lines)),
      "logos des marques": at(first("front.logos")),
      "list_library": first("cmd.list_library")?.ms ?? null,
      "get_brand_logos": first("cmd.get_brand_logos")?.ms ?? null,
      "list_other_mods": first("cmd.list_other_mods")?.ms ?? null,
    },
    cards: first("cmd.list_library.cards")?.n ?? null,
    setup,
  };
}

function stats(values) {
  const v = values.filter((x) => x != null).sort((a, b) => a - b);
  if (!v.length) return null;
  return { median: v[Math.floor(v.length / 2)], min: v[0], max: v[v.length - 1] };
}

const ms = (x) => (x == null ? "—" : `${Math.round(x)} ms`);

// --- Main ---

const saved = setBackupsAside();
const runs = [];
try {
  for (let i = 1; i <= RUNS; i++) {
    process.stdout.write(`[bench] lancement ${i}/${RUNS}… `);
    const m = measuresOf(await run(i));
    runs.push(m);
    console.log(`cartes à ${ms(m.headline["cartes à l'écran"])}`);
  }
} finally {
  putBackupsBack(saved);
}

const headline = {};
for (const key of Object.keys(runs[0].headline)) headline[key] = stats(runs.map((r) => r.headline[key]));
const setup = {};
for (const key of new Set(runs.flatMap((r) => Object.keys(r.setup)))) setup[key] = stats(runs.map((r) => r.setup[key]));
const cards = runs.find((r) => r.cards != null)?.cards ?? null;

mkdirSync(RESULTS, { recursive: true });
const previous = readdirSync(RESULTS)
  .filter((f) => f.startsWith("startup-") && f.endsWith(".json"))
  .sort()
  .at(-1);
const before = previous ? JSON.parse(readFileSync(join(RESULTS, previous), "utf8")) : null;

console.log(`\n[bench] ${RUNS} lancement(s), ${cards ?? "?"} cartes — médiane (min–max) depuis le lancement`);
for (const [key, s] of Object.entries(headline)) {
  const was = before?.headline?.[key]?.median;
  const delta = s && was != null ? `   (avant : ${ms(was)}, ${s.median - was >= 0 ? "+" : ""}${Math.round(s.median - was)} ms)` : "";
  console.log(`  ${key.padEnd(26)} ${s ? `${ms(s.median)} (${ms(s.min)}–${ms(s.max)})` : "—"}${delta}`);
}
if (!headline["cartes à l'écran"]) {
  console.log("  (pas de cartes : l'app s'est rouverte ailleurs que sur une bibliothèque — la laisser sur Voitures)");
}
const slowest = Object.entries(setup)
  .filter(([, s]) => s)
  .sort((a, b) => b[1].median - a[1].median)
  .slice(0, 5);
console.log(`  préparation, étapes les plus longues : ${slowest.map(([k, s]) => `${k} ${ms(s.median)}`).join(" · ")}`);
if (before && before.cards !== cards) {
  console.log(`  la bibliothèque a changé depuis la mesure précédente (${before.cards} → ${cards} cartes)`);
}

// Local time, the one the user reads on the clock; sorts like the name.
const d = new Date();
const two = (n) => String(n).padStart(2, "0");
const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}_${two(d.getHours())}-${two(d.getMinutes())}-${two(d.getSeconds())}`;
const out = join(RESULTS, `startup-${stamp}.json`);
writeFileSync(out, JSON.stringify({ date: new Date().toISOString(), runs: RUNS, cards, headline, setup }, null, 2));
console.log(`\n[bench] résultat gardé dans .bench/startup-${stamp}.json${before ? `, comparé à ${previous}` : ""}`);
