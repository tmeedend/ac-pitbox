// Editing the country aliases (Countries tab, TAXO§6).
//
// A country is a TERM: the name it is stored under (the game's English name,
// which is what carries a flag) and the spellings that lead to it. The table
// is `alias → name`, keys in their compared form (lowercased, trimmed) - the
// same shape `rules::canonical_country` reads, so what this module writes is
// exactly what the harmonisation applies.
//
// Every function returns a NEW table, for the same reason as `familyEdit.ts`.
import type { CountryAliases } from "$lib/workshop/rules";

const key = (s: string) => s.trim().toLowerCase();

/** The spellings that lead to `name`, sorted - what the detail of a country
 * lists. */
export function aliasesOf(a: CountryAliases, name: string): string[] {
  return Object.entries(a.map)
    .filter(([, to]) => to === name)
    .map(([from]) => from)
    .sort();
}

/** One more spelling for `name`. An alias equal to the name itself is dead
 * weight that looks like a decision, and is not written. */
export function addAlias(a: CountryAliases, alias: string, name: string): CountryAliases {
  const k = key(alias);
  if (!k || k === key(name)) return a;
  return { ...a, map: { ...a.map, [k]: name } };
}

export function removeAlias(a: CountryAliases, alias: string): CountryAliases {
  const map = { ...a.map };
  delete map[key(alias)];
  return { ...a, map };
}

/**
 * Merges the country stored as `from` into `to` (TAXO§7): `from` becomes a
 * spelling of `to`, and so does every spelling that led to `from` - otherwise
 * a mod written `Holland` would keep landing on the old name while one written
 * `Netherlands ` moved. Also how a country is RENAMED: attaching it to another
 * name is the same operation.
 */
export function attachCountry(a: CountryAliases, from: string, to: string): CountryAliases {
  if (from === to) return a;
  const map: Record<string, string> = {};
  for (const [k, v] of Object.entries(a.map)) map[k] = v === from ? to : v;
  if (key(from) !== key(to)) map[key(from)] = to;
  // The target may have been an alias of something else: it is a name now.
  delete map[key(to)];
  return { ...a, map, ignored: (a.ignored ?? []).filter((v) => v !== from) };
}

/** "Ignore" on a proposal (TAXO§7.2): remembered, reversible. */
export function ignoreCountry(a: CountryAliases, value: string): CountryAliases {
  const ignored = [...new Set([...(a.ignored ?? []), value])].sort();
  return { ...a, ignored };
}

export function unignoreCountry(a: CountryAliases, value: string): CountryAliases {
  return { ...a, ignored: (a.ignored ?? []).filter((v) => v !== value) };
}

/** Whether the spellings of `name` differ from those the app shipped - the
 * flag of the list (TAXO§6.1). */
export function isCountryCurated(a: CountryAliases, shipped: CountryAliases, name: string): boolean {
  const mine = aliasesOf(a, name);
  const theirs = aliasesOf(shipped, name);
  return mine.length !== theirs.length || mine.some((m, i) => m !== theirs[i]);
}

/** Puts the spellings of `name` back as shipped: the ones added are removed,
 * the shipped ones restored - even if they had been attached elsewhere. */
export function restoreCountry(a: CountryAliases, shipped: CountryAliases, name: string): CountryAliases {
  const map: Record<string, string> = {};
  for (const [k, v] of Object.entries(a.map)) if (v !== name) map[k] = v;
  for (const [k, v] of Object.entries(shipped.map)) if (v === name) map[k] = v;
  return { ...a, map };
}

/** Case, accents and anything but letters folded away, for comparing. */
function fold(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z]/g, "");
}

function distance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let prev = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const cur = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = cur;
    }
  }
  return row[b.length];
}

/**
 * The game country a value unknown to the game most likely means, or `null`
 * (TAXO§7.2: proposals ranked by proximity).
 *
 * Only a close call is proposed - a typo (`Swizerland`), or one name inside
 * the other (`Czech` / `Czech Republic`). **A proposal, never a decision**: the
 * user merges or ignores, and without a close candidate the tab offers the
 * full list instead of a guess, since a wrong suggestion one clicks through is
 * a wrong flag on every mod of that spelling.
 */
export function closestCountry(value: string, names: string[]): string | null {
  const v = fold(value);
  if (v.length < 3) return null;
  let best: { name: string; d: number } | null = null;
  for (const name of names) {
    const n = fold(name);
    if (!n) continue;
    const d = n.includes(v) || v.includes(n) ? 0 : distance(v, n);
    if (d <= Math.max(1, Math.floor(Math.min(v.length, n.length) / 5)) && (!best || d < best.d)) best = { name, d };
  }
  return best?.name ?? null;
}
