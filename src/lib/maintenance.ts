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

export function maintenanceScan(): Promise<MaintenanceReport> {
  return invoke<MaintenanceReport>("maintenance_scan");
}

/** Relit sur le disque les champs cache de tous les mods et réapplique l'ontologie. Renvoie le nb traité.
 * `recalcSize` (§9.4) : recalcule aussi la taille sur disque de chaque mod — plus lent, décoché par défaut. */
export function reindexLibrary(recalcSize: boolean): Promise<number> {
  return invoke<number>("reindex_library", { recalcSize });
}

export function deleteBrokenMod(id: string): Promise<void> {
  return invoke<void>("delete_broken_mod", { id });
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
