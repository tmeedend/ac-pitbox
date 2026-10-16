// Families › Tracks (TAXO§6.4): what the tab computes and what its gestures
// decide, as pure functions - the component only lays them out.
//
// A track category lives in three places, and each gesture writes to the one
// that owns it:
//   - its place in the list and whether it is on: the rules overlay
//     (`track_categories`, an ordered allowlist), re-applied to the library
//     since it changes the category a track stores - `rulesEdit.ts` has the
//     verbs, shared with nothing else now that the list left the Rules tab;
//   - its name and icon: `track_category_looks` of the taxonomy overlay,
//     display only, nothing re-applied;
//   - the server words that announce it: `server_keywords` of the same
//     overlay, read by the Online page.
import { categoryTag } from "$lib/library/trackCategoryNames";
import type { CategoryLook, CategoryRow, MapOverlay, RuleRow, TagMerge } from "./rules";

/**
 * Whether the user decided anything about this category - the ⚑ of the list:
 * a name or an icon, switched off, one of his own, or moved. Moved is read off
 * the shipped categories that are on, in the order they now have against the
 * one the catalogue gave them: moving one up moves its neighbour down, and
 * both are flagged, which is what happened.
 */
export function isCurated(row: CategoryRow, rows: CategoryRow[], looks: Record<string, CategoryLook>): boolean {
  if (looks[row.name] || !row.on || !row.shipped) return true;
  const placed = rows.filter((r) => r.on && r.shipped_rank !== null);
  const now = placed.findIndex((r) => r.name === row.name);
  const shipped = [...placed].sort((a, b) => a.shipped_rank! - b.shipped_rank!).findIndex((r) => r.name === row.name);
  return now !== shipped;
}

/** The merge rules whose output is this category: what produces it. */
export function producers(rules: RuleRow<TagMerge>[], tag: string): RuleRow<TagMerge>[] {
  return rules.filter((r) => r.rule.to.some((to) => categoryTag(to) === tag));
}

/** A name or an icon given to a category; an emptied one goes back to the
 * app's. */
export function setLook(
  looks: Record<string, CategoryLook>,
  tag: string,
  patch: CategoryLook,
): Record<string, CategoryLook> {
  const next = { ...looks };
  const look = { ...(next[tag] ?? {}), ...patch };
  if (!look.name?.trim()) delete look.name;
  if (!look.icon) delete look.icon;
  if (Object.keys(look).length) next[tag] = look;
  else delete next[tag];
  return next;
}

/** Without the name and icon of one category. */
export function withoutLook(looks: Record<string, CategoryLook>, tag: string): Record<string, CategoryLook> {
  const next = { ...looks };
  delete next[tag];
  return next;
}

// --- Server keywords (Online, "Catégorie de circuit") -------------------------

const keyOf = (word: string) => word.trim().toLowerCase();

/** The words that announce this category, as they apply. */
export function keywordsOf(effective: Record<string, string>, tag: string): string[] {
  return Object.keys(effective)
    .filter((w) => categoryTag(effective[w]) === tag)
    .sort();
}

/** Shipped words that announced this category and no longer do - removed, or
 * sent to another: shown struck through, one click from coming back. */
export function goneKeywords(catalog: Record<string, string>, effective: Record<string, string>, tag: string): string[] {
  return Object.keys(catalog)
    .filter((w) => categoryTag(catalog[w]) === tag && categoryTag(effective[w] ?? "") !== tag)
    .sort();
}

function copy(o: MapOverlay): Required<MapOverlay> {
  return { set: { ...(o.set ?? {}) }, removed: [...(o.removed ?? [])] };
}

/** A word now announces this category. Sending a shipped word back where the
 * catalogue puts it removes the decision rather than restating it. */
export function addKeyword(o: MapOverlay, catalog: Record<string, string>, word: string, tag: string): MapOverlay {
  const key = keyOf(word);
  if (!key) return o;
  const next = copy(o);
  next.removed = next.removed.filter((r) => r !== key);
  if (categoryTag(catalog[key] ?? "") === tag) delete next.set[key];
  else next.set[key] = tag;
  return next;
}

/** A word no longer announces anything. A shipped one gets a tombstone, or
 * the catalogue would bring it back. */
export function removeKeyword(o: MapOverlay, catalog: Record<string, string>, word: string): MapOverlay {
  const key = keyOf(word);
  const next = copy(o);
  delete next.set[key];
  if (key in catalog && !next.removed.includes(key)) next.removed.push(key);
  return next;
}

/** A shipped word back as the catalogue has it. */
export function restoreKeyword(o: MapOverlay, word: string): MapOverlay {
  const key = keyOf(word);
  const next = copy(o);
  delete next.set[key];
  next.removed = next.removed.filter((r) => r !== key);
  return next;
}
