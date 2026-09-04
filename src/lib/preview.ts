// Pont typé vers l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §7).
//
// Le `.glb` ne passe jamais par ici : la commande renvoie une URL servie par
// le protocole `carpreview`, que le chargeur three.js va chercher lui-même
// (§7.2). Le seul objet qui transite est ce petit descripteur.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Préfixe que la conversion pose sur le nom de chaque maillage du mannequin
 * greffé — `DRIVER_MESH_PREFIX` de `kn5-gltf/src/driver.rs`, **à garder
 * identique des deux côtés**.
 *
 * C'est le seul repère qui traverse la conversion : l'arbre est aplati et les
 * maillages sont regroupés par matériau, donc le dummy qui portait le pilote
 * ne survit pas. Il permet à la vue de le montrer ou de le retirer sans
 * reconvertir.
 */
export const DRIVER_MESH_PREFIX = "PITBOX_DRIVER:";

export interface CarPreview {
  /** URL à donner à `GLTFLoader`. */
  url: string;
  triangleCount: number;
  materialCount: number;
  textureCount: number;
  /** `true` quand rien n'a été converti — deuxième affichage d'une voiture. */
  fromCache: boolean;
}

/** Étapes émises pendant une conversion, dans cet ordre. */
export type PreviewStage = "geometry" | "textures" | "writing";

/**
 * Prépare l'aperçu d'une voiture. Rejette avec une clé i18n : `errors.preview*`
 * pour les cas attendus (modèle absent, modèle protégé, demande remplacée par
 * une sélection plus récente), un message technique sinon.
 *
 * `errors.previewSuperseded` n'est pas une panne : c'est la réponse normale
 * quand l'utilisateur a changé de voiture avant la fin de la conversion, et
 * l'appelant doit simplement l'ignorer.
 *
 * `driver` vaut `null` pour ne pas afficher de pilote, sinon l'angle du volant
 * et la tenue imposée. Il fait partie de l'identité de l'entrée de cache —
 * le pilote est greffé dans le `.glb` et sa pose y est cuite — donc le changer
 * demande une conversion, une seule, après quoi les versions déjà vues se
 * rendent instantanément.
 */
export interface DriverView {
  /** Corps substitué à celui de la voiture, `null` = le sien. Le substituer
   * fait tomber la garde-robe de la livrée avec lui, côté backend : elle est
   * nommée d'après l'ancien corps (SPEC-ecran-pilote §10.1). */
  model?: string | null;
  /** Tenue imposée, `null` par pièce = celle que le skin déclare. */
  suit?: string | null;
  gloves?: string | null;
  helmet?: string | null;
}

/**
 * `steer` est l'angle du volant en degrés : il tourne les roues avant, le
 * volant du poste de pilotage et, quand il y en a un, les bras du pilote. Il
 * vaut donc avec ou sans mannequin, d'où sa place hors de `driver`.
 */
export function prepareCarPreview(
  carId: string,
  skinId?: string | null,
  steer = 0,
  driver: DriverView | null = null,
): Promise<CarPreview> {
  return invoke<CarPreview>("prepare_car_preview", { carId, skinId: skinId ?? null, steer, driver });
}

/** Vide le cache d'aperçus, renvoie le nombre d'octets libérés. */
export function clearPreviewCache(): Promise<number> {
  return invoke<number>("clear_preview_cache");
}

/** Octets actuellement occupés par le cache d'aperçus. */
export function previewCacheSize(): Promise<number> {
  return invoke<number>("preview_cache_size");
}

/**
 * Fixe le plafond du cache, en octets, et l'applique immédiatement — baisser
 * le plafond libère la place tout de suite, sans attendre une conversion.
 *
 * Le réglage vit côté frontend (`ui_prefs.json`) : c'est donc lui qui le
 * pousse au backend, au démarrage et à chaque changement. Le backend borne la
 * valeur lui-même, l'appelant n'a pas à s'en occuper.
 */
export function setPreviewCacheCap(bytes: number): Promise<void> {
  return invoke<void>("set_preview_cache_cap", { bytes });
}

/**
 * S'abonne à la progression de la conversion en cours, pour alimenter le
 * squelette de chargement (§7.3). Renvoie la fonction de désabonnement.
 */
export function onPreviewProgress(handler: (stage: PreviewStage) => void): Promise<() => void> {
  return listen<PreviewStage>("preview://progress", (event) => handler(event.payload));
}

/** Repères du rig d'un mannequin, en mètres, dans l'espace du `.glb`
 * (SPEC-ecran-pilote §5.1). Le volant générique s'y pose et la caméra s'y
 * vise : l'application le dessine elle-même, il n'est pas dans le modèle. */
export interface DriverRig {
  /** Poignet gauche puis droit, ou `null` si le mannequin n'a pas d'os de
   * main sous un nom connu — le plateau se passe alors de volant. */
  hands: [[number, number, number], [number, number, number]] | null;
  /** Là où les doigts se referment : 13 cm devant les poignets, mesuré sur
   * douze voitures posées. C'est par là que passe le volant. */
  grip: [[number, number, number], [number, number, number]] | null;
  head: [number, number, number] | null;
  hips: [number, number, number] | null;
}

export interface DriverPreview {
  url: string;
  triangleCount: number;
  fromCache: boolean;
  rig: DriverRig;
}

/**
 * Prépare le mannequin seul, habillé, pour le plateau d'essayage.
 *
 * `null` quand Assetto Corsa n'est pas configuré ou que le corps n'est pas
 * installé : le plateau retombe alors sur l'échantillon plat et la galerie
 * reste utilisable (§12.4), jamais une erreur.
 *
 * La tenue **adoptée** entre dans la clé de cache, donc l'adopter convertit
 * une fois. L'essai au survol, lui, ne repasse jamais par ici : le frontend
 * échange la texture sur place, avec le `.jpg` qu'AC range à côté de son
 * `.dds` — même image, mêmes dimensions (vérifié).
 */
export function prepareDriverPreview(
  carId: string,
  skinId: string | null,
  outfit: DriverView,
): Promise<DriverPreview | null> {
  return invoke<DriverPreview | null>("prepare_driver_preview", { carId, skinId, outfit });
}

/**
 * Le même mannequin, pour la **vignette** d'un corps dans la galerie (§9.1).
 *
 * Deux différences avec le plateau, une seule raison derrière : il y en a
 * quarante-cinq à produire. Aucune n'en périme une autre ni ne périme le
 * plateau, et toutes montrent le corps dans la tenue de la livrée, pour se
 * comparer entre elles.
 */
export function prepareBodyPreview(
  carId: string,
  skinId: string | null,
  body: string,
): Promise<DriverPreview | null> {
  return invoke<DriverPreview | null>("prepare_body_preview", { carId, skinId, body });
}

/**
 * La vignette déjà rendue pour ce corps, ou `null` s'il faut la produire.
 *
 * Ne convertit rien : le backend recalcule le nom d'entrée du mannequin —
 * quelques `stat` — et regarde si le PNG est là. C'est ce qui permet de la
 * demander pour chaque case sans rien payer quand la réponse est oui.
 */
export function bodyThumbnail(carId: string, skinId: string | null, body: string): Promise<string | null> {
  return invoke<string | null>("body_thumbnail", { carId, skinId, body });
}

/** Range la vignette qu'on vient de rendre. Le nom sous lequel elle atterrit
 * appartient au backend : c'est celui de l'entrée de cache du mannequin, donc
 * elle se périme exactement quand celle-ci se périmerait. */
export function saveBodyThumbnail(
  carId: string,
  skinId: string | null,
  body: string,
  png: Uint8Array,
): Promise<string | null> {
  return invoke<string | null>("save_body_thumbnail", { carId, skinId, body, png: Array.from(png) });
}
