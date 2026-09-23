// Editing the category family table (Categories tab, TAXO§6).
//
// Every function returns a NEW table: the screen assigns it, saves it whole
// (`saveCategoryFamilies`), and a `$state` proxy sees one write. Nothing here
// touches the disk or the screen - it is the part of the tab whose edge cases
// only show when run, hence its own tests.
import { familyLookup, familyTag, type CategoryFamily } from "$lib/library/families";

/**
 * Attaches a tag to a family - and detaches it from whichever family held it.
 *
 * **A tag belongs to one family** (TAXO§7.3): attached elsewhere, it MOVES.
 * Copied, it would count a car twice in the index and the counters would stop
 * meaning anything.
 */
export function moveTag(families: CategoryFamily[], tag: string, toId: string): CategoryFamily[] {
  const key = familyTag(tag);
  if (!key) return families;
  return families.map((f) => {
    const kept = f.tags.filter((t) => familyTag(t) !== key);
    return f.id === toId ? { ...f, tags: [...kept, key] } : kept.length === f.tags.length ? f : { ...f, tags: kept };
  });
}

export function removeTag(families: CategoryFamily[], id: string, tag: string): CategoryFamily[] {
  const key = familyTag(tag);
  return families.map((f) => (f.id === id ? { ...f, tags: f.tags.filter((t) => familyTag(t) !== key) } : f));
}

export function patchFamily(
  families: CategoryFamily[],
  id: string,
  patch: Partial<Pick<CategoryFamily, "name" | "icon">>,
): CategoryFamily[] {
  return families.map((f) => {
    if (f.id !== id) return f;
    const next = { ...f, ...patch };
    // An empty name is no name: a shipped family falls back on its
    // translation instead of showing a blank line.
    if (!next.name?.trim()) delete next.name;
    if (!next.icon) delete next.icon;
    return next;
  });
}

/**
 * A new family, named by the user.
 *
 * Its id is a slug of the name, made unique. **The id is what a filter chip
 * stores**, so it is fixed at creation and never follows a later rename: a
 * chip posed on "Endurance" must keep working once it is renamed "Le Mans".
 */
export function addFamily(families: CategoryFamily[], name: string): { families: CategoryFamily[]; id: string } {
  const base =
    name
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "family";
  const taken = new Set(families.map((f) => f.id));
  let id = base;
  for (let n = 2; taken.has(id); n++) id = `${base}-${n}`;
  return { families: [...families, { id, name: name.trim(), tags: [] }], id };
}

export function deleteFamily(families: CategoryFamily[], id: string): CategoryFamily[] {
  return families.filter((f) => f.id !== id);
}

/**
 * Whether a family differs from what the app shipped - the flag of the list
 * (TAXO§6.1), and what "Restore" undoes. A family the user created has no
 * shipped version: it is curated by definition.
 *
 * Tags compare as a SET: order carries nothing, and a family whose tags were
 * removed then put back is not "modified".
 */
export function isCurated(f: CategoryFamily, shipped: CategoryFamily | undefined): boolean {
  if (!shipped) return true;
  if ((f.name ?? "") !== (shipped.name ?? "") || (f.icon ?? "") !== (shipped.icon ?? "")) return true;
  const a = new Set(f.tags.map(familyTag));
  const b = new Set(shipped.tags.map(familyTag));
  return a.size !== b.size || [...a].some((t) => !b.has(t));
}

/**
 * Puts one family back as shipped: its tags are taken back from whichever
 * family they had been moved to, and the tags it had gained go home.
 *
 * **Home, not nowhere.** A tag moved in from another family returns to the
 * family that ships it, when that family still exists; only a tag no shipped
 * family lists is dropped. Restoring Classic after moving `gt3` into it must
 * not leave `gt3` in no family at all - Race would lose its GT3 cars without
 * anyone having touched Race.
 */
export function restoreFamily(families: CategoryFamily[], shippedTable: CategoryFamily[], id: string): CategoryFamily[] {
  const shipped = shippedTable.find((f) => f.id === id);
  if (!shipped) return families;
  const gained = (families.find((f) => f.id === id)?.tags ?? []).filter(
    (t) => !shipped.tags.some((s) => familyTag(s) === familyTag(t)),
  );
  let out = families.map((f) => (f.id === id ? { ...shipped, tags: [] } : f));
  if (!out.some((f) => f.id === id)) out = [...out, { ...shipped, tags: [] }];
  for (const tag of shipped.tags) out = moveTag(out, tag, id);
  const home = familyLookup(shippedTable);
  for (const tag of gained) {
    const owner = home.get(familyTag(tag));
    if (owner && owner !== id && out.some((f) => f.id === owner)) out = moveTag(out, tag, owner);
  }
  return out;
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
