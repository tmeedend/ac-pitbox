// Brand logos as the whole application reads them (TAXO§3 to §5, `logos.rs`):
// the logo elected for each brand - drawn where the BRAND is shown (index
// tile, Brands tab) - and the car badges whose background is baked in, drawn
// on the light plate wherever the CAR is shown.
//
// One state for every screen: the listing that measures it reads every
// badge once (a second, from the cache after that). Reloaded when the
// library changes (`AppShell` watches `libraryVersion`) and after a choice in
// the Brands tab.
import { invoke } from "@tauri-apps/api/core";
import { invokeSafe } from "$lib/invokeSafe";

export type LogoBackground = "transparent" | "baked" | "opaque";

export interface LogoVariant {
  hash: string;
  path: string;
  width: number;
  height: number;
  background: LogoBackground;
  /** Cars shipping this very file: the vote (TAXO§4). */
  cars: number;
}

/** What the user decided for a brand; empty = automatic. */
export interface BrandPref {
  variant?: string;
  custom?: string;
  plaque?: boolean;
}

export interface BrandLogo {
  brand: string;
  path: string | null;
  plaque: boolean;
  choice: "auto" | "variant" | "custom";
  /** In election order: the first is the automatic choice. */
  variants: LogoVariant[];
  pref: BrandPref;
}

interface LogosView {
  brands: Record<string, BrandLogo>;
  plaque: string[];
}

export const brandLogos = $state<{ brands: Record<string, BrandLogo>; plaque: Record<string, true> }>({
  brands: {},
  plaque: {},
});

/** A READ: `invokeSafe` - a missing logo is better than a frozen shell. */
export async function loadBrandLogos(): Promise<void> {
  const v = await invokeSafe<LogosView | null>("get_brand_logos", undefined, null);
  if (!v) return;
  brandLogos.brands = v.brands;
  brandLogos.plaque = Object.fromEntries(v.plaque.map((p) => [p, true as const]));
}

/** The brand's logo (file path) and whether it goes on the plate. */
export function logoOf(brand: string): { path: string; plaque: boolean } | null {
  const b = brandLogos.brands[brand];
  return b?.path ? { path: b.path, plaque: b.plaque } : null;
}

/** Whether a car's badge goes on the plate: a property of the file (TAXO§5). */
export function isPlaque(path: string | null | undefined): boolean {
  return !!path && path in brandLogos.plaque;
}

/** WRITES: plain `invoke`, the error goes to the caller, who shows it. */
export async function saveBrandLogo(brand: string, pref: BrandPref): Promise<void> {
  await invoke("save_brand_logo", { brand, pref });
  await loadBrandLogos();
}

export async function importBrandLogo(brand: string, path: string): Promise<void> {
  await invoke("import_brand_logo", { brand, path });
  await loadBrandLogos();
}
