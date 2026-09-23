// Editing the category families (Categories tab, TAXO§6) - as DECISIONS on top
// of the catalogue, never as a rewritten table (REGLES§2).
//
// Every function takes the user's overlay and the shipped catalogue, and
// returns a new overlay. **The effective table is never computed here**: that
// is `taxonomy.rs`, the one place the engine reads, and the tab displays what
// it sends back after each save. Two implementations of the merge would end up
// disagreeing - the country aliases once had a TypeScript copy, and it is the
// copy nobody saw that won.
//
// The overlay is keyed by TAG, not by family: forking a whole family to move
// one tag would freeze it, and the next catalogue could no longer add to it.
import { familyLookup, familyTag, type CategoryFamily } from "$lib/library/families";
import type { FamilyOverlay } from "$lib/workshop/rules";

function copy(o: FamilyOverlay): Required<FamilyOverlay> {
  return {
    meta: { ...(o.meta ?? {}) },
    created: [...(o.created ?? [])],
    removed: [...(o.removed ?? [])],
    tags: { ...(o.tags ?? {}) },
  };
}

/**
 * Attaches a tag to a family - which detaches it from whichever family held it,
 * since the overlay says where each tag goes (TAXO§7.3: one tag, one family).
 * Sending a tag back where the catalogue puts it removes the decision rather
 * than restating it: the tag is back on the catalogue, and follows it.
 */
export function attachTag(o: FamilyOverlay, catalog: CategoryFamily[], tag: string, toId: string): FamilyOverlay {
  const key = familyTag(tag);
  if (!key) return o;
  const next = copy(o);
  if (familyLookup(catalog).get(key) === toId) delete next.tags[key];
  else next.tags[key] = toId;
  return next;
}

/** Detaches a tag from every family. `""` is a decision only when the
 * catalogue gives the tag a family; otherwise there is nothing to override. */
export function detachTag(o: FamilyOverlay, catalog: CategoryFamily[], tag: string): FamilyOverlay {
  const key = familyTag(tag);
  const next = copy(o);
  if (familyLookup(catalog).has(key)) next.tags[key] = "";
  else delete next.tags[key];
  return next;
}

/** Renames a family or changes its icon. On a shipped family only what
 * differs from the catalogue is kept; an emptied name falls back on the
 * translation. */
export function setFamilyLook(
  o: FamilyOverlay,
  catalog: CategoryFamily[],
  id: string,
  patch: { name?: string; icon?: string },
): FamilyOverlay {
  const next = copy(o);
  const mine = next.created.findIndex((c) => c.id === id);
  if (mine >= 0) {
    const c = { ...next.created[mine], ...patch };
    if (!c.icon) delete c.icon;
    next.created[mine] = c;
    return next;
  }
  const shipped = catalog.find((f) => f.id === id);
  const meta = { ...(next.meta[id] ?? {}), ...patch };
  if (!meta.name?.trim() || meta.name === shipped?.name) delete meta.name;
  if (!meta.icon || meta.icon === shipped?.icon) delete meta.icon;
  if (Object.keys(meta).length) next.meta[id] = meta;
  else delete next.meta[id];
  return next;
}

/**
 * A new family, named by the user. Its id is a slug of the name, made unique,
 * and **fixed at creation**: it is what a filter chip stores, so a chip posed on
 * "Endurance" must keep working once the family is renamed "Le Mans".
 */
export function createFamily(o: FamilyOverlay, taken: string[], name: string): { overlay: FamilyOverlay; id: string } {
  const base =
    name
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "family";
  const used = new Set([...taken, ...(o.created ?? []).map((c) => c.id)]);
  let id = base;
  for (let n = 2; used.has(id); n++) id = `${base}-${n}`;
  const next = copy(o);
  next.created.push({ id, name: name.trim(), tags: [] });
  return { overlay: next, id };
}

/** Deletes a family. A shipped one gets a tombstone - an absence would let the
 * next catalogue bring it back. Either way the tags sent to it go nowhere. */
export function deleteFamily(o: FamilyOverlay, catalog: CategoryFamily[], id: string): FamilyOverlay {
  const next = copy(o);
  for (const [k, v] of Object.entries(next.tags)) if (v === id) delete next.tags[k];
  delete next.meta[id];
  const mine = next.created.some((c) => c.id === id);
  next.created = next.created.filter((c) => c.id !== id);
  if (!mine && catalog.some((f) => f.id === id) && !next.removed.includes(id)) next.removed.push(id);
  return next;
}

/**
 * Puts a shipped family back as the catalogue has it: its look, its existence,
 * and every tag decision touching it - the tags it gained go back to the family
 * the catalogue gives them, the ones it lost come home. Nothing is computed:
 * removing the decisions IS restoring, which is why a gained tag can no longer
 * vanish from every family as it did when the tab rewrote whole tables.
 */
export function restoreFamily(o: FamilyOverlay, catalog: CategoryFamily[], id: string): FamilyOverlay {
  const next = copy(o);
  const shipped = familyLookup(catalog);
  delete next.meta[id];
  next.removed = next.removed.filter((r) => r !== id);
  for (const [k, v] of Object.entries(next.tags)) if (v === id || shipped.get(k) === id) delete next.tags[k];
  return next;
}

/** Whether the user decided anything about this family - the flag of the list
 * (TAXO§6.1). A family he made is his by definition. */
export function isCurated(o: FamilyOverlay, catalog: CategoryFamily[], id: string): boolean {
  if ((o.created ?? []).some((c) => c.id === id)) return true;
  if (o.meta?.[id] || (o.removed ?? []).includes(id)) return true;
  const shipped = familyLookup(catalog);
  return Object.entries(o.tags ?? {}).some(([k, v]) => v === id || shipped.get(k) === id);
}

/**
 * How many cars carry each tag, in its compared form. A car counts once per
 * tag, however many origins bring it (file, rule, manual) - which is what the
 * index counts too.
 */
export function tagCounts(cars: string[][]): Map<string, number> {
  const m = new Map<string, number>();
  for (const tags of cars) {
    for (const key of new Set(tags.map(familyTag))) if (key) m.set(key, (m.get(key) ?? 0) + 1);
  }
  return m;
}
