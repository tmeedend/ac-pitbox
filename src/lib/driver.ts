// Pont typé vers la surcharge de pilote (docs/SPEC-preview-3d-kn5.md §4.6ter).
//
// AC habille un pilote par le `skin.ini` de la livrée, sous le nom du
// mannequin que la voiture impose. Le mannequin, lui, ne se choisit pas : il
// vit dans `driver3d.ini`, donc dans le `data.acd` que le serveur de course
// vérifie. La tenue, elle, ne tient qu'à un fichier de skin.
import { invoke } from "@tauri-apps/api/core";

/** Un dossier de garde-robe, tel qu'il s'offre au choix. */
export interface WardrobeOption {
  /** Valeur telle que `skin.ini` l'écrit : `plain/red`. */
  id: string;
  /** Ce qu'on affiche. Les noms de dossier AC ne se traduisent pas. */
  label: string;
  /** Chemin de la vignette qu'AC range à côté des `.dds`, s'il y en a une. */
  thumbnail: string | null;
}

/** Ce qu'une voiture donnée permet de choisir. */
export interface DriverChoices {
  /** Le mannequin que la voiture impose — affiché, jamais modifiable. */
  model: string;
  suits: WardrobeOption[];
  gloves: WardrobeOption[];
  /** Filtrés par l'époque du mannequin : un casque moderne ne change rien sur
   * un mannequin des années 80. Vide sur un mannequin de mod qui utilise ses
   * propres noms de fichiers — on préfère ne rien proposer à proposer un choix
   * sans effet. */
  helmets: WardrobeOption[];
}

/**
 * Les tenues qui marcheront réellement sur le mannequin de cette voiture.
 *
 * `null` quand Assetto Corsa n'est pas configuré, quand la voiture ne nomme
 * aucun mannequin, ou quand celui-ci n'est pas installé — trois cas où il n'y
 * a simplement rien à proposer, jamais une erreur.
 */
export function listDriverChoices(carId: string): Promise<DriverChoices | null> {
  return invoke<DriverChoices | null>("list_driver_choices", { carId });
}
