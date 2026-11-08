// What plays on the screen on display (§16.5) — pure, no Svelte, no i18n.
//
// **Music changes on a gesture of navigation, never on a change of filter.**
// A tile of the library index writes into the very filter state the chips
// write into (INDEX§2, R2), so the filter cannot tell "the user clicked
// Porsche" from "the user typed Porsche in the chip editor". The music
// therefore follows explicit events — a tile clicked, a screen reached — and
// this module turns the last one into a source. Who calls `resolve` decides
// WHEN (an arrival); this only decides WHAT.
//
// The anchor is what a tile click leaves behind: the term it posed, in its
// library. It holds while one browses that term (the list, its sheets), and
// goes when the index of its library comes back. One anchor at a time.
import type { FilterMap } from "$lib/library/filters";

/** The four taxonomies a music can be attached to (§16.4). */
export type Taxonomy = "brand" | "family" | "track_category" | "country";

/** Screens with a music of their own, keyed as `music.json` stores them. The
 * rail's entries, Story aside; `race` has none — GRID is its default. */
export const SCREEN_KEYS = [
  "cars",
  "tracks",
  "driver",
  "online",
  "apps",
  "others",
  "sorting",
  "files",
  "settings",
  "about",
] as const;
export type ScreenKey = (typeof SCREEN_KEYS)[number];

export type Library = "cars" | "tracks";

/** Source path by canonical id, per taxonomy, and by screen. */
export interface Assignments {
  brand: Record<string, string>;
  family: Record<string, string>;
  track_category: Record<string, string>;
  country: Record<string, string>;
  screen: Record<string, string>;
}

export function emptyAssignments(): Assignments {
  return { brand: {}, family: {}, track_category: {}, country: {}, screen: {} };
}

export interface Anchor {
  library: Library;
  taxonomy: Taxonomy;
  term: string;
}

/** What a car is filed under, as the cascade reads it. Families in the order
 * of the family table. */
export interface CarFacts {
  brand: string | null;
  families: string[];
  country: string | null;
}

/** Categories in rank order: the first one is the track's main category. */
export interface TrackFacts {
  categories: string[];
  country: string | null;
}

/** The screen reached. */
export type View =
  | { kind: "index"; library: Library }
  /** `anchorPosed`: the anchor's chip is still in the filter, read at the
   * moment the list is reached — not watched after. */
  | { kind: "list"; library: Library; anchorPosed: boolean }
  | { kind: "car"; car: CarFacts }
  | { kind: "track"; track: TrackFacts }
  /** The session settings: the pair about to be launched. */
  | { kind: "session"; car: CarFacts | null; track: TrackFacts | null }
  /** Any other screen; `null` for one with no music of its own (Story). */
  | { kind: "screen"; key: ScreenKey | null };

/** What the engine plays: the assigned source, or the base ambience. */
export interface Resolved {
  assigned: string | null;
  base: "menu" | "grid";
}

/** The filter key a tile of each taxonomy poses (INDEX§5, §6). */
export const FILTER_KEY: Record<Taxonomy, string> = {
  brand: "brand",
  family: "family",
  track_category: "category",
  country: "country",
};

const lc = (s: string) => s.toLowerCase();
const same = (a: string | null, b: string) => a != null && lc(a) === lc(b);

/** Whether the anchor's term is still posed, positively, in a filter map —
 * excluded is not "still browsing it". */
export function anchorPosedIn(filters: FilterMap, anchor: Anchor | null): boolean {
  if (!anchor) return false;
  const st = filters[FILTER_KEY[anchor.taxonomy]];
  return st?.type === "val" && st.values.some((v) => v.sign > 0 && lc(v.value) === lc(anchor.term));
}

/** The anchor after a tile click: the term clicked, whatever was there. */
export function anchorOnTile(library: Library, taxonomy: Taxonomy, term: string): Anchor {
  return { library, taxonomy, term };
}

/** The anchor after reaching a view: the index of its library clears it
 * (last chip removed, "Clear all"); every other screen keeps it in memory. */
export function anchorAfter(anchor: Anchor | null, view: View): Anchor | null {
  if (anchor && view.kind === "index" && view.library === anchor.library) return null;
  return anchor;
}

function carMatches(anchor: Anchor, car: CarFacts): boolean {
  if (anchor.library !== "cars") return false;
  switch (anchor.taxonomy) {
    case "brand":
      return same(car.brand, anchor.term);
    case "family":
      return car.families.some((f) => same(f, anchor.term));
    case "country":
      return same(car.country, anchor.term);
    default:
      return false;
  }
}

function trackMatches(anchor: Anchor, track: TrackFacts): boolean {
  if (anchor.library !== "tracks") return false;
  switch (anchor.taxonomy) {
    // Any of its categories, not only the main one: the user browsed "drift"
    // and opened a track that is a circuit AND a drift track.
    case "track_category":
      return track.categories.some((c) => same(c, anchor.term));
    case "country":
      return same(track.country, anchor.term);
    default:
      return false;
  }
}

/** The assignment of one term, matched on its canonical id, case aside. */
function lookup(map: Record<string, string>, id: string | null | undefined): string | null {
  if (!id) return null;
  if (map[id]) return map[id];
  const key = Object.keys(map).find((k) => lc(k) === lc(id));
  return key ? map[key] : null;
}

function first(...levels: (string | null)[]): string | null {
  return levels.find((l) => l != null) ?? null;
}

function carCascade(car: CarFacts, a: Assignments): string | null {
  return first(
    lookup(a.brand, car.brand),
    first(...car.families.map((f) => lookup(a.family, f))),
    lookup(a.country, car.country),
  );
}

function trackCascade(track: TrackFacts, a: Assignments): string | null {
  // Rank 1 only: the main category says what the track is.
  return first(lookup(a.track_category, track.categories[0]), lookup(a.country, track.country));
}

const menu = (assigned: string | null): Resolved => ({ assigned, base: "menu" });

/**
 * The source for a view (§16.5 table), the first level that has an
 * assignment winning:
 *
 * - library index → its screen → MENU;
 * - list → the anchor if its chip is still posed, else the screen;
 * - car sheet → the anchor if the car is filed under it, else brand → family
 *   → country → screen → MENU; track sheet → the anchor, else category (rank
 *   1) → country → screen → MENU;
 * - session settings → track category (rank 1) → track country → car brand →
 *   car family → GRID;
 * - any other screen → its screen → MENU.
 *
 * An anchor whose term has no music adds nothing: the cascade goes on.
 */
export function resolve(view: View, anchor: Anchor | null, a: Assignments): Resolved {
  const anchored = anchor ? lookup(a[anchor.taxonomy], anchor.term) : null;
  switch (view.kind) {
    case "index":
      return menu(lookup(a.screen, view.library));
    case "list": {
      const ours = anchor?.library === view.library && view.anchorPosed;
      return menu(first(ours ? anchored : null, lookup(a.screen, view.library)));
    }
    case "car": {
      const ours = anchor != null && carMatches(anchor, view.car);
      return menu(first(ours ? anchored : null, carCascade(view.car, a), lookup(a.screen, "cars")));
    }
    case "track": {
      const ours = anchor != null && trackMatches(anchor, view.track);
      return menu(first(ours ? anchored : null, trackCascade(view.track, a), lookup(a.screen, "tracks")));
    }
    case "session": {
      const t = view.track;
      const c = view.car;
      const assigned = first(
        t ? lookup(a.track_category, t.categories[0]) : null,
        t ? lookup(a.country, t.country) : null,
        c ? lookup(a.brand, c.brand) : null,
        c ? first(...c.families.map((f) => lookup(a.family, f))) : null,
      );
      return { assigned, base: "grid" };
    }
    case "screen":
      return menu(view.key ? lookup(a.screen, view.key) : null);
  }
}

/** Two resolutions that play the same thing — the second need not be sent. */
export function sameResolution(a: Resolved | null, b: Resolved): boolean {
  return a != null && a.base === b.base && (a.assigned ?? "").toLowerCase() === (b.assigned ?? "").toLowerCase();
}

/** The screen key of a section of the app, or `null` when it has no music of
 * its own (the session settings, Story). Classement and Fichiers are one
 * screen each: their tabs share its music. */
export function screenKeyOf(section: string, sortingTabs: readonly string[], filesTabs: readonly string[]): ScreenKey | null {
  if (sortingTabs.includes(section)) return "sorting";
  if (filesTabs.includes(section)) return "files";
  return (SCREEN_KEYS as readonly string[]).includes(section) ? (section as ScreenKey) : null;
}

/** Every assignment, flattened, for the summary of the Music settings. */
export interface AssignmentRow {
  taxonomy: Taxonomy | "screen";
  id: string;
  source: string;
}

export function assignmentRows(a: Assignments): AssignmentRow[] {
  const order: (Taxonomy | "screen")[] = ["screen", "brand", "family", "track_category", "country"];
  return order.flatMap((taxonomy) =>
    Object.entries(a[taxonomy])
      .sort(([x], [y]) => x.localeCompare(y))
      .map(([id, source]) => ({ taxonomy, id, source })),
  );
}

/** `next` with `id` of `taxonomy` assigned `source`, or unassigned (`null`).
 * A new object: the caller saves it whole. */
export function withAssignment(
  a: Assignments,
  taxonomy: Taxonomy | "screen",
  id: string,
  source: string | null,
): Assignments {
  const map = { ...a[taxonomy] };
  if (source) map[id] = source;
  else delete map[id];
  return { ...a, [taxonomy]: map };
}

/** The "Recent" list after `source` was picked: first, once, bounded. */
export function withRecent(recent: string[], source: string, max = 8): string[] {
  return [source, ...recent.filter((r) => lc(r) !== lc(source))].slice(0, max);
}
