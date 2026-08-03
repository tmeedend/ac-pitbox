// Skin/layout préféré par entité (§8.6) : mémorise le dernier choisi, avec
// assez d'infos (nom, preview) pour l'afficher immédiatement (sidebar, grille
// bibliothèque) sans re-résoudre la liste des skins/layouts de l'entité.
import type { SkinItem } from "$lib/launch";
import type { LayoutItem } from "$lib/library";
import { StorageKey } from "./storage";

export type PreferredSkin = Pick<SkinItem, "id" | "name" | "preview">;
export type PreferredLayout = Pick<LayoutItem, "id" | "name" | "preview" | "outline">;

function readJSON<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

export function getPreferredSkin(carId: string): PreferredSkin | null {
  return readJSON<PreferredSkin>(StorageKey.preferredSkin(carId));
}

export function setPreferredSkin(carId: string, skin: PreferredSkin): void {
  localStorage.setItem(StorageKey.preferredSkin(carId), JSON.stringify(skin));
}

export function getPreferredLayout(trackId: string): PreferredLayout | null {
  return readJSON<PreferredLayout>(StorageKey.preferredLayout(trackId));
}

export function setPreferredLayout(trackId: string, layout: PreferredLayout): void {
  localStorage.setItem(StorageKey.preferredLayout(trackId), JSON.stringify(layout));
}
