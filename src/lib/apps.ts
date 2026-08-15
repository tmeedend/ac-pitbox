// Pont typé vers les commandes Apps (§12bis.4).
import { invoke } from "@tauri-apps/api/core";
import type { ResourceFile } from "$lib/library";

export interface AppItem {
  id: string;
  source_archive: string | null;
  imported_at: string;
  active: boolean;
}

export function listApps(): Promise<AppItem[]> {
  return invoke<AppItem[]>("list_apps");
}

export function activateApp(id: string): Promise<void> {
  return invoke<void>("activate_app", { id });
}

export function deactivateApp(id: string): Promise<void> {
  return invoke<void>("deactivate_app", { id });
}

/** Supprime proprement une app (junction + fichiers + overlay, §12bis.4). */
export function deleteApp(id: string): Promise<void> {
  return invoke<void>("delete_app", { id });
}

/** Fichiers annexes d'une app (§4.5.2, même mécanisme que les mods voiture/circuit),
 * lus en direct sur disque — pas de cache, un dépôt manuel apparaît sans réimport. */
export function listAppResources(id: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_app_resources", { id });
}

/** Ouvre un fichier annexe d'une app avec l'application par défaut de l'OS (§4.5.2). */
export function openAppResource(id: string, relPath: string): Promise<void> {
  return invoke<void>("open_app_resource", { id, relPath });
}

/** Ouvre le dossier bibliothèque de l'app dans l'explorateur. */
export function openAppFolder(id: string): Promise<void> {
  return invoke<void>("open_app_folder", { id });
}
