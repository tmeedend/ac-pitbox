// Pont typé vers les commandes L6 / §12bis (contenu de base + sous-éléments).
import { invoke } from "@tauri-apps/api/core";

export interface SubModRow {
  id: string;
  sub_type: "SKIN" | "SOUND" | "TRACK_SKIN" | "TRACK_MOD";
  parent_id: string;
  name: string;
  library_path: string;
  source_archive: string | null;
  is_active: boolean;
  /** Faux si fourni avec le contenu initial du mod (§4.6bis) — non supprimable individuellement. */
  removable: boolean;
  imported_at: string;
}

/** Indexe le contenu de base Kunos présent dans content/ (§12bis.1). Renvoie le nb indexé. */
export function indexStockContent(): Promise<number> {
  return invoke<number>("index_stock_content");
}

/** Sous-éléments rattachés à une entité (skins/sons d'une voiture, §12bis.3). */
export function listSubMods(parentId: string): Promise<SubModRow[]> {
  return invoke<SubModRow[]>("list_sub_mods", { parentId });
}

/** Tous les sous-éléments d'un type, vue transversale (§12bis.3). */
export function listSubsByType(subType: string): Promise<SubModRow[]> {
  return invoke<SubModRow[]>("list_subs_by_type", { subType });
}

/** Active un mod de son (bascule exclusive du sfx/, §12bis.2). */
export function activateSound(subId: string): Promise<void> {
  return invoke<void>("activate_sound", { subId });
}

/** Restaure le son d'origine d'une voiture (§12bis.2). */
export function restoreSound(parentId: string): Promise<void> {
  return invoke<void>("restore_sound", { parentId });
}

/** Reconnaît les skins de circuit fournis avec le mod (§4.6bis) — à appeler avant de les lister. */
export function syncTrackSkins(trackId: string): Promise<void> {
  return invoke<void>("sync_track_skins", { trackId });
}

/** Skins de circuit actuellement actifs (§4.6bis, plusieurs possibles). */
export function listActiveTrackSkins(trackId: string): Promise<string[]> {
  return invoke<string[]>("list_active_track_skins", { trackId });
}

/** Active/désactive un skin de circuit (§4.6bis, pas exclusif). */
export function setTrackSkinActive(trackId: string, skinName: string, active: boolean): Promise<void> {
  return invoke<void>("set_track_skin_active", { trackId, skinName, active });
}

/** Supprime un sous-élément (skin/son) de l'overlay (§12bis.3). */
export function deleteSubMod(id: string): Promise<void> {
  return invoke<void>("delete_sub_mod", { id });
}
