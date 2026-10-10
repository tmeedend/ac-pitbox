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
import { mark } from "$lib/timing";

export type LogoBackground = "transparent" | "baked" | "opaque";

export interface LogoVariant {
  hash: string;
  path: string;
  width: number;
  height: number;
  background: LogoBackground;
  /** Cars shipping this very file: the vote (TAXO§4). */
  cars: number;
  /** A team's or modder's badge, on cars filed here without their file
   * naming the brand (VRC on Peugeot): offered, never elected (TAXO§4). */
  borrowed: boolean;
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
  /** In election order: the first one not borrowed is the automatic choice. */
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
  mark("logos");
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

/** A logo offered online (TAXO§9.1, `logo_search.rs`). */
export interface LogoCandidate {
  /** File name at its source, for the tooltip. */
  name: string;
  /** What is downloaded when picked - a rendition rather than a huge original. */
  url: string;
  /** A PNG preview, an SVG's included. */
  thumb: string;
  /** The size of what is downloaded; null for a vector file, or when the
   * source does not say. */
  width: number | null;
  height: number | null;
  format: "svg" | "png";
  /** car-logos-dataset, the logo Wikidata gives the brand, or a Commons file. */
  source: "carlogos" | "wikidata" | "commons";
}

export interface LogoPage {
  candidates: LogoCandidate[];
  /** Where the next page starts, `null` past the last. */
  next: number | null;
}

/** A search the user asked for: plain `invoke`, its failure is said where he
 * is looking. */
export function searchBrandLogos(query: string, offset: number): Promise<LogoPage> {
  return invoke<LogoPage>("search_brand_logos", { query, offset });
}

/** WRITES: downloads the logo he picked and makes it the brand's. */
export async function adoptBrandLogo(brand: string, url: string): Promise<void> {
  await invoke("adopt_brand_logo", { brand, url });
  await loadBrandLogos();
}
