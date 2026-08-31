// Pont typé vers la surcharge de pilote (docs/SPEC-preview-3d-kn5.md §4.6ter).
//
// AC habille un pilote par le `skin.ini` de la livrée, sous le nom du
// mannequin que la voiture impose. **Deux natures d'objet, deux
// comportements** (`docs/SPEC-ecran-pilote.md` §1.3) : la tenue ne tient qu'à
// un fichier de skin, donc elle se choisit ; le corps vit dans `driver3d.ini`,
// donc dans le `data.acd` que le serveur de course vérifie, donc le substituer
// ne vaut que dans l'aperçu.
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

/** Époque de la boîte à casques d'un corps. Clé i18n (`driver.era.<clé>`),
 * pas un libellé : `null` = mannequin qui nomme ses images autrement. */
export type DriverEra = "modern" | "1980s" | "1970s" | "1960s";

/** Ce qu'une voiture donnée permet de choisir. */
export interface DriverChoices {
  /** Le corps sur lequel ces listes ont été calculées : celui de la voiture,
   * ou celui qu'on lui a substitué. */
  model: string;
  /** `true` quand ce corps n'est pas celui de la voiture — le mode « corps
   * substitué », qui ne vaut que dans l'aperçu (SPEC-ecran-pilote §10). */
  substituted: boolean;
  era: DriverEra | null;
  suits: WardrobeOption[];
  gloves: WardrobeOption[];
  /** Filtrés par l'époque du corps : un casque moderne ne change rien sur
   * un mannequin des années 80. Vide sur un mannequin de mod qui utilise ses
   * propres noms de fichiers — on préfère ne rien proposer à proposer un choix
   * sans effet. */
  helmets: WardrobeOption[];
}

/** Un mannequin installé, tel qu'il s'offre au choix (§9.1). */
export interface BodyOption {
  /** Nom de fichier sans extension : `driver_60`. Ne se traduit pas. */
  id: string;
  era: DriverEra | null;
}

/**
 * Les tenues qui marcheront réellement sur le mannequin de cette voiture.
 *
 * `null` quand Assetto Corsa n'est pas configuré, quand la voiture ne nomme
 * aucun mannequin, ou quand celui-ci n'est pas installé — trois cas où il n'y
 * a simplement rien à proposer, jamais une erreur.
 */
export function listDriverChoices(carId: string, body: string | null = null): Promise<DriverChoices | null> {
  return invoke<DriverChoices | null>("list_driver_choices", { carId, body });
}

/**
 * Les corps installés, pour la galerie des corps (§9.1).
 *
 * Ceux qu'on ne peut pas prendre — illisibles, sans squelette — n'y sont pas :
 * une option qu'on ne peut pas choisir n'a pas à être montrée (§9.3). Liste
 * vide, jamais une erreur, quand Assetto Corsa n'est pas configuré.
 */
export function listDriverBodies(): Promise<BodyOption[]> {
  return invoke<BodyOption[]>("list_driver_bodies");
}
