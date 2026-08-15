// Pont typé vers les commandes L5 (maintenance & export, §9).
import { invoke } from "@tauri-apps/api/core";

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

export interface RelativizeReport {
  converted: number;
  already_relative: number;
  unrecognized: string[];
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
export function deleteBrokenMod(id: string): Promise<void> {
  return invoke<void>("delete_broken_mod", { id });
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

/** Convertit en chemins relatifs à la bibliothèque toutes les lignes overlay encore
 * écrites en absolu (§11) — répare une bibliothèque copiée depuis une autre machine.
 * Ne touche aucun fichier, uniquement une réécriture en base ; sûr à rejouer. */
export function relativizeLibraryPaths(): Promise<RelativizeReport> {
  return invoke<RelativizeReport>("relativize_library_paths");
}

export function removeOrphanJunction(kind: string, id: string): Promise<void> {
  return invoke<void>("remove_orphan_junction", { kind, id });
}

/** Désinstalle tout un pack (§4.7) : supprime chaque mod du pack. Renvoie le nb supprimé. */
export function deletePack(pack: string): Promise<number> {
  return invoke<number>("delete_pack", { pack });
}

/** Exporte la version active d'un mod en archive autonome dans `destDir` (§9.1). */
export function exportMod(id: string, destDir: string): Promise<ExportReport> {
  return invoke<ExportReport>("export_mod", { id, destDir });
}
