// The track category of a server (SPEC-play-online.md, "Catégorie de
// circuit"). Pure, for Vitest: the page's filter and its panel both read it.
//
// In this order, the first that answers gives the track's categories:
//   1. the user's choice, made on the panel (`overlay.sqlite`);
//   2. the library, when the track is there with categories;
//   3. the catalogue (`taxonomy-catalog.json` and its overlay).
// Then the server's own words, marked "≈": a keyword of its name, of its
// description when its details were read, or of the track's id. They ADD to
// the track's categories, never replace them — "Drift" keeps a server whose
// track is drift OR which announces drift.
//
// A server none of them answers for has an UNKNOWN category. The filter then
// leaves it out of an included category and keeps it under an excluded one
// (`tokens.ts`): most of the lobby's tracks are not in the library, and
// letting them all through made "Drift" list whatever ran anywhere.
import type { Looks } from "./looks";
import type { ServerSummary, TrackRef } from "./online";

/** What the backend knows beyond the library (`online/categories.rs`). */
export interface CategoryKnowledge {
  /** The rules' allowlist of track categories, `#`-prefixed: what a choice
   * is made from. */
  vocabulary: string[];
  /** Lowercase track id → the categories the user chose. */
  chosen: Record<string, string[]>;
  /** Lowercase track id → the catalogue's categories. */
  catalog: Record<string, string[]>;
  /** Lowercase keyword → the category it announces. */
  keywords: Record<string, string>;
}

export const NO_KNOWLEDGE: CategoryKnowledge = { vocabulary: [], chosen: {}, catalog: {}, keywords: {} };

export type CategorySource = "chosen" | "library" | "catalog";
export type HintSource = "name" | "description" | "track";

/** A category a server's own words announce, and which word, where. */
export interface Hint {
  category: string;
  word: string;
  source: HintSource;
}

export interface ServerCategories {
  /** The track's categories, and who said them; `null` when nobody did. */
  base: string[];
  source: CategorySource | null;
  /** The categories the server's words add, one hint per category. */
  hints: Hint[];
  /** Base and hints together: what the filter compares. */
  all: string[];
}

/** What the category token compares: every category, `null` for an
 * unknown one (`tokens.ts`). */
export function filterCategories(c: ServerCategories): string[] | null {
  return c.all.length ? c.all : null;
}

/** Lowercase, accents gone: the comparison the search makes everywhere. */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/** The words of a text, as a keyword is matched: whole words, any
 * punctuation or `_` between them — a track id `ek_akina_drift` is three. */
export function words(text: string): string[] {
  return fold(text)
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean);
}

/** The keywords `text` holds, as hints from `source`. A keyword of several
 * words is matched as a phrase. One preceded by "no" is not an announcement
 * but its contrary ("No Drift", "No traffic"): ignored. */
export function keywordHints(text: string, source: HintSource, keywords: Record<string, string>): Hint[] {
  const ws = words(text);
  const out: Hint[] = [];
  for (const [keyword, category] of Object.entries(keywords)) {
    const kw = words(keyword);
    if (!kw.length) continue;
    for (let i = 0; i + kw.length <= ws.length; i++) {
      if (kw.every((w, j) => ws[i + j] === w) && ws[i - 1] !== "no") {
        out.push({ category, word: keyword, source });
        break;
      }
    }
  }
  return out;
}

/** The track's own categories and who gives them, in the order of the spec. */
function trackBase(
  trackId: string,
  looks: Looks,
  k: CategoryKnowledge,
): { base: string[]; source: CategorySource | null } {
  const id = trackId.toLowerCase();
  const chosen = k.chosen[id];
  if (chosen?.length) return { base: chosen, source: "chosen" };
  // A track the library holds without any category says nothing about its
  // category: the catalogue may still know it.
  const library = looks.tracks[id]?.categories;
  if (library?.length) return { base: library.map((c) => c.toLowerCase()), source: "library" };
  const catalog = k.catalog[id];
  if (catalog?.length) return { base: catalog, source: "catalog" };
  return { base: [], source: null };
}

/** The categories a track is drawn with, where no picture of it exists
 * (`TrackThumb`): its own, else what its id announces (`ek_myogi-freeroam`) -
 * never a server's name or description, which speak of the server, and a
 * group of servers has as many as it has servers. */
export function trackGlyphCategories(track: TrackRef, looks: Looks, k: CategoryKnowledge): string[] {
  const { base } = trackBase(track.id, looks, k);
  return base.length ? base : keywordHints(track.kunos_id, "track", k.keywords).map((h) => h.category);
}

/** Everything a server says of its category. `description` is its details'
 * text, when they were read — the list does not have it, the panel does. */
export function serverCategories(
  server: ServerSummary,
  looks: Looks,
  k: CategoryKnowledge,
  description: string | null = null,
): ServerCategories {
  const { base, source } = trackBase(server.track.id, looks, k);
  const found = [
    ...keywordHints(server.name, "name", k.keywords),
    ...(description ? keywordHints(description, "description", k.keywords) : []),
    ...keywordHints(server.track.kunos_id, "track", k.keywords),
  ];
  // One hint per category, the first source winning: the name before the
  // description before the track's id. A category the track already has
  // needs no hint.
  const hints: Hint[] = [];
  for (const h of found) {
    if (!base.includes(h.category) && !hints.some((x) => x.category === h.category)) hints.push(h);
  }
  return { base, source, hints, all: [...base, ...hints.map((h) => h.category)] };
}
