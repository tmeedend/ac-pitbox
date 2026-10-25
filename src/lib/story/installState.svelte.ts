// A story package being installed (SPEC-story-mode, Content: installation
// and updates), from a file, a link or an update - one at a time: what it
// downloads, the question an update asks when the game under way would not
// resume on it, and the cars and tracks the player lacked, imported after.
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { errorText } from "$lib/errors";
import { t } from "$lib/i18n/index.svelte";
import { importDownloadedArchive } from "$lib/workshop/importState.svelte";
import { forgetImported, type InstallOutcome } from "./play";

export const installing = $state<{
  busy: boolean;
  /** What is downloaded so far, in bytes, over what is to download. */
  progress: { received: number; total: number } | null;
  /** How the last install ended: a sentence to show, or an error. */
  notice: string | null;
  problem: string | null;
}>({ busy: false, progress: null, notice: null, problem: null });

/** Runs one install: `call` installs, asked again with `true` when the
 * player agrees to lose the game under way. */
export async function runInstall(call: (loseGame: boolean) => Promise<InstallOutcome>): Promise<void> {
  if (installing.busy) return;
  Object.assign(installing, { busy: true, progress: null, notice: null, problem: null });
  const stop = listen<{ received: number; total: number }>("story:download", (e) => {
    installing.progress = e.payload;
  });
  try {
    let outcome = await call(false);
    if (outcome.kind === "loses_game") {
      if (!(await confirm(t("story.play.losesGameConfirm", outcome), { kind: "warning" }))) return;
      outcome = await call(true);
    }
    if (outcome.kind === "installed") {
      const { title, version, previous } = outcome;
      installing.notice =
        previous === null
          ? t("story.play.installDone", { title, version })
          : t("story.play.updateDone", { title, version, previous });
      installing.progress = null;
      await importLacking(outcome.imports);
    }
  } catch (e) {
    if (String(e) === "errors.storyPackageCancelled") installing.notice = errorText(e);
    else installing.problem = errorText(e);
  } finally {
    void stop.then((unlisten) => unlisten());
    installing.busy = false;
    installing.progress = null;
  }
}

/** The cars and tracks the player lacked, through the ordinary import -
 * its report and its questions - one archive after the other. */
async function importLacking(archives: string[]): Promise<void> {
  for (const archive of archives) {
    const done = await importDownloadedArchive(archive);
    if (!done) {
      installing.problem = t("story.play.importBusy");
      return;
    }
    // A question left pending re-reads the archive when answered.
    if (!done.stillNeeded) await forgetImported(archive);
  }
}
