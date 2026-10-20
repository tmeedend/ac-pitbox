// The header form of a car or a track - name, brand, year (the pencil next to
// the title) - as pure functions: what a typed text changes, and what the form
// hands back. The component only draws it.
//
// Nothing here writes: the three values are the user's decisions, kept in
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
  year?: number | null;
}

export interface FicheIdentity {
  name: IdentityField<string>;
  /** Cars only: a track has no brand. */
  brand?: IdentityField<string> & { suggestions: () => Promise<string[]> };
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
