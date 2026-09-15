// Pont typé vers les commandes « Autres mods » (§7.3).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { ResourceFile } from "$lib/library";

export interface ConflictInfo {
  other_id: string;
  count: number;
}

/** Sur quoi un mod se greffe et ce qu'il fait (REFONTE§2). Recalculé côté
 * Rust à chaque listage — jamais stocké, sauf la correction de l'utilisateur. */
export interface Attachment {
  kind: "CAR" | "TRACK" | "APP" | "GAME" | "STANDALONE";
  target_id: string | null;
  target_name: string | null;
  /** Force du signal, de la plus sûre à la plus faible. `ARCHIVE` est une
   * conjecture assumée : un pack peut livrer une police pour une voiture qu'il
   * ne touche pas autrement. */
  signal: "USER" | "PATH" | "CONFIG_NAME" | "ARCHIVE" | "NONE";
  nature: "DOCUMENT" | "CONTENT" | "APPEARANCE" | "BEHAVIOUR" | "DEPENDENCY" | "UNRECOGNISED";
}

export interface OtherModRow {
  id: string;
  library_path: string;
  source_archive: string | null;
  imported_at: string;
  is_priority: boolean;
  is_active: boolean;
  junctions: string[];
  /** Nom repris à la main (REFONTE§6.1) : ces mods portent des noms
   * d'archive, c'est-à-dire rien de lisible. `null` tant que rien n'a été
   * saisi — le nom affiché reste alors `id`. */
  display_name_user: string | null;
  /** Note libre (REFONTE§9). */
  notes_user: string | null;
  /** Rattachement corrigé à la main (REFONTE§2.3), `null` tant qu'on n'a rien corrigé. */
  attachment_user: string | null;
  /** Rattachement effectif et nature, déduits ou corrigés. */
  attachment: Attachment;
  conflicts: ConflictInfo[];
  /** Fichiers visant une zone qu'un outil externe synchronise
   * (`extension/config/*​/loaded/`, vao-patches) : Content Manager peut y
   * remplacer la version du mod par la sienne (§4.5.3). */
  externally_managed: number;
  /** Chemins réellement posés dans le jeu, relatifs à la racine d'AC (§11).
   * Vide quand rien n'est posé — mod désactivé, ou copie plus ancienne que ce
   * qui tourne déjà (règle d'or n°5). */
  placed: string[];
  /** Zones du jeu touchées par le mod, dans l'ordre des onglets (§7.3).
   * Plusieurs quand il en touche plusieurs — il apparaît alors sous chacune. */
  categories: string[];
}

export interface ActivateOtherResult {
  junctions: number;
  warnings: string[];
}

export function listOtherMods(): Promise<OtherModRow[]> {
  return invoke<OtherModRow[]>("list_other_mods");
}

/** Corrige le rattachement d'un mod « autre » (REFONTE§2.3). Chaîne vide = revenir à
 * la déduction. */
export function setOtherAttachment(id: string, target: string): Promise<void> {
  return invoke<void>("set_other_attachment", { id, target });
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

// Cinquième exemplaire du même quintuplet que les mods/apps/packs/sons — voir
// `ResourcesBlock.svelte`.

export function listOtherResources(id: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_other_resources", { id });
}

export function openOtherResource(id: string, relPath: string): Promise<void> {
  return invoke<void>("open_other_resource", { id, relPath });
}

export function otherResourcePath(id: string, relPath: string): Promise<string> {
  return invoke<string>("get_other_resource_path", { id, relPath });
}

export async function otherResourceSrc(id: string, relPath: string): Promise<string> {
  return convertFileSrc(await otherResourcePath(id, relPath));
}

export function readOtherResource(id: string, relPath: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_other_resource", { id, relPath });
}
