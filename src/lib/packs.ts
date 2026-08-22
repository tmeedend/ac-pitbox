// Pont typé vers les commandes « fiche de pack » (§4.4).
//
// Un pack n'est qu'un nom d'archive en base, porté par la colonne
// `source_pack` de chaque mod livré avec. Il possède pourtant ses propres
// ajouts au jeu et ses propres ressources — ce que cette fiche montre.
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { ModCard } from "./library";
import type { ExtraFile } from "./library";
import type { ResourceFile } from "./library";

export interface PackDetail {
  name: string;
  members: ModCard[];
  extras: ExtraFile[];
  /** Somme des tailles sur disque des membres, octets. */
  members_bytes: number;
  /** Taille des ajouts au jeu du pack, octets. */
  extras_bytes: number;
  imported_at: string | null;
}

export function getPackDetail(pack: string): Promise<PackDetail> {
  return invoke<PackDetail>("get_pack_detail", { pack });
}

export function listPackExtras(pack: string): Promise<ExtraFile[]> {
  return invoke<ExtraFile[]>("list_pack_extras", { pack });
}

export function listPackResources(pack: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_pack_resources", { pack });
}

export function openPackResource(pack: string, relPath: string): Promise<void> {
  return invoke<void>("open_pack_resource", { pack, relPath });
}

export function packResourcePath(pack: string, relPath: string): Promise<string> {
  return invoke<string>("get_pack_resource_path", { pack, relPath });
}

/** URL `asset://` d'une ressource du pack, pour un `<img>` (§4.5.2). */
export async function packResourceSrc(pack: string, relPath: string): Promise<string> {
  return convertFileSrc(await packResourcePath(pack, relPath));
}

export async function readPackResource(pack: string, relPath: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_pack_resource", { pack, relPath });
}
