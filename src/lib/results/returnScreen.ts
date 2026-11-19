// When the game closes after a session of the user's, the session zone opens
// on its result (RESULTAT§4). The backend brings the window back first.
import { nav, requestSection } from "$lib/shell/nav.svelte";
import { onSessionEnded } from "./results";

/** Follows the end of sessions for the life of the window; returns its stop. */
export function showResultOnReturn(): () => void {
  let stop: (() => void) | null = null;
  void onSessionEnded((launchId) => {
    nav.resultLaunch = launchId;
    nav.resultSession = null;
    void requestSection("result");
  }).then((off) => (stop = off));
  return () => stop?.();
}
