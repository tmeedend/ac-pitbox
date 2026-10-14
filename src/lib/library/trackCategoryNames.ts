// What a track category is called and how it is drawn (Families › Tracks).
//
// A category IS its `#tag`: that is what the rules produce, what a track
// stores, what a filter chip keeps. Its name and its icon are display only,
// and come in this order: what the user gave it (`track_category_looks` of
// the taxonomy overlay), then what the app ships - a translation and the
// pictogram of its own name for the categories Pit Box knows - then the tag
// itself and the circuit. Pure, for Vitest: the reactive table is
// `trackCategoryTable.svelte.ts`.
import type { CategoryLook } from "$lib/workshop/rules";
import { DEFAULT_TRACK_ICON, TRACK_CATEGORY_ICONS } from "./trackCategoryIcons";

/** A category without its `#`, lowercased: `#Drift` → `drift`. */
export function bareCategory(tag: string): string {
  return tag.trim().toLowerCase().replace(/^#+/, "").trim();
}

/** A category as it is stored: `drift` → `#drift`. */
export function categoryTag(raw: string): string {
  const bare = bareCategory(raw);
  return bare ? `#${bare}` : "";
}

/** Whether Pit Box ships a name and a pictogram for this category. */
export function isKnownCategory(tag: string): boolean {
  return bareCategory(tag) in TRACK_CATEGORY_ICONS;
}

/** The icon id of a category: the user's choice, else its own pictogram,
 * else the circuit. */
export function categoryIconId(tag: string, looks: Record<string, CategoryLook>): string {
  const bare = bareCategory(tag);
  return looks[categoryTag(tag)]?.icon ?? (bare in TRACK_CATEGORY_ICONS ? bare : DEFAULT_TRACK_ICON);
}

/** The name of a category: the user's, else the app's translation, else the
 * tag without its `#` - a category the catalogue added after this build. */
export function categoryName(
  tag: string,
  looks: Record<string, CategoryLook>,
  translate: (key: string) => string,
): string {
  const own = looks[categoryTag(tag)]?.name;
  if (own) return own;
  const bare = bareCategory(tag);
  return bare in TRACK_CATEGORY_ICONS ? translate(`trackCategories.${bare}`) : bare;
}

/**
 * The tag of a category the user creates from a name. **Fixed at creation**,
 * like a family's id: it is what the tracks store and what a chip keeps, so
 * renaming the category later must not move it.
 */
export function tagForName(name: string, taken: string[]): string {
  const base =
    name
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_|_$/g, "") || "category";
  const used = new Set(taken.map(categoryTag));
  let tag = `#${base}`;
  for (let n = 2; used.has(tag); n++) tag = `#${base}_${n}`;
  return tag;
}
