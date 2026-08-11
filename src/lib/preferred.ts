// Skin/layout préféré par entité (§8.6) : mémorise le dernier choisi, avec
// assez d'infos (nom, preview) pour l'afficher immédiatement (sidebar, grille
// bibliothèque) sans re-résoudre la liste des skins/layouts de l'entité.
//
// Lu de façon *synchrone* depuis des expressions de template (`{@const}` par
// carte, potentiellement des centaines dans la bibliothèque) : `peekUiPref`
// (§6.2, `uiPrefs.svelte.ts`) plutôt que l'API asynchrone `getUiPref` —
// `null` le temps très bref du premier chargement, comme
// `nav.sessionCar`/`sessionTrack`.
import type { SkinItem } from "$lib/launch";
import type { LayoutItem } from "$lib/library";
import { StorageKey } from "./storage";
import { peekUiPref, setUiPref } from "./uiPrefs.svelte";

export type PreferredSkin = Pick<SkinItem, "id" | "name" | "preview">;
export type PreferredLayout = Pick<LayoutItem, "id" | "name" | "preview" | "outline">;

function readJSON<T>(key: string): T | null {
  const raw = peekUiPref(key);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function getPreferredSkin(carId: string): PreferredSkin | null {
  return readJSON<PreferredSkin>(StorageKey.preferredSkin(carId));
}

export function setPreferredSkin(carId: string, skin: PreferredSkin): void {
  setUiPref(StorageKey.preferredSkin(carId), JSON.stringify(skin));
}

export function getPreferredLayout(trackId: string): PreferredLayout | null {
  return readJSON<PreferredLayout>(StorageKey.preferredLayout(trackId));
}

export function setPreferredLayout(trackId: string, layout: PreferredLayout): void {
  setUiPref(StorageKey.preferredLayout(trackId), JSON.stringify(layout));
}
