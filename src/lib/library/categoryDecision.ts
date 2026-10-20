// Forcing the category of a car or a track (REGLES§3 level 4): the typed
// bridge to the two commands, and what the pickers and the tag field do with a
// text. The decision itself - what it means, how it is laid on the computed
// category - is the backend's (`categories.rs`); the fields it leaves on a
// card (`category_forced`, `categories_added`, `category_auto`…) are what the
// screens read.
import { invoke } from "@tauri-apps/api/core";
import type { ModKind } from "./library";
import { bareCategory } from "./trackCategoryNames";

/** One gesture, as `categories::CategoryOp` reads it. */
export type CategoryOp =
  | { op: "force"; tag: string }
  | { op: "add"; tag: string }
  | { op: "remove"; tag: string }
  | { op: "reset" };

/** Lays a gesture on the category of every mod of `ids`, all or nothing. */
export function applyCategoryOp(ids: string[], op: CategoryOp): Promise<void> {
  return invoke<void>("apply_category_op", { ids, op });
}

/** What a picker offers for a kind: what the rules can produce, then what the
 * user decided on other mods. */
export function listCategoryChoices(kind: ModKind): Promise<string[]> {
  return invoke<string[]>("list_category_choices", { kind });
}

/** The categories a typed text points at, for the tag field's suggestions:
 * those whose name starts with it first, then those that contain it. A text
 * with nothing in it points at none. */
export function matchCategories(typed: string, choices: string[], max = 5): string[] {
  const bare = bareCategory(typed);
  if (!bare) return [];
  const starts: string[] = [];
  const contains: string[] = [];
  for (const c of choices) {
    const name = bareCategory(c);
    if (name.startsWith(bare)) starts.push(c);
    else if (name.includes(bare)) contains.push(c);
  }
  return [...starts, ...contains].slice(0, max);
}
