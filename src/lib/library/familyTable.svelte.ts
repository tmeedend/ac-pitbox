// The category family table, as the car library reads it (INDEX§6.1).
//
// It lives in the tag rules (`rules.rs`, `car.category_families`) - the table
// the user can already edit, and the one the future Categories tab of the
// Workshop will edit. There is deliberately no second copy in TypeScript: the
// country aliases had one for a while, and it is the copy the user could not
// see that won in silence (see `flags.ts`).
import { invokeSafe } from "$lib/invokeSafe";
import type { Rules } from "$lib/workshop/rules";
import type { CategoryFamily } from "./families";

const table = $state<{ list: CategoryFamily[] }>({ list: [] });

let loading: Promise<void> | null = null;

/**
 * Loads the table once per session. A READ, hence `invokeSafe`: a table that
 * does not come costs the category section of the index (INDEX§8), never
 * the library screen.
 */
export function loadFamilies(): Promise<void> {
  loading ??= invokeSafe<Rules | null>("get_rules", undefined, null).then((rules) => {
    table.list = rules?.car?.category_families ?? [];
  });
  return loading;
}

/** Reactive read: an empty list until `loadFamilies` has answered. */
export function categoryFamilies(): CategoryFamily[] {
  return table.list;
}
