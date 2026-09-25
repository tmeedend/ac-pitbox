// Claude Code PreToolUse hook (.claude/settings.json): runs `npm run check`
// before any `git commit` issued through the Bash or PowerShell tool, and
// blocks the commit when it fails.
//
// **Why a hook and not a line in CLAUDE.md.** CLAUDE.md already says to run
// the checks before committing; a written rule gets forgotten, which is the
// whole reason `check-conventions.mjs` exists. Nobody reviews the diffs of
// this project, so the commit is the last point where a red check can still
// be caught before it lands on main.
//
// Only `npm run check` (~15 s), not `npm run verify` (clippy + cargo test,
// minutes): a hook that slow would get disabled. `verify` stays the end-of-task
// step of CLAUDE.md, and the CI runs it anyway.
//
// Exit 2 = blocking error: stderr is fed back to Claude, the commit does not
// run. An unreadable hook input exits 0 — a hook confused about its own input
// must not make committing impossible.

import { execSync } from "node:child_process";

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  let command = "";
  try {
    command = JSON.parse(input)?.tool_input?.command ?? "";
  } catch {
    process.exit(0);
  }
  // Anywhere in the command line: `git add -A && git commit -m …` must match
  // as much as a bare `git commit`.
  if (!/\bgit\s+(?:-C\s+\S+\s+)?commit\b/.test(command)) process.exit(0);

  const cwd = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  try {
    const out = execSync("npm run check", { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    // The code weight report goes back to Claude as context: the moment of the
    // commit is when the "Refactoring" section of the report gets written.
    const report = out.split(/\r?\n/).filter((l) => l.startsWith("[code]"));
    if (report.length) {
      const hookSpecificOutput = { hookEventName: "PreToolUse", additionalContext: report.join("\n") };
      process.stdout.write(JSON.stringify({ hookSpecificOutput }));
    }
    process.exit(0);
  } catch (e) {
    const out = `${e.stdout ?? ""}\n${e.stderr ?? ""}`.trim().split(/\r?\n/);
    process.stderr.write(
      "Commit blocked: `npm run check` failed. Fix the errors below, never weaken the check " +
        "(CLAUDE.md, « Ne jamais affaiblir un contrôle pour le faire passer »).\n\n" +
        out.slice(-40).join("\n") +
        "\n",
    );
    process.exit(2);
  }
});
