// The category family table, as the car library reads it (INDEX§6.1).
//
// It lives in the tag rules (`rules.rs`, `car.category_families`) - the table
// the user can already edit, and the one the future Categories tab of the
// Workshop will edit. There is deliberately no second copy in TypeScript: the
// country aliases had one for a while, and it is the copy the user could not
// see that won in silence (see `flags.ts`).
import { invoke } from "@tauri-apps/api/core";
import { readOnce } from "$lib/readOnce";
import type { Rules } from "$lib/workshop/rules";
import type { CategoryFamily } from "./families";

const table = $state<{ list: CategoryFamily[] }>({ list: [] });

/** Set once the Categories tab has handed its own table: a read still in
 * flight from before the save must not put the old one back. */
let saved = false;

/**
 * Loads the table once per session. Nothing waits on it for more than a few
 * seconds - a table that does not come costs the category section of the
 * index (INDEX§8), never the library screen - but a late one still lands, and
 * a failed one is asked again (`readOnce`).
 */
export const loadFamilies = readOnce(
  () => invoke<Rules | null>("get_rules"),
  (rules) => {
    if (!saved) table.list = rules?.car?.category_families ?? [];
  },
  { label: "get_rules", usable: (rules) => rules != null },
);

/** Reactive read: an empty list until `loadFamilies` has answered. */
export function categoryFamilies(): CategoryFamily[] {
  return table.list;
}

/** Replaces the cached table after the Categories tab saved it, so the index
 * and the Family filter follow without a restart. */
export function setFamilies(list: CategoryFamily[]): void {
  table.list = list;
  saved = true;
}
