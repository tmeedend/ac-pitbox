// The page's marks for the startup timing (`timing.rs`), off unless the app
// was started with `PITBOX_TIMING` - which only `npm run bench:startup` does.
//
// The clock is read when the mark is set, not when the backend receives it:
// at startup the IPC is busy, and a mark delivered late would say the page got
// there later than it did.

import { invoke } from "@tauri-apps/api/core";

let enabled: Promise<boolean> | null = null;

/** Records that the page reached `label` (written `front.<label>`). Asks the
 * backend once whether timing is on; off, nothing more is ever sent. */
export function mark(label: string): void {
  const webview = performance.now();
  enabled ??= invoke<boolean>("timing_enabled").catch(() => false);
  void enabled.then((on) => {
    if (on) void invoke("timing_mark", { label, webview }).catch(() => {});
  });
}
