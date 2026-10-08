// The boot screen (§13): what the window shows from its first frame until the
// shell is drawn at its final zoom.
//
// Before it, a start went through four looks in a row - white (the webview
// before any CSS), black (the global stylesheet, nothing mounted), the shell
// at 100 %, then the shell again at the user's zoom, applied once the config
// came back. The window's own background is now dark (`tauri.conf.json`), the
// screen is plain HTML in `app.html` - there before any script runs - and it
// only goes once the prefs are applied and the first frame is painted.

import { setLocale } from "$lib/i18n/index.svelte";
import { setZoom, zoomFactor } from "$lib/shell/zoom.svelte";
import { mark } from "$lib/timing";

/** Language and zoom of the user, applied before anything is mounted: the
 * shell's first frame is already the right one.
 *
 * The boot screen is already on screen by then - the zoom lives in
 * `config.json`, which no page can read before its script runs - and the zoom
 * set on `<html>` scaled it up under the user's eyes (seen: the logo growing
 * a moment before the shell appeared). CSS `zoom` compounds, so the boot
 * screen takes the inverse in the same task, before any paint: measured, its
 * logo stays 46 px at 125 %, centred, and it still covers the window. */
export function applyStartupPrefs(prefs: { language: string | null; ui_zoom: number | null }): void {
  if (prefs.language) setLocale(prefs.language);
  setZoom(prefs.ui_zoom);
  const boot = document.getElementById("boot");
  if (boot) boot.style.zoom = String(1 / zoomFactor());
}

/** Fades the boot screen out once what replaced it has been painted. Two
 * frames: the first is the one Svelte's mount lands in, the second the one
 * after it was actually drawn. */
export function dismissBootScreen(): void {
  const boot = document.getElementById("boot");
  if (!boot) return;
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      boot.classList.add("done");
      mark("boot_dismissed");
      // `transitionend` does not come when the transition is skipped (reduced
      // motion, a hidden window): the timer removes it all the same.
      setTimeout(() => boot.remove(), 400);
    }),
  );
}
