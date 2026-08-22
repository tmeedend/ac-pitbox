// Pont typé vers les commandes L5 (maintenance & export, §9).
import { invoke } from "@tauri-apps/api/core";
import { bumpLibraryVersion } from "./libraryVersion.svelte";

export interface BrokenMod {
  id: string;
  kind: string;
  name: string | null;
  reason: string;
}

export interface OrphanJunction {
  kind: string;
  id: string;
  path: string;
}

export interface MaintenanceReport {
  broken: BrokenMod[];
  orphans: OrphanJunction[];
  orphan_subs: OrphanSub[];
}

export interface ExportReport {
  archive: string;
  included: string[];
  warnings: string[];
}

export interface RepairProjectionsReport {
  repaired: number;
  already_ok: number;
  failed: string[];
}

export interface ReinstallOutcome {
  id: string;
  error: string;
}

export interface RepairAllReport {
  projections: RepairProjectionsReport;
  /** Mods actifs redéployés depuis la bibliothèque. */
  redeployed: number;
  redeploy_errors: ReinstallOutcome[];
  reinstalled: string[];
  reinstall_errors: ReinstallOutcome[];
}

/** Skin ou son dont la voiture/le circuit parent n'existe plus (§9.3). */
export interface OrphanSub {
  id: string;
  sub_type: string;
  parent_id: string;
  name: string;
}

/** Efface les skins/sons sans parent. Jamais automatique : ils sont conservés à
 * la suppression d'un mod pour qu'un réimport du même id les retrouve. */
export function purgeOrphanSubs(): Promise<number> {
  return invoke<number>("purge_orphan_subs");
}

export function maintenanceScan(): Promise<MaintenanceReport> {
  return invoke<MaintenanceReport>("maintenance_scan");
}

/** Relit sur le disque les champs cache de tous les mods et réapplique l'ontologie. Renvoie le nb traité.
 * `recalcSize` (§9.4) : recalcule aussi la taille sur disque de chaque mod — plus lent, décoché par défaut. */
export function reindexLibrary(recalcSize: boolean): Promise<number> {
  return invoke<number>("reindex_library", { recalcSize });
}

/** Supprime un mod de la bibliothèque : fichiers (toutes versions) + junction + overlay.
 * Action distincte de la désactivation (§10) — utilisable pour tout mod, pas seulement cassé. */
export async function deleteBrokenMod(id: string): Promise<void> {
  await invoke<void>("delete_broken_mod", { id });
  bumpLibraryVersion();
}

/** Ce qu'a réellement fait `deleteModVersion` (§10). */
export interface DeleteVersionOutcome {
  /** Fichiers récupérables dans la corbeille Windows ; faux = la corbeille a
   * refusé (volume réseau, version plus grosse que son quota) et ils ont été
   * effacés définitivement. */
  recycled: boolean;
  /** Octets rendus au disque. */
  freed_bytes: number;
  /** Profils qui épinglaient cette version et pointent désormais celle en place. */
  profiles_repointed: string[];
}

/** Profils épinglant une version (§10) — à lire avant de proposer la
 * suppression, la confirmation devant pouvoir les nommer. */
export function profilesUsingVersion(versionId: string): Promise<string[]> {
  return invoke<string[]>("profiles_using_version", { versionId });
}

/** Supprime une version non active d'un mod (§10) : ses fichiers partent à
 * la corbeille Windows avec l'archive source qu'elle avait fait conserver. La
 * version en place n'est pas supprimable — le backend refuse. */
export async function deleteModVersion(versionId: string): Promise<DeleteVersionOutcome> {
  const outcome = await invoke<DeleteVersionOutcome>("delete_mod_version", { versionId });
  bumpLibraryVersion();
  return outcome;
}

/** Réinstalle un mod depuis son archive/dossier source conservé (§10/§11, réglage
 * « conserver l'archive source »). Réextrait et remplace les fichiers de la version active. */
export function reinstallFromArchive(id: string): Promise<void> {
  return invoke<void>("reinstall_from_archive", { id });
}

/** Réparation générale (§9.3) : recalcule tout ce qui dérive de la bibliothèque —
 * projections skin/circuit cassées, puis redéploiement des mods actifs (donc aussi
 * leurs ajouts au jeu). Si `reinstallBroken`, réinstalle en plus depuis l'archive
 * source conservée chaque mod cassé qui en a une : la seule étape qui touche la
 * bibliothèque elle-même, d'où l'opt-in. */
export function repairAll(reinstallBroken: boolean): Promise<RepairAllReport> {
  return invoke<RepairAllReport>("repair_all", { reinstallBroken });
}

export function removeOrphanJunction(kind: string, id: string): Promise<void> {
  return invoke<void>("remove_orphan_junction", { kind, id });
}

/** Désinstalle tout un pack (§4.4) : supprime chaque mod du pack. Renvoie le nb supprimé. */
export async function deletePack(pack: string): Promise<number> {
  const n = await invoke<number>("delete_pack", { pack });
  bumpLibraryVersion();
  return n;
}

/** Exporte la version active d'un mod en archive autonome dans `destDir` (§9.1). */
export function exportMod(id: string, destDir: string): Promise<ExportReport> {
  return invoke<ExportReport>("export_mod", { id, destDir });
}
