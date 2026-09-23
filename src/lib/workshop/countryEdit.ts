// Editing the countries (Countries tab, TAXO§6) - as DECISIONS on top of the
// catalogue (REGLES§2), for two tables keyed the same way:
//   - the ALIASES normalise a country a mod declares (`u.s.a.` → United
//     States), always;
//   - the TAGS give a country to a mod declaring none (`germany` → Germany),
//     only then. Two tables because a tag must never rewrite what the author
//     declared.
// Both are `key → country name`, keys in their compared form (lowercased,
// trimmed). Every function returns a new overlay; the effective tables come
// back from `taxonomy.rs` after each save and are never merged here (see
// `familyEdit.ts` for why).
import type { MapOverlay } from "$lib/workshop/rules";

type Table = Record<string, string>;

const key = (s: string) => s.trim().toLowerCase();

function copy(o: MapOverlay): { set: Table; removed: string[] } {
  return { set: { ...(o.set ?? {}) }, removed: [...(o.removed ?? [])] };
}

/** The keys leading to `name` in an effective table, sorted. */
export function keysTo(effective: Table, name: string): string[] {
  return Object.entries(effective)
    .filter(([, to]) => to === name)
    .map(([k]) => k)
    .sort();
}

/**
 * `k → name`. Restating what the catalogue says removes the decision instead:
 * the entry is back on the catalogue and follows it.
 *
 * `spelling`: the table is the aliases, where a key equal to the name itself
 * (`japan → Japan`) is dead weight that looks like a decision, and is not
 * written. NOT for the tags: `usa → USA` says a tag gives a country, which is
 * exactly what that table is for - caught by the merge test.
 */
export function setEntry(o: MapOverlay, catalog: Table, k: string, name: string, spelling = false): MapOverlay {
  const kk = key(k);
  if (!kk || (spelling && kk === key(name))) return o;
  const next = copy(o);
  next.removed = next.removed.filter((r) => r !== kk);
  if (catalog[kk] === name) delete next.set[kk];
  else next.set[kk] = name;
  return next;
}

/** Removes an entry. A shipped one gets a tombstone - an absence would let the
 * next catalogue bring it back. */
export function removeEntry(o: MapOverlay, catalog: Table, k: string): MapOverlay {
  const kk = key(k);
  const next = copy(o);
  delete next.set[kk];
  if (kk in catalog && !next.removed.includes(kk)) next.removed.push(kk);
  return next;
}

/**
 * Retargets every entry leading to `from` onto `to` - the half of a merge that
 * carries the spellings along (TAXO§7). Otherwise a mod written `Holland` would
 * keep landing on the old name while one written `Netherlands` moved.
 */
export function retarget(
  o: MapOverlay,
  catalog: Table,
  effective: Table,
  from: string,
  to: string,
  spelling = false,
): MapOverlay {
  let next = o;
  for (const k of keysTo(effective, from)) next = setEntry(next, catalog, k, to, spelling);
  return next;
}

/**
 * Merges the country stored as `from` into `to`: its spellings and its tags
 * follow, and `from` itself becomes a spelling of `to`. Also how a country is
 * RENAMED - attaching it to another name is the same operation. The target
 * may have been a spelling of something else: it is a name now.
 */
export function attachCountry(
  aliases: { overlay: MapOverlay; catalog: Table; effective: Table },
  tags: { overlay: MapOverlay; catalog: Table; effective: Table },
  from: string,
  to: string,
): { aliases: MapOverlay; tags: MapOverlay } {
  if (from === to) return { aliases: aliases.overlay, tags: tags.overlay };
  let a = retarget(aliases.overlay, aliases.catalog, aliases.effective, from, to, true);
  a = setEntry(a, aliases.catalog, from, to, true);
  if (key(to) in aliases.effective) a = removeEntry(a, aliases.catalog, to);
  return { aliases: a, tags: retarget(tags.overlay, tags.catalog, tags.effective, from, to) };
}

/** Removes every decision touching `name`: its entries are back as shipped. */
export function restoreEntries(o: MapOverlay, catalog: Table, name: string): MapOverlay {
  const next = copy(o);
  for (const [k, v] of Object.entries(next.set)) if (v === name || catalog[k] === name) delete next.set[k];
  next.removed = next.removed.filter((k) => catalog[k] !== name);
  return next;
}

/** Whether the user decided anything about `name` in this table - the flag of
 * the list (TAXO§6.1). */
export function touches(o: MapOverlay, catalog: Table, name: string): boolean {
  return (
    Object.entries(o.set ?? {}).some(([k, v]) => v === name || catalog[k] === name) ||
    (o.removed ?? []).some((k) => catalog[k] === name)
  );
}

/** "Ignore" on a proposal (TAXO§7.2): remembered, reversible. */
export function ignoreCountry(ignored: string[], value: string): string[] {
  return [...new Set([...ignored, value])].sort();
}

export function unignoreCountry(ignored: string[], value: string): string[] {
  return ignored.filter((v) => v !== value);
}

/** Case, accents and anything but letters folded away, for comparing. */
function fold(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
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
