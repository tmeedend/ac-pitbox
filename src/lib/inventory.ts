// Pont typé vers l'inventaire des compléments (refonte §4).
import { invoke } from "@tauri-apps/api/core";
import type { Attachment } from "$lib/others";

/** Doit rester aligné sur `inventory::RowKind` côté Rust. */
export type RowKind = "SKIN" | "SOUND" | "TRACK_SKIN" | "LAYER" | "OTHER";

export interface InventoryRow {
  /** Unique dans l'inventaire : les cinq tables ont chacune leurs ids, et rien
   * n'empêche une livrée et une couche de porter le même. */
  uid: string;
  kind: RowKind;
  /** Id dans sa propre table, pour les commandes qui l'attendent. */
  id: string;
  name: string;
  /** Identifiant technique, montré sous le nom : c'est ce qui permet de
   * renommer sans rien perdre (§4.2). */
  tech_id: string;
  attachment: Attachment;
  active: boolean;
  priority: boolean;
  has_note: boolean;
  source_archive: string | null;
  imported_at: string;
  size_bytes: number;
}

export function listInventory(): Promise<InventoryRow[]> {
  return invoke<InventoryRow[]>("list_inventory");
}
