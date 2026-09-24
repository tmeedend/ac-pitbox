// Editing the brands (Brands tab, TAXO§6, §7) - as DECISIONS on top of the
// catalogue (REGLES§2): `spelling → brand`, keys lowercased. The table works
// like the country aliases, and reuses their operations; what is proper to the
// brands is the PROPOSALS, since no fixed list (the game's countries) tells
// the right name.
import type { MapOverlay } from "$lib/workshop/rules";
import { removeEntry, retarget, setEntry } from "./countryEdit";

type Table = Record<string, string>;

/**
 * Files the brand stored as `from` under `to`: its spellings follow, and
 * `from` becomes a spelling of `to` (TAXO§7). Also how a brand is renamed -
 * filing it under a new name is the same operation. The target may have been
 * a spelling of something else: it is a name now.
 */
export function mergeBrand(overlay: MapOverlay, catalog: Table, effective: Table, from: string, to: string): MapOverlay {
  const target = to.trim();
  if (!target || from === target) return overlay;
  let o = retarget(overlay, catalog, effective, from, target, true);
  o = setEntry(o, catalog, from, target, true);
  if (target.toLowerCase() in effective) o = removeEntry(o, catalog, target);
  return o;
}

/** How "Ignore" remembers a proposal (TAXO§7.2). */
export const mergeKey = (from: string, to: string) => `${from} → ${to}`;

export interface BrandCount {
  name: string;
  cars: number;
}

export interface BrandProposal {
  from: string;
  to: string;
  fromCars: number;
  toCars: number;
}

/** Lowercase, no accents, words separated by single spaces - `Mercedes-Benz`
 * reads `mercedes benz`. */
function words(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
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

/** Two names close enough to ASK about: one is the other's first words
 * (`Alfa` / `Alfa Romeo`, `Lotus` / `Lotus Classic Cars`), or a typo apart
 * (`Lamborgini`). Never merged by this - only proposed. */
function close(a: string, b: string): boolean {
  const [wa, wb] = [words(a), words(b)];
  if (!wa || !wb || wa === wb) return false;
  if (wb.startsWith(`${wa} `) || wa.startsWith(`${wb} `)) return true;
  const [la, lb] = [wa.replace(/ /g, ""), wb.replace(/ /g, "")];
  const shortest = Math.min(la.length, lb.length);
  return shortest >= 5 && distance(la, lb) <= Math.max(1, Math.floor(shortest / 5));
}

/**
 * The merges worth proposing (TAXO§7.2), each brand at most once as the one
 * that goes: into the brand with more cars - the name the library already
 * uses most - or, on a tie, into the longer name, the fuller one (`Alfa` →
 * `Alfa Romeo`). An ignored pair does not come back, in either direction.
 * Sorted by cars concerned, the ones that matter first.
 */
export function brandProposals(brands: BrandCount[], ignored: string[]): BrandProposal[] {
  const out: BrandProposal[] = [];
  const shown = brands.filter((b) => b.cars > 0);
  for (const a of shown) {
    let best: BrandCount | null = null;
    for (const b of shown) {
      if (a === b || !close(a.name, b.name)) continue;
      const goes = a.cars < b.cars || (a.cars === b.cars && a.name.length < b.name.length);
      if (!goes) continue;
      if (ignored.includes(mergeKey(a.name, b.name)) || ignored.includes(mergeKey(b.name, a.name))) continue;
      if (!best || b.cars > best.cars) best = b;
    }
    if (best) out.push({ from: a.name, to: best.name, fromCars: a.cars, toCars: best.cars });
  }
  return out.sort((x, y) => y.fromCars + y.toCars - (x.fromCars + x.toCars) || x.from.localeCompare(y.from));
}
