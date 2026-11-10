// The term a taxonomy tab of Classement should open on arrival - set by a
// screen that names a term without owning its tab: the car sheet's "edit this
// brand" (TAXO§10). Read and cleared by the tab; a state rather than a route
// parameter, like every section of the Workshop (`nav.section` is the tab).
import { requestSection } from "$lib/shell/nav.svelte";

export const termFocus = $state<{ section: string | null; term: string | null; tracks: boolean }>({
  section: null,
  term: null,
  tracks: false,
});

/** Opens `section` on `term`. `tracks`: the Tracks half of Families, where the
 * track categories are. */
export async function focusTerm(section: string, term: string, tracks = false): Promise<void> {
  termFocus.section = section;
  termFocus.term = term;
  termFocus.tracks = tracks;
  if (!(await requestSection(section))) termFocus.term = null;
}

/** The term asked for `section`, cleared once read; `null` when none. */
export function takeTermFocus(section: string): string | null {
  if (termFocus.section !== section || !termFocus.term) return null;
  const term = termFocus.term;
  termFocus.term = null;
  termFocus.section = null;
  return term;
}

export function editBrand(brand: string): Promise<void> {
  return focusTerm("brands", brand);
}
