// Pont typé vers les commandes d'édition groupée (§6.3bis). Distinct de
// l'import en masse (voir library.ts) : ici on modifie des mods déjà
// présents en bibliothèque, sélectionnés à plusieurs.
import { invoke } from "@tauri-apps/api/core";
import type { ExportReport } from "./maintenance";

export interface BulkFailure {
  id: string;
  error: string;
}

export interface BulkReport {
  ok: string[];
  failed: BulkFailure[];
  /** Lot interrompu : ce qui n'apparaît ni en succès ni en échec n'a pas été
   * traité du tout (miroir de `BulkReport` dans `src-tauri/src/bulk.rs`). */
  cancelled: boolean;
}

/** Émis sous `bulk:progress` pendant les lots qui touchent au disque. Miroir
 * de `Progress` dans `src-tauri/src/bulk.rs` — les deux se changent ensemble. */
export interface BulkProgress {
  index: number;
  total: number;
  op: string;
  id: string;
}

export interface BulkExportItem {
  id: string;
  report: ExportReport | null;
  error: string | null;
}

export function bulkSetFavorite(ids: string[], favorite: boolean): Promise<void> {
  return invoke<void>("bulk_set_favorite", { ids, favorite });
}

export function bulkSetCategory(ids: string[], category: string | null): Promise<void> {
  return invoke<void>("bulk_set_category", { ids, category });
}

export function bulkAddTag(ids: string[], tag: string): Promise<void> {
  return invoke<void>("bulk_add_tag", { ids, tag });
}

export function bulkRemoveTag(ids: string[], tag: string): Promise<void> {
  return invoke<void>("bulk_remove_tag", { ids, tag });
}

export function bulkActivate(ids: string[]): Promise<BulkReport> {
  return invoke<BulkReport>("bulk_activate", { ids });
}

export function bulkDeactivate(ids: string[]): Promise<BulkReport> {
  return invoke<BulkReport>("bulk_deactivate", { ids });
}

export function bulkDelete(ids: string[]): Promise<BulkReport> {
  return invoke<BulkReport>("bulk_delete", { ids });
}

export function bulkExport(ids: string[], destDir: string): Promise<BulkExportItem[]> {
  return invoke<BulkExportItem[]>("bulk_export", { ids, destDir });
}

/** Demande l'arrêt du lot en cours. Constaté entre deux mods côté Rust. */
export function cancelBulk(): Promise<void> {
  return invoke<void>("cancel_bulk");
}
