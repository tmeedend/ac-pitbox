// The header form of a car or a track - name, brand, category, year (the pencil
// next to the title) - as pure functions: what a typed text changes, and what
// the form hands back. The component only draws it.
//
// Nothing here writes: the values are the user's decisions, kept in
// Pit Box's base beside what the mod's own files say (golden rule 1), and every
// field can go back to the original.

/** One field of the identity: what is shown, what the mod's own files say
 * (where "back to original" goes), and whether the shown value is a decision
 * of the user's. */
export interface IdentityField<T> {
  value: T | null;
  original: T | null;
  overridden: boolean;
}

/** What the form hands back: only the fields that changed. A value is a new
 * decision, `null` goes back to the original, an absent key is untouched. */
export interface IdentityPatch {
  name?: string | null;
  brand?: string | null;
  /** A car's category; `null` goes back to the one the rules compute. */
  category?: string | null;
  /** A track's categories: what to add to its list and what to take off. */
  categories?: CategoriesChange;
  year?: number | null;
}

/** A track's categories, as deltas on the list shown when the form opened. */
export interface CategoriesChange {
  add: string[];
  remove: string[];
}

/** A text field with a list of existing values to pick from, or to depart from. */
export type SuggestedField = IdentityField<string> & { suggestions: () => Promise<string[]> };

export interface FicheIdentity {
  name: IdentityField<string>;
  /** Cars only: a track has no brand. */
  brand?: SuggestedField;
  /** Cars only: one category. `original` is what the rules compute, `overridden` whether the user filed the car. */
  category?: SuggestedField;
  /** Tracks only: a track has a list of categories. `original` is the list the
   * rules compute, `overridden` whether the user added or took one off. */
  categories?: {
    value: string[];
    original: string[];
    overridden: boolean;
    suggestions: () => Promise<string[]>;
  };
  year: IdentityField<number>;
  onsave: (patch: IdentityPatch) => void;
}

/** A year the form accepts: four digits, in a range no AC mod leaves. */
export function parseYear(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d{4}$/.test(trimmed)) return null;
  const year = Number(trimmed);
  return year >= 1800 && year <= 2100 ? year : null;
}

/** What one text field changes. What is shown already changes nothing;
 * emptying a decision, or typing the original back over it, gives it up. */
export function textChange(draft: string, field: IdentityField<string>): { change: string | null } | null {
  const next = draft.trim();
  if (next === "") return field.overridden ? { change: null } : null;
  if (next === field.value) return null;
  return next === field.original && field.overridden ? { change: null } : { change: next };
}

/** Same, for the year. An invalid text is not a change: the form refuses it
 * before it gets here. */
export function yearChange(draft: string, field: IdentityField<number>): { change: number | null } | null {
  if (draft.trim() === "") return field.overridden ? { change: null } : null;
  const year = parseYear(draft);
  if (year === null || year === field.value) return null;
  return year === field.original && field.overridden ? { change: null } : { change: year };
}

/** A year typed in the form that it must refuse. The year already shown is never
 * refused, whatever it is: the tech sheet takes years the form would not
 * (FICHE§8), and opening the form on one must not block the other fields. */
export function yearRefused(draft: string, field: IdentityField<number>): boolean {
  const text = draft.trim();
  if (text === "" || parseYear(text) !== null) return false;
  return text !== String(field.value ?? "");
}

/** How two spellings of a category compare: with a `#`, in lower case
 * (`categories::key` on the Rust side). */
function categoryKey(tag: string): string {
  return `#${tag.trim().replace(/^#+/, "").toLowerCase()}`;
}

/** What the category field changes. Same rules as [`textChange`], except that
 * `gt3` and `#GT3` are one category. The text is handed back as typed: the
 * backend knows the spelling of the categories that exist. */
export function categoryChange(draft: string, field: IdentityField<string>): { change: string | null } | null {
  const next = draft.trim();
  if (next === "") return field.overridden ? { change: null } : null;
  const key = categoryKey(next);
  if (field.value != null && key === categoryKey(field.value)) return null;
  return field.original != null && key === categoryKey(field.original) && field.overridden
    ? { change: null }
    : { change: next };
}

/** What a track's category list gains and loses between the list shown when the
 * form opened and the one it ends with; `null` when they are the same. The
 * order of the list is not a decision (the backend orders it). */
export function categoriesChange(draft: string[], shown: string[]): CategoriesChange | null {
  const keyed = (list: string[]) => new Map(list.map((tag) => [categoryKey(tag), tag]));
  const was = keyed(shown);
  const now = keyed(draft);
  const add = [...now].filter(([key]) => !was.has(key)).map(([, tag]) => tag);
  const remove = [...was].filter(([key]) => !now.has(key)).map(([, tag]) => tag);
  return add.length || remove.length ? { add, remove } : null;
}

/** Whether a category list holds `tag`, whatever its spelling. */
export function hasCategory(list: string[], tag: string): boolean {
  const key = categoryKey(tag);
  return list.some((c) => categoryKey(c) === key);
}
