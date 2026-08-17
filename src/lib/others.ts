// Pont typé vers les commandes « Autres mods » (§7.3).
import { invoke } from "@tauri-apps/api/core";

export interface ConflictInfo {
  other_id: string;
  count: number;
}

export interface OtherModRow {
  id: string;
  library_path: string;
  source_archive: string | null;
  imported_at: string;
  is_priority: boolean;
  is_active: boolean;
  junctions: string[];
  conflicts: ConflictInfo[];
  /** Fichiers visant une zone qu'un outil externe synchronise
   * (`extension/config/*​/loaded/`, vao-patches) : Content Manager peut y
   * remplacer la version du mod par la sienne (§4.5.3). */
  externally_managed: number;
}

export interface ActivateOtherResult {
  junctions: number;
  warnings: string[];
}

export function listOtherMods(): Promise<OtherModRow[]> {
  return invoke<OtherModRow[]>("list_other_mods");
}

export function setOtherPriority(id: string, priority: boolean): Promise<void> {
  return invoke<void>("set_other_priority", { id, priority });
}

export function activateOther(id: string): Promise<ActivateOtherResult> {
  return invoke<ActivateOtherResult>("activate_other", { id });
}

export function deactivateOther(id: string): Promise<void> {
  return invoke<void>("deactivate_other", { id });
}

/** Supprime proprement un mod « autre » (jonctions + fichiers + overlay, §7.3). */
export function deleteOtherMod(id: string): Promise<void> {
  return invoke<void>("delete_other_mod", { id });
}

/** Ouvre le dossier de bibliothèque du mod « autre » dans l'explorateur —
 * résolu et ouvert côté backend, comme `openModFolder`. */
export function openOtherModFolder(id: string): Promise<void> {
  return invoke<void>("open_other_mod_folder", { id });
}
