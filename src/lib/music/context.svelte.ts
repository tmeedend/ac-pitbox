// The music context of the app (§16.5): the anchor a tile left, the screen
// last reached, and the shared copy of `music.json` that the pickers edit.
//
// **Called on arrivals only.** `reachView` is what a screen calls when it is
// reached — a library index, its list, a sheet, another screen — and never
// from something a chip, the search, a density or a sort re-runs. That is
// the whole rule of §16.5: the filter state is shared by tiles and chips
// (INDEX§2), so music that followed it would change on every chip.
//
// What is resolved goes to the engine (`musicSetContext`), which waits its
// own delay before playing it and does nothing when it is what already plays.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { familiesOfTags, familyLookup, familyTagsOf, type CategoryFamily } from "$lib/library/families";
import { categoryFamilies, loadFamilies } from "$lib/library/familyTable.svelte";
import { listLibrary, type ModCard } from "$lib/library/library";
import { getMusicConfig, musicSetContext, musicSetMinimized, saveMusicConfig, type MusicConfig } from "./music";
import {
  anchorAfter,
  anchorOnTile,
  resolve,
  sameResolution,
  waitsBeforePlaying,
  withAssignment,
  withRecent,
  type Anchor,
  type CarFacts,
  type Library,
  type Resolved,
  type Taxonomy,
  type TrackFacts,
  type View,
} from "./resolve";

const shared = $state<{ config: MusicConfig | null }>({ config: null });

let anchor: Anchor | null = null;
let lastView: View = { kind: "screen", key: null };
let lastSent: Resolved | null = null;
let loading: Promise<void> | null = null;

/** The shared `music.json`, `null` until read. Reactive. */
export function musicConfig(): MusicConfig | null {
  return shared.config;
}

/** Whether the music fields may show (§16.4): only with the user's own music
 * on. Off, the embedded pack plays MENU and GRID, and nothing else is
 * offered anywhere — the assignments stay in `music.json`, unread. */
export function ownMusicOn(): boolean {
  return shared.config?.use_custom_folders ?? false;
}

/** Reads `music.json` once; later calls wait for the same read. */
export function loadMusicConfig(): Promise<void> {
  loading ??= getMusicConfig()
    .then((cfg) => {
      shared.config = cfg;
      send();
    })
    .catch((e) => {
      loading = null;
      console.warn("music: config not read", e);
    });
  return loading;
}

function send(): void {
  const cfg = shared.config;
  if (!cfg) return;
  const next = resolve(lastView, anchor, cfg.assignments);
  if (sameResolution(lastSent, next)) return;
  lastSent = next;
  musicSetContext({ ...next, delayed: waitsBeforePlaying(lastView) }).catch((e) => {
    // Sent again at the next arrival: forget it was sent.
    lastSent = null;
    console.warn("music: context not sent", e);
  });
}

/** Bumped at every arrival: an arrival that had to read the library first
 * (`reachSession`) gives up if another screen was reached meanwhile. */
let arrival = 0;

/** A screen was reached (§16.5). */
export function reachView(view: View): void {
  arrival++;
  anchor = anchorAfter(anchor, view);
  lastView = view;
  send();
}

/** A tile of the library index was clicked (§16.5): its term becomes the
 * anchor. The list it opens is reached right after, and plays it. */
export function tileClicked(library: Library, taxonomy: Taxonomy, term: string): void {
  anchor = anchorOnTile(library, taxonomy, term);
}

/** The anchor, for the list to check its chip on arrival. */
export function currentAnchor(): Anchor | null {
  return anchor;
}

// --- Writes ---------------------------------------------------------------
//
// One chain for every write of `music.json` outside the Music settings (the
// lines of Classement): two quick picks are written in the order they were
// made, and a failed write does not freeze the ones after it (CLAUDE.md, rule
// 6). A failure is returned to the caller, which shows it.

let chain: Promise<unknown> = Promise.resolve();

function write(edit: (cfg: MusicConfig) => MusicConfig): Promise<void> {
  const run = chain.then(async () => {
    await loadMusicConfig();
    const current = shared.config;
    // A key, shown translated by `errorText` under the line that failed.
    if (!current) throw new Error("errors.musicConfigUnreadable");
    const next = edit(current);
    await saveMusicConfig(next);
    shared.config = next;
    send();
  });
  chain = run.catch((e) => console.warn("music: write failed", e));
  return run;
}

/** Assigns a source to a term or a screen, or removes it (`null`). */
export function assignMusic(taxonomy: Taxonomy | "screen", id: string, source: string | null): Promise<void> {
  return write((cfg) => ({
    ...cfg,
    assignments: withAssignment(cfg.assignments, taxonomy, id, source),
    recent_sources: source ? withRecent(cfg.recent_sources, source) : cfg.recent_sources,
  }));
}

/** A term merged into another (a brand or a country, TAXO§7): its music
 * follows it when the term it joins has none — the merge loses nothing. When
 * both have one, the joined term keeps its own, and the other stays, shown as
 * an orphan in the Music settings until removed. */
export async function moveMusicTerm(taxonomy: Taxonomy, from: string, to: string): Promise<void> {
  await loadMusicConfig();
  const a = shared.config?.assignments[taxonomy];
  if (!a || !to || from === to || !a[from] || a[to]) return;
  const source = a[from];
  return write((cfg) => ({
    ...cfg,
    assignments: withAssignment(withAssignment(cfg.assignments, taxonomy, to, source), taxonomy, from, null),
  }));
}

/** The Music settings saved the whole file: take it as the shared copy. */
export function musicConfigSaved(cfg: MusicConfig): void {
  shared.config = $state.snapshot(cfg) as MusicConfig;
  send();
}

// --- Facts --------------------------------------------------------------

/** What the cascade reads of a car (§16.5). */
export function carFacts(card: ModCard, families: CategoryFamily[]): CarFacts {
  const lookup = familyLookup(families);
  return {
    brand: card.brand,
    families: familiesOfTags(familyTagsOf(card), lookup, families),
    country: card.country,
  };
}

/** What the cascade reads of a track: its categories, main one first. */
export function trackFacts(card: ModCard): TrackFacts {
  return { categories: card.categories, country: card.country };
}

/** The library as last listed, with the version it was listed at: the
 * session settings read the pair's facts from it, and the library screens may
 * never have been mounted (the app reopened on the session settings). */
let cards: { version: number; list: Promise<ModCard[]> } | null = null;

/** The library listing at `version`, read once per version — shared by the
 * session settings and the summary of the Music settings. */
export function cardsAt(version: number): Promise<ModCard[]> {
  if (cards?.version !== version) {
    const list = listLibrary();
    cards = { version, list };
    list.catch(() => (cards = null));
  }
  return cards.list;
}

/** The session settings were reached, or their pair changed (§16.5). */
export async function reachSession(carId: string | null, trackId: string | null, version: number): Promise<void> {
  const mine = ++arrival;
  let car: CarFacts | null = null;
  let track: TrackFacts | null = null;
  if (carId || trackId) {
    try {
      const [list] = await Promise.all([cardsAt(version), loadFamilies()]);
      const c = carId ? list.find((x) => x.id_interne === carId) : undefined;
      const t = trackId ? list.find((x) => x.id_interne === trackId) : undefined;
      car = c ? carFacts(c, categoryFamilies()) : null;
      track = t ? trackFacts(t) : null;
    } catch (e) {
      console.warn("music: library not read for the session settings", e);
    }
  }
  // Another screen reached while the library was read: it has the say.
  if (mine !== arrival) return;
  reachView({ kind: "session", car, track });
}

// --- The window ---------------------------------------------------------

/** Minimising fades the music out, restoring brings it back (§16.1). A mere
 * loss of focus sends nothing: music goes on behind another window. */
export function watchMinimized(): () => void {
  const win = getCurrentWindow();
  let last = false;
  const check = () =>
    win
      .isMinimized()
      .then((min) => {
        if (min === last) return;
        last = min;
        return musicSetMinimized(min);
      })
      .catch((e) => console.warn("music: window state not read", e));
  const off = win.onResized(() => void check());
  return () => void off.then((f) => f());
}
