// Pont typé vers les commandes Apps (§12bis.4).
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { ExtraFile, ResourceFile } from "$lib/library";

export interface AppItem {
  id: string;
  source_archive: string | null;
  imported_at: string;
  active: boolean;
  /** "python" | "lua" : dit si l'app suit la convention historique d'AC ou
   * celle de CSP, et donc sous quel `apps/<langue>/` elle est posée. */
  lang: string;
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

/** URL `asset://` d'une ressource d'app, pour un `<img>` (§4.5.2) — jumeau de
 * `modResourceSrc`. */
export async function appResourceSrc(id: string, relPath: string): Promise<string> {
  return convertFileSrc(await appResourcePath(id, relPath));
}

/** Chemin absolu d'une ressource d'app, non converti — jumeau de
 * `modResourcePath`, pour le générateur de miniatures. */
export function appResourcePath(id: string, relPath: string): Promise<string> {
  return invoke<string>("get_app_resource_path", { id, relPath });
}

/** Octets bruts d'une ressource d'app (§4.5.2) — jumeau de `readModResource` :
 * l'IPC plutôt que `asset://`, pour ne pas dépendre du CORS du protocole. */
export function readAppResource(id: string, relPath: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_app_resource", { id, relPath });
}

/** Ce que l'app installe hors de son dossier `apps/<langue>/<id>` (§4.5.3).
 * Une app en a autant qu'une voiture : configs CSP, textures, fichiers de
 * `cfg/` livrés à côté de son dossier. */
export function listAppExtras(id: string): Promise<ExtraFile[]> {
  return invoke<ExtraFile[]>("list_app_extras", { id });
}
