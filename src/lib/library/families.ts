// Category families of the car library (INDEX§6.1, TAXO§7.3).
//
// A family groups tags: `Prototype ← lmp1, group c, lmh…`. It is a relation of
// PARENTHOOD, not of identity — attaching `lmp1` to Prototype does not make the
// tag disappear, it stays filterable on its own through the Tag filter.
//
// **An index, not a rule engine.** Nothing here writes anything: the table is
// read from the tag rules (the one table the user can already edit, see
// `rules.rs`), and a car's families are a lookup over the tags it already
// carries. A mod imported tomorrow finds its families without any
// re-evaluation.
//
// Pure on purpose — no i18n, no Svelte — so the lookup is testable on its own.
import type { CategoryFamily } from "$lib/workshop/rules";
import { modTags } from "./cardSearch";
import type { ModCard } from "./library";

export type { CategoryFamily };

/**
 * A tag as families compare it: lowercased, trimmed, without its leading `#`.
 *
 * The same tag reaches a car under two spellings — `#rally` through a merge
 * rule, `rally` straight from its `ui_car.json` — and they must land in the
 * same family, otherwise the count of a tile depends on where the tag came
 * from.
 */
export function familyTag(tag: string): string {
  return tag.trim().toLowerCase().replace(/^#+/, "").trim();
}

/** Tag → family id. A tag listed in two families stays in the FIRST one: the
 * rule is "one tag, one family" (TAXO§7.3), and when a hand-edited table
 * breaks it, the order of the table is the only deterministic tie-break. */
export function familyLookup(families: CategoryFamily[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const f of families) {
    for (const tag of f.tags) {
      const key = familyTag(tag);
      if (key && !m.has(key)) m.set(key, f.id);
    }
  }
  return m;
}

/**
 * What a car's families are read from: its tags - the file's, the rules',
 * the user's (`modTags`) - AND its class.
 *
 * The class is a tag the file declares in another field: `street` or `race`
 * (or what a class rule made of it). Read from the tags alone, a road car
 * whose file only says `"class": "street"` belonged to no family - measured
 * on the first Mod Organizer survey: 35 of its 43 unclassified cars (Tesla
 * Model 3, Subaru 22B, Honda Prelude…). The engine drops the `street`/`race`
 * TAGS precisely because the class carries them.
 */
export function familyTagsOf(
  c: Pick<ModCard, "tags_from_mod" | "tags_from_rule" | "tags_manual" | "car_class">,
): string[] {
  const tags = modTags(c);
  return c.car_class ? [...tags, c.car_class] : tags;
}

/** Families a set of tags reaches, each once, in the order of the table. */
export function familiesOfTags(tags: string[], lookup: Map<string, string>, families: CategoryFamily[]): string[] {
  const hit = new Set<string>();
  for (const tag of tags) {
    const id = lookup.get(familyTag(tag));
    if (id) hit.add(id);
  }
  return families.filter((f) => hit.has(f.id)).map((f) => f.id);
}
