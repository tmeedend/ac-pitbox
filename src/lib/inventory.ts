// Pont typé vers l'inventaire des compléments (REFONTE§4).
import { invoke } from "@tauri-apps/api/core";
import type { Attachment } from "$lib/others";

/** Doit rester aligné sur `inventory::RowKind` côté Rust. */
export type RowKind = "SKIN" | "SOUND" | "TRACK_SKIN" | "LAYER" | "DRIVER" | "DOCUMENT" | "OTHER";

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
  /** Zones du jeu touchées, vide pour les sources qui n'en ont pas. Affichées
   * à la place du type sur un mod « autre », dont le type ne dit rien : une
   * police se lit « Font », pas « Mod ». */
  areas: string[];
  /** Déployé ou non, `null` quand la notion ne s'applique pas : une livrée ne
   * s'active pas, elle est là. */
  active: boolean | null;
  priority: boolean;
  has_note: boolean;
  source_archive: string | null;
  imported_at: string;
  size_bytes: number;
}

export function listInventory(): Promise<InventoryRow[]> {
  return invoke<InventoryRow[]>("list_inventory");
}

/** Ce qui est greffé sur une entité (REFONTE§4.3) : la contrepartie de
 * l'inventaire, du côté de l'hôte. Une déduction ratée n'y coûte qu'un
 * raccourci manquant — le mod, lui, reste dans l'inventaire. */
export function listAttached(entityId: string): Promise<InventoryRow[]> {
  return invoke<InventoryRow[]>("list_attached", { entityId });
}
