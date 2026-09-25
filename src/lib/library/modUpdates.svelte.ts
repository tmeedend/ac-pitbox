// Mod updates (§4.7): which library mods have a newer version in the CUP
// registry — the one Content Manager reads — and the one-click update.
//
// Lives here, at module level, for the reason the import state does: the check
// runs in the background, its result is read by three places at once (the
// toast, the library cards, the fiche), and an update started from a fiche
// keeps going when the fiche is closed.
//
// The update itself is two steps the user sees as one: the backend downloads
// the archive, then the **ordinary import** takes it (`importDownloadedArchive`)
// — same report, same arbitrations, same history line as a drop on the window.
// Nothing here knows how a mod is replaced.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errorText } from "$lib/errors";
import { StorageKey } from "$lib/storage";
import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
import { importDownloadedArchive, importState } from "$lib/workshop/importState.svelte";
import type { ModKind } from "./library";

/** Mirrors `cup::ModUpdate`. */
export interface ModUpdate {
  kind: ModKind;
  id: string;
  name: string | null;
  /** `null` when the mod's `ui_*.json` gives no version. */
  installed: string | null;
  available: string;
  /** Paid or gated content: the update opens the author's page. */
  limited: boolean;
}

/** Mirrors `cup::UpdateDetails`. */
export interface UpdateDetails {
  version: string | null;
  author: string | null;
  changelog: string | null;
  informationUrl: string | null;
  alternativeIds: string[];
}

type DownloadOutcome = { status: "archive"; path: string } | { status: "browser"; url: string } | { status: "cancelled" };

/** Mirrors `commands::updates::UpdateProgress`. */
interface UpdateProgress {
  id: string;
  received: number;
  total: number | null;
}

/** First check a minute after startup — the library is loaded and the
 * machine has settled — then once a day for as long as the app stays open.
 * The registry changes a few times a day at most, and one request a day is
 * what "without anyone thinking about it" costs. */
const FIRST_CHECK_MS = 60_000;
const CHECK_EVERY_MS = 24 * 60 * 60 * 1000;

export function updateKey(u: { kind: ModKind; id: string }): string {
  return `${u.kind}/${u.id}`;
}

export const modUpdates = $state<{
  /** Updates not ignored by the user, sorted by name. */
  list: ModUpdate[];
  /** `key → version` the user chose to skip (`StorageKey.modUpdatesIgnored`). */
  ignored: Record<string, string>;
  /** `key → version` a toast already announced (`StorageKey.modUpdatesAnnounced`). */
  announced: Record<string, string>;
  checking: boolean;
  /** Last check: when, and the error when it failed — shown in Settings. */
  lastCheck: { at: number; error: string | null } | null;
  /** The update in progress — one at a time, like the import it feeds. */
  busy: { kind: ModKind; id: string; name: string; phase: "download" | "import"; received: number; total: number | null } | null;
  cancelling: boolean;
  /** Per key: the download failed (message) or went to the browser. Cleared
   * at the next attempt. */
  outcome: Record<string, { error: string } | { browser: true }>;
}>({
  list: [],
  ignored: {},
  announced: {},
  checking: false,
  lastCheck: null,
  busy: null,
  cancelling: false,
  outcome: {},
});

/** Updates not announced yet — what the toast shows. */
export function unannounced(): ModUpdate[] {
  return modUpdates.list.filter((u) => modUpdates.announced[updateKey(u)] !== u.available);
}

/** The update of one mod, if there is one — for a card or a fiche. */
export function updateFor(kind: ModKind, id: string): ModUpdate | null {
  return modUpdates.list.find((u) => u.kind === kind && u.id === id) ?? null;
}

function parseMap(raw: string | null): Record<string, string> {
  if (!raw) return {};
  try {
    const v = JSON.parse(raw);
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, string>) : {};
  } catch {
    return {};
  }
}

let prefsLoaded: Promise<void> | null = null;

function loadPrefs(): Promise<void> {
  prefsLoaded ??= getUiPrefs([StorageKey.modUpdatesIgnored, StorageKey.modUpdatesAnnounced]).then((p) => {
    modUpdates.ignored = parseMap(p[StorageKey.modUpdatesIgnored]);
    modUpdates.announced = parseMap(p[StorageKey.modUpdatesAnnounced]);
  });
  return prefsLoaded;
}

/** Asks the registry now. Never throws: a failed check keeps the previous
 * list (an update known this morning is still real tonight) and records the
 * error for the Settings screen. */
export async function checkModUpdates(): Promise<void> {
  if (modUpdates.checking) return;
  modUpdates.checking = true;
  try {
    await loadPrefs();
    const found = await invoke<ModUpdate[]>("check_mod_updates");
    modUpdates.list = found.filter((u) => modUpdates.ignored[updateKey(u)] !== u.available);
    // Only what is still pending stays announced: once a mod is updated, its
    // entry would otherwise sit in the file forever.
    const current = new Set(found.map(updateKey));
    const kept = Object.fromEntries(Object.entries(modUpdates.announced).filter(([k]) => current.has(k)));
    if (Object.keys(kept).length !== Object.keys(modUpdates.announced).length) {
      modUpdates.announced = kept;
      setUiPref(StorageKey.modUpdatesAnnounced, JSON.stringify(kept));
    }
    modUpdates.lastCheck = { at: Date.now(), error: null };
  } catch (e) {
    console.error("check_mod_updates", e);
    modUpdates.lastCheck = { at: Date.now(), error: errorText(e) };
  } finally {
    modUpdates.checking = false;
  }
}

/** The toast was closed: what it showed will not be announced again. The
 * updates stay on the cards and fiches until installed or ignored. */
export function markAnnounced(): void {
  const next = { ...modUpdates.announced };
  for (const u of modUpdates.list) next[updateKey(u)] = u.available;
  modUpdates.announced = next;
  setUiPref(StorageKey.modUpdatesAnnounced, JSON.stringify(next));
}

/** Skips this version of this mod. A newer one is announced again — the same
 * "ignore this update" as Content Manager's. */
export function ignoreUpdate(u: ModUpdate): void {
  const next = { ...modUpdates.ignored, [updateKey(u)]: u.available };
  modUpdates.ignored = next;
  setUiPref(StorageKey.modUpdatesIgnored, JSON.stringify(next));
  modUpdates.list = modUpdates.list.filter((x) => updateKey(x) !== updateKey(u));
}

export function modUpdateDetails(u: ModUpdate): Promise<UpdateDetails> {
  return invoke<UpdateDetails>("mod_update_details", { kind: u.kind, id: u.id });
}

export function cancelUpdateDownload(): void {
  modUpdates.cancelling = true;
  invoke("cancel_mod_update_download").catch((e) => console.error("cancel_mod_update_download", e));
}

/** Whether an update can start now: one at a time, and never while an import
 * runs — the download ends in an import, and the backend admits one. */
export function canStartUpdate(): boolean {
  return !modUpdates.busy && !importState.importing;
}

/**
 * Updates one mod: download, then import. When what the registry points to
 * is not an archive (a file host's page, a Patreon post), the page opens in
 * the browser instead, and the user drops the archive once they have it.
 */
export async function installUpdate(u: ModUpdate): Promise<void> {
  if (!canStartUpdate()) return;
  const key = updateKey(u);
  const outcomes = { ...modUpdates.outcome };
  delete outcomes[key];
  modUpdates.outcome = outcomes;
  modUpdates.cancelling = false;
  modUpdates.busy = { kind: u.kind, id: u.id, name: u.name ?? u.id, phase: "download", received: 0, total: null };
  try {
    const outcome = await invoke<DownloadOutcome>("download_mod_update", { kind: u.kind, id: u.id });
    if (outcome.status === "browser") {
      modUpdates.outcome = { ...modUpdates.outcome, [key]: { browser: true } };
      await openUrl(outcome.url);
      return;
    }
    if (outcome.status === "cancelled") return;
    modUpdates.busy.phase = "import";
    const done = await importDownloadedArchive(outcome.path);
    // Kept only while an arbitration still needs it; otherwise gone at once.
    // A file left behind by a crash is swept at the next startup.
    if (!done?.stillNeeded) {
      invoke("discard_mod_update_download", { path: outcome.path }).catch((e) =>
        console.error("discard_mod_update_download", e),
      );
    }
    // The registry is asked again rather than the entry dropped by hand: an
    // import that stopped on a question has not updated anything yet, and
    // only the installed version can say.
    await checkModUpdates();
  } catch (e) {
    console.error("installUpdate", e);
    modUpdates.outcome = { ...modUpdates.outcome, [key]: { error: errorText(e) } };
  } finally {
    modUpdates.busy = null;
    modUpdates.cancelling = false;
  }
}

/** To call once, from the app root. Schedules the checks and follows the
 * download progress; returns the cleanup. */
export function startModUpdateChecks(): () => void {
  void loadPrefs();
  const first = setTimeout(() => void checkModUpdates(), FIRST_CHECK_MS);
  const daily = setInterval(() => void checkModUpdates(), CHECK_EVERY_MS);
  const unlisten = listen<UpdateProgress>("update:progress", (e) => {
    const busy = modUpdates.busy;
    if (!busy || busy.id !== e.payload.id) return;
    busy.received = e.payload.received;
    busy.total = e.payload.total;
  });
  return () => {
    clearTimeout(first);
    clearInterval(daily);
    unlisten.then((f) => f());
  };
}
