// The index of the library (INDEX§1): what the Cars and Tracks screens show
// when nothing is filtered, instead of a wall of three hundred thumbnails.
//
// **The index is a way of entering a filter, not a second mechanism** (INDEX§2,
// R2). A tile poses a chip through `poseValue`, and from there on the
// ordinary filter state does everything: crossing with a year, excluding a tag,
// "Clear all", the result count, pinning. Nothing in this module selects or
// filters cards - it only decides WHICH tiles to show, in what order.
//
// Pure, no i18n: the component translates, this orders.
import { UNSET_VALUE, type FilterOption } from "./filters";

export interface IndexTile {
  value: string;
  label: string;
  count: number;
  /** The "Not set" / "Unclassified" tile: last, dashed, neutral emblem
   * (INDEX§4.3). */
  unset: boolean;
}

/**
 * Tiles of one index section, from the options the filter editor already
 * offers - the same values and the same counts, so a tile and the suggestion
 * list of its chip can never disagree.
 *
 * Largest first: the terms that matter are the ones seen most. Ties by label,
 * so two launches give the same grid. A value no mod carries gets no tile (a
 * shipped family with nothing in it leads nowhere), and the absent-value tile
 * closes the grid, whatever its count - it is the gap in the data, not a value
 * among the others.
 */
export function indexTiles(options: FilterOption[]): IndexTile[] {
  const tiles = options
    .filter((o) => o.count > 0 && o.value !== UNSET_VALUE)
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))
    .map((o) => ({ value: o.value, label: o.label, count: o.count, unset: false }));
  const unset = options.find((o) => o.value === UNSET_VALUE && o.count > 0);
  if (unset) tiles.push({ value: unset.value, label: unset.label, count: unset.count, unset: true });
  return tiles;
}

/** Bounds of the brand grid (INDEX§6.2): under eight the grid says
 * nothing a list would not, over twenty-four it is the wall again. */
export const BRAND_TILES_MIN = 8;
export const BRAND_TILES_MAX = 24;
/** Share of the library the shown brands must cover together. */
export const BRAND_COVERAGE = 0.8;

/**
 * How many brand tiles to show before "All brands" (INDEX§6.2).
 *
 * **Dynamic, never a fixed top N**: the brands that, taken by decreasing count,
 * cover 80 % of the library - bounded to [8, 24]. A fixed "top 18" breaks both
 * ways: on a library of forty cars it lists brands of one car each, on one of
 * two thousand it hides brands of thirty.
 *
 * `counts` sorted by decreasing count; `total` is the size of the library,
 * brandless cars included - they are part of what has to be covered.
 */
export function significantBrandCount(counts: number[], total: number): number {
  let covered = 0;
  let n = 0;
  while (n < counts.length && covered < BRAND_COVERAGE * total) covered += counts[n++];
  return Math.min(counts.length, Math.max(BRAND_TILES_MIN, Math.min(BRAND_TILES_MAX, n)));
}

/**
 * The brand tiles to draw: the significant ones by count, then - once unfolded
 * - the rest in alphabetical order (INDEX§6.2), which is how one looks a
 * brand up in a long list.
 */
export function brandTiles(tiles: IndexTile[], total: number, unfolded: boolean): { shown: IndexTile[]; hidden: number } {
  const n = significantBrandCount(
    tiles.map((t) => t.count),
    total,
  );
  const head = tiles.slice(0, n);
  const rest = tiles.slice(n);
  if (!unfolded) return { shown: head, hidden: rest.length };
  return { shown: [...head, ...[...rest].sort((a, b) => a.label.localeCompare(b.label))], hidden: 0 };
}

/**
 * The emblem of a brand tile, until the canonical logo exists (TAXO§4).
 *
 * The badge of the car with the alphabetically first id among those that have
 * one: arbitrary, but deterministic - two launches show the same logo, which is
 * criterion 4 of the canonical election and the only one that needs no image
 * analysis. The election proper (transparent background, resolution, majority
 * vote) replaces this, it does not add to it.
 */
export function brandBadges(cards: { id_interne: string; brand: string | null; badge: string | null }[]): Map<string, string> {
  const best = new Map<string, { id: string; badge: string }>();
  for (const c of cards) {
    if (!c.brand || !c.badge) continue;
    const cur = best.get(c.brand);
    if (!cur || c.id_interne < cur.id) best.set(c.brand, { id: c.id_interne, badge: c.badge });
  }
  return new Map([...best].map(([brand, v]) => [brand, v.badge]));
}

/** Initials of a brand without a logo - never an empty disc (INDEX§4.1). */
export function initials(name: string): string {
  const words = name.split(/[\s\-_.]+/).filter(Boolean);
  const letters = words.length > 1 ? words.map((w) => w[0]).join("") : name;
  return letters.slice(0, 2).toUpperCase();
}
