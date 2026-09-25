// Code weight — **a report, never a gate.**
//
// **Why.** Nobody reads the diffs of this project (CLAUDE.md, "Personne ne
// relit le code"), so a file that swells one feature at a time is noticed by
// no one — the same way `SPEC.md` reached 2 448 lines before anyone saw it
// (`report-docs.mjs`). This prints, on every `npm run check`, the heaviest
// source files, the heavy files that grew on the current branch, and the long
// Rust functions in the files the branch touched: the refactoring candidates
// CLAUDE.md asks to flag in every end-of-task report. It also says when a
// refactoring review is due (`--review-done` resets that counter).
//
// **No threshold fails anything.** A gate that refuses a commit because a file
// grew by ten lines ends up disabled — the lesson of `check-conventions.mjs`.
// The thresholds below only decide what gets *printed*; they match the ones
// written in CLAUDE.md, and must move together with them.
//
// Always exits 0: this script is a `report:`, not a `check:`.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";

const FILE_LINES = 1500;
const FN_LINES = 80;
const EXTENSIONS = /\.(rs|ts|svelte|js|mjs)$/;
const ROOTS = ["src/", "src-tauri/src/", "src-tauri/crates/", "scripts/"];
const EXCLUDED = /(^|\/)generated\//;

function git(...args) {
  try {
    return execFileSync("git", args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return null;
  }
}

// Refactoring review pacing: see the comment above the review report below.
const REVIEW_EVERY = 50;
const REVIEW_REF = "refs/pitbox/refactoring-review";

if (process.argv.includes("--review-done")) {
  git("update-ref", REVIEW_REF, "HEAD");
  console.log(`[code] refactoring review marker moved to HEAD: next one in ${REVIEW_EVERY} commits`);
  process.exit(0);
}

const isSource = (f) => EXTENSIONS.test(f) && ROOTS.some((r) => f.startsWith(r)) && !EXCLUDED.test(f);
const lineCount = (f) => readFileSync(f, "utf8").split(/\r?\n/).length;
const fmt = (n) => n.toLocaleString("fr-FR");
const short = (f) => f.split("/").pop();

// Tracked files plus new ones not yet added: a file created in this session
// must weigh from its first `npm run check`, not from its first commit.
const listed = [git("ls-files"), git("ls-files", "--others", "--exclude-standard")]
  .filter(Boolean)
  .join("\n")
  .split("\n");
const files = [...new Set(listed)].filter((f) => isSource(f) && existsSync(f));
const sizes = new Map(files.map((f) => [f, lineCount(f)]));

const total = [...sizes.values()].reduce((a, b) => a + b, 0);
const heaviest = [...sizes].sort((a, b) => b[1] - a[1]).slice(0, 3);
console.log(`[code] ${files.length} source files, ${fmt(total)} lines`);
console.log(`[code] heaviest: ${heaviest.map(([f, n]) => `${short(f)} (${fmt(n)})`).join(" · ")}`);

// Refactoring review, paced by activity rather than by the calendar: this is a
// hobby project, with weeks at 100 commits and whole months at zero (summer
// 2026). A weekly schedule would review an idle codebase; counting commits
// asks only when there is something new to look at. 50 commits ≈ one active
// week at the measured pace. The marker is a plain ref — shared by every
// worktree, never pushed — moved by `npm run review:done` once the user has
// answered, yes or no (CLAUDE.md, « Revue de refactoring »). No marker (CI, a
// fresh clone): nothing to say.
const reviewed = git("rev-parse", "--verify", "-q", REVIEW_REF);
if (reviewed) {
  const since = Number(git("rev-list", "--count", "--no-merges", `${REVIEW_REF}..HEAD`) ?? 0);
  const when = git("log", "-1", "--format=%as", REVIEW_REF);
  console.log(
    since >= REVIEW_EVERY
      ? `[code] refactoring review due: ${since} commits since the last one (${when}) — ask the user (CLAUDE.md, « Revue de refactoring »)`
      : `[code] refactoring review: ${since}/${REVIEW_EVERY} commits since the last one (${when})`,
  );
}

// Growth is measured against the fork point with main, working tree included,
// so uncommitted work counts too. On main itself the fork point is HEAD, which
// leaves exactly the uncommitted changes. A shallow CI clone may have no main
// at all: the growth part is then skipped rather than guessed.
const base = git("merge-base", "HEAD", "main") ?? git("merge-base", "HEAD", "origin/main");
if (!base) {
  console.log("[code] no main branch to compare with: growth not reported");
  process.exit(0);
}

const growth = new Map();
for (const row of (git("diff", "--numstat", base) ?? "").split("\n")) {
  const [added, deleted, file] = row.split("\t");
  if (file && sizes.has(file) && added !== "-") growth.set(file, Number(added) - Number(deleted));
}
for (const f of (git("ls-files", "--others", "--exclude-standard") ?? "").split("\n")) {
  if (sizes.has(f)) growth.set(f, sizes.get(f));
}

const swelling = [...growth]
  .filter(([f, delta]) => delta > 0 && sizes.get(f) > FILE_LINES)
  .sort((a, b) => b[1] - a[1]);
if (swelling.length) {
  const list = swelling.map(([f, d]) => `${short(f)} ${fmt(sizes.get(f))} (+${d})`).join(" · ");
  console.log(`[code] over ${fmt(FILE_LINES)} lines and still growing on this branch: ${list}`);
}

// Long functions, Rust only, and only in the files this branch touched: the
// whole codebase would print the same list forever and stop being read.
// Brace counting on lines stripped of strings and comments — an estimate, which
// a multi-line raw string can skew; good enough for a report.
function longRustFunctions(file) {
  const found = [];
  let depth = 0;
  let current = null;
  for (const [i, raw] of readFileSync(file, "utf8").split(/\r?\n/).entries()) {
    if (!current && depth === 0 && /^\s*#\[cfg\(test\)\]/.test(raw)) break;
    const line = raw
      .replace(/\/\/.*$/, "")
      .replace(/"(?:\\.|[^"\\])*"/g, '""')
      .replace(/'(?:\\.|[^'\\])'/g, "''");
    const head = /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+)*fn\s+(\w+)/.exec(line);
    if (!current && head) current = { name: head[1], start: i, opened: false, depth };
    for (const ch of line) {
      if (ch === "{") {
        depth++;
        if (current) current.opened = true;
      } else if (ch === "}") depth--;
    }
    if (current && !current.opened && line.includes(";")) current = null; // trait method declaration
    else if (current?.opened && depth === current.depth) {
      const length = i - current.start + 1;
      if (length > FN_LINES) found.push({ name: current.name, length });
      current = null;
    }
  }
  return found;
}

const longFns = [...growth.keys()]
  .filter((f) => f.endsWith(".rs"))
  .flatMap((f) => longRustFunctions(f).map((fn) => ({ ...fn, file: f })))
  .sort((a, b) => b.length - a.length);
if (longFns.length) {
  const shown = longFns.slice(0, 5).map((fn) => `${short(fn.file)}::${fn.name} (${fn.length})`);
  const more = longFns.length > 5 ? ` · +${longFns.length - 5} more` : "";
  console.log(`[code] functions over ${FN_LINES} lines in files touched by this branch: ${shown.join(" · ")}${more}`);
}
