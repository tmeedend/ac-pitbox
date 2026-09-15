// Fiche d'une livrée (§8.3) — voiture (`SKIN`) ou circuit (`TRACK_SKIN`).
//
// À part des autres bindings de sous-éléments (`enginesound.ts` pour les sons)
// pour la même raison qu'eux : ce qui décrit une livrée n'a rien à voir avec ce
// qui décrit un bank, et les mélanger ferait un module qui parle de deux choses.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface SkinFile {
  /** Chemin relatif au dossier stocké, séparateurs `/`. */
  path: string;
  sizeBytes: number;
}

export interface SkinDetail {
  id: string;
  subType: "SKIN" | "TRACK_SKIN";
  name: string;
  /** Nom déclaré par `ui_skin.json`, quand il en porte un. */
  uiName: string | null;
  parentId: string;
  parentName: string | null;
  sourceArchive: string | null;
  importedAt: string;
  /** N'a de sens que pour un habillage de circuit (§8). */
  isActive: boolean;
  removable: boolean;
  sizeBytes: number;
  displayNameUser: string | null;
  notesUser: string | null;
  folder: string;
  /** Projetée dans le `skins/` de l'hôte : faux = le jeu ne la charge pas. */
  projected: boolean;
  preview: string | null;
  livery: string | null;
  files: SkinFile[];
}

export function skinDetail(subId: string): Promise<SkinDetail> {
  return invoke<SkinDetail>("skin_detail", { subId });
}

export function openSkinFolder(subId: string): Promise<void> {
  return invoke<void>("open_skin_folder", { subId });
}

/** Chemin local → URL affichable. Même conversion que `library::previewSrc`,
 * répétée ici plutôt qu'importée : la fiche n'a rien d'autre à prendre à la
 * bibliothèque, et l'import tirerait tout son module. */
export function skinImageSrc(path: string | null): string | null {
  return path ? convertFileSrc(path) : null;
}
