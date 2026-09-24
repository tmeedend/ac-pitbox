// The brand the Brands tab should open on arrival - set by the car sheet's
// "edit this brand" (TAXO§10: the sheet is where one thinks of the brand, so
// it is a natural way in, even though it shows the car's own badge). Read and
// cleared by the tab; a state rather than a route parameter, like every
// section of the Workshop (`nav.section` is the tab).
import { requestSection } from "$lib/shell/nav.svelte";

export const brandFocus = $state<{ brand: string | null }>({ brand: null });

export async function editBrand(brand: string): Promise<void> {
  brandFocus.brand = brand;
  if (!(await requestSection("brands"))) brandFocus.brand = null;
}
