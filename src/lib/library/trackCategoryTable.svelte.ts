// The names and icons of the track categories, as every screen reads them:
// the track index, the Category filter, the Online page, Families › Tracks.
//
// Read from the taxonomy overlay once per session (`readOnce`, same reasons as
// `familyTable.svelte.ts`), and replaced by the Families tab after each save
// so that every screen follows without a restart. Until it answers, a
// category is named and drawn as shipped - the user's renames come late,
// never wrong.
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/index.svelte";
import { readOnce } from "$lib/readOnce";
import type { CategoryLook, TaxonomyView } from "$lib/workshop/rules";
import { categoryIconId, categoryName } from "./trackCategoryNames";
import { trackCategoryIcon } from "./trackCategoryIcons";

const table = $state<{ looks: Record<string, CategoryLook> }>({ looks: {} });

/** Set once the Families tab has handed its own looks: a read still in
 * flight from before the save must not put the old ones back. */
let saved = false;

export const loadTrackCategoryLooks = readOnce(
  () => invoke<TaxonomyView | null>("get_taxonomy"),
  (view) => {
    if (!saved) table.looks = view?.overlay.track_category_looks ?? {};
  },
  { label: "get_taxonomy", usable: (view) => view != null },
);

/** Replaces the looks after the Families tab saved them. */
export function setTrackCategoryLooks(looks: Record<string, CategoryLook>): void {
  table.looks = looks;
  saved = true;
}

/** Reactive: the name a category is shown under. */
export function trackCategoryName(tag: string): string {
  return categoryName(tag, table.looks, t);
}

/** Reactive: the icon id of a category (`trackCategoryIcons.ts`). */
export function trackCategoryIconId(tag: string): string {
  return categoryIconId(tag, table.looks);
}

/** Reactive: the inner SVG markup of a category's pictogram. */
export function trackCategoryGlyph(tag: string): string {
  return trackCategoryIcon(trackCategoryIconId(tag));
}

/** Names the values of the filters that carry track categories (the
 * library's Category, the Online page's Track category), like
 * `withCountryLabels` names countries. The stored value stays the tag. */
export function withTrackCategoryLabels<D extends { categoryIcons?: boolean; labelOf?: (v: string) => string }>(
  defs: D[],
): D[] {
  return defs.map((d) => (d.categoryIcons ? { ...d, labelOf: trackCategoryName } : d));
}
