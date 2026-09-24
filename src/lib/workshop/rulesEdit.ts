// What the Rules screen does to the overlay (REGLES§5, §8), as pure functions:
// every gesture is a DECISION added to or taken off `rules-overlay.json`,
// never an edit of the catalogue. Kept apart from the component so the three
// verbs that are easy to confuse - disable, fork, delete - are tested.
//
// Every function returns a new overlay; the backend normalises what it
// receives (ids of new rules, origin of new forks), so the screen never has to
// invent an id nor fingerprint a rule.
import type {
  BrandFix,
  ClassFix,
  ListOverlay,
  NameToTag,
  RulesOverlay,
  RuleRow,
  SetRule,
  TagMerge,
} from "./rules";

export type Section =
  | "brand_fix"
  | "name_to_tag"
  | "class_fix"
  | "car_tag_merge"
  | "drivetrain"
  | "aspiration"
  | "engine_config"
  | "engine_pos"
  | "gearbox"
  | "track_tag_merge";

export type AnyRule = BrandFix | NameToTag | ClassFix | TagMerge | SetRule;

export const CAR_SECTIONS: Section[] = [
  "brand_fix",
  "name_to_tag",
  "class_fix",
  "car_tag_merge",
  "drivetrain",
  "aspiration",
  "engine_config",
  "engine_pos",
  "gearbox",
];
export const TRACK_SECTIONS: Section[] = ["track_tag_merge"];

function clone(o: RulesOverlay): RulesOverlay {
  return JSON.parse(JSON.stringify(o)) as RulesOverlay;
}

function list(o: RulesOverlay, s: Section): ListOverlay<AnyRule> {
  const l = ((o[s] as ListOverlay<AnyRule> | undefined) ??= {});
  l.disabled ??= [];
  l.forks ??= {};
  l.own ??= [];
  return l;
}

/** The switch of a row: immediate, no confirmation, and the rule keeps
 * receiving improvements while off (REGLES§5). */
export function setEnabled(o: RulesOverlay, s: Section, id: string, on: boolean): RulesOverlay {
  const out = clone(o);
  const l = list(out, s);
  l.disabled = l.disabled!.filter((d) => d !== id);
  if (!on) l.disabled.push(id);
  return out;
}

/** Saves an edited rule. His own rule is replaced in place; a shipped rule -
 * or an existing fork - becomes (stays) his frozen copy. A fork edited again
 * keeps its origin, so "a new version exists" is not forgotten. */
export function editRule(o: RulesOverlay, s: Section, id: string, rule: AnyRule): RulesOverlay {
  const out = clone(o);
  const l = list(out, s);
  const i = l.own!.findIndex((r) => r.id === id);
  if (i >= 0) {
    l.own![i] = { ...rule, id };
  } else {
    l.forks![id] = { rule: { ...rule, id }, forked_from: l.forks![id]?.forked_from ?? "" };
  }
  return out;
}

/** "Restore the default": the fork goes, the shipped rule - its current
 * version - applies again. */
export function restoreDefault(o: RulesOverlay, s: Section, id: string): RulesOverlay {
  const out = clone(o);
  delete list(out, s).forks![id];
  return out;
}

/** Deletes a rule of his: his own, or his fork of a rule the catalogue has
 * retired. A shipped rule is never deleted - it is switched off. */
export function removeRule(o: RulesOverlay, s: Section, id: string): RulesOverlay {
  const out = clone(o);
  const l = list(out, s);
  l.own = l.own!.filter((r) => r.id !== id);
  delete l.forks![id];
  l.disabled = l.disabled!.filter((d) => d !== id);
  return out;
}

/** A new rule of his, first of his section: his rules run before the
 * catalogue's, and the newest is the one being worked on. No id - the
 * backend gives the next `own-N`. */
export function addRule(o: RulesOverlay, s: Section, rule: AnyRule): RulesOverlay {
  const out = clone(o);
  const l = list(out, s);
  const { id: _drop, ...rest } = rule;
  l.own = [rest as AnyRule, ...l.own!];
  return out;
}

export function setCatalogOn(o: RulesOverlay, on: boolean): RulesOverlay {
  const out = clone(o);
  if (on) delete out.catalog_off;
  else out.catalog_off = true;
  return out;
}

/** Whether a row offers "restore the default". */
export function isFork(row: RuleRow<AnyRule>): boolean {
  return row.origin === "fork";
}

/** Whether a row can be deleted: anything of the user's. */
export function isDeletable(row: RuleRow<AnyRule>): boolean {
  return row.origin === "own";
}

// --- The track categories allowlist ------------------------------------------

/** `Hillclimb ` → `#hillclimb`. */
export function normCategory(s: string): string {
  const v = s.trim().toLowerCase().replace(/^#+/, "");
  return v ? `#${v}` : "";
}

function order(o: RulesOverlay) {
  const t = (o.track_categories ??= {});
  t.removed ??= [];
  t.added ??= [];
  return t;
}

/** A shipped category switched off (removed) or back on. */
export function setCategoryOn(o: RulesOverlay, name: string, on: boolean): RulesOverlay {
  const out = clone(o);
  const t = order(out);
  t.removed = t.removed!.filter((r) => r !== name);
  if (!on) t.removed.push(name);
  return out;
}

/** Adds a category; one the catalogue ships and he had removed comes back
 * instead of being added twice. */
export function addCategory(o: RulesOverlay, raw: string, shipped: string[]): RulesOverlay {
  const name = normCategory(raw);
  if (!name) return o;
  const out = clone(o);
  const t = order(out);
  if (shipped.includes(name)) t.removed = t.removed!.filter((r) => r !== name);
  else if (!t.added!.includes(name)) t.added!.push(name);
  return out;
}

export function removeCategory(o: RulesOverlay, name: string): RulesOverlay {
  const out = clone(o);
  const t = order(out);
  t.added = t.added!.filter((a) => a !== name);
  if (t.order) t.order = t.order.filter((a) => a !== name);
  return out;
}

/** Moves a category in the effective order - the order becomes his. */
export function moveCategory(o: RulesOverlay, effective: string[], i: number, dir: -1 | 1): RulesOverlay {
  const j = i + dir;
  if (j < 0 || j >= effective.length) return o;
  const out = clone(o);
  const next = [...effective];
  [next[i], next[j]] = [next[j], next[i]];
  order(out).order = next;
  return out;
}

// --- The row editor ----------------------------------------------------------

/** A rule as the inline editor holds it: up to three text fields. `a` is what
 * the rule looks for, `b` what it gives, `c` the extra tags of a class rule. */
export interface RuleForm {
  a: string;
  b: string;
  c: string;
}

const join = (l: string[]) => l.join(", ");
const tags = (s: string) =>
  s
    .split(",")
    .map((x) => x.trim().toLowerCase())
    .filter(Boolean);

export function toForm(s: Section, r: AnyRule): RuleForm {
  const x = r as Partial<BrandFix & NameToTag & ClassFix & TagMerge & SetRule>;
  switch (s) {
    case "brand_fix":
      return { a: x.name_contains ?? "", b: x.set_brand ?? "", c: "" };
    case "name_to_tag":
      return { a: x.name_contains ?? "", b: join(x.add ?? []), c: "" };
    case "class_fix":
      return { a: join(x.from ?? []), b: x.set_class ?? "", c: join(x.add ?? []) };
    case "car_tag_merge":
    case "track_tag_merge":
      return { a: join(x.from ?? []), b: join(x.to ?? []), c: "" };
    default:
      return { a: join(x.from ?? []), b: x.set ?? "", c: "" };
  }
}

/** The rule a form describes, `null` while it would match everything or give
 * nothing - an empty "name contains" matches every car. */
export function fromForm(s: Section, f: RuleForm): AnyRule | null {
  const a = f.a.trim();
  const b = f.b.trim();
  switch (s) {
    case "brand_fix":
      return a && b ? { name_contains: a.toLowerCase(), set_brand: b } : null;
    case "name_to_tag": {
      const add = tags(b);
      return a && add.length ? { name_contains: a.toLowerCase(), add } : null;
    }
    case "class_fix": {
      const from = tags(a);
      const add = tags(f.c);
      const set_class = b === "race" || b === "street" ? b : null;
      return from.length && (set_class || add.length) ? { from, set_class, add } : null;
    }
    case "car_tag_merge":
    case "track_tag_merge": {
      const from = tags(a);
      const to = tags(b);
      return from.length && to.length ? { from, to } : null;
    }
    default: {
      const from = tags(a);
      return from.length && b ? { from, set: b } : null;
    }
  }
}

/** What a row reads: what the rule looks for, and what it gives. */
export function describe(s: Section, r: AnyRule): { from: string; to: string } {
  const f = toForm(s, r);
  if (s === "class_fix") return { from: f.a, to: [f.b, f.c].filter(Boolean).join(" + ") };
  return { from: f.a, to: f.b };
}
