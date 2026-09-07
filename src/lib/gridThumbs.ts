// Pont typé vers les vignettes régénérées de la grille (docs/SPEC-grille.md §5).
//
// Le `.glb` ne transite jamais par ici : `prepareGridModel` rend une URL servie
// par le protocole `carpreview`, que le chargeur three.js va chercher lui-même.
// Le seul binaire qui passe par l'IPC est le PNG fini, dans l'autre sens, et il
// pèse deux cents kilo-octets — pas vingt mégaoctets.
import { invoke } from "@tauri-apps/api/core";

/**
 * Gabarit de rendu (§5.6), tel que l'écran de réglages le tient.
 *
 * Tout est entier, en unités lisibles : des degrés pour les angles, des
 * pourcentages pour le reste. Ces huit valeurs font **la moitié de l'identité**
 * d'une vignette — le backend les empreinte dans le nom du fichier — donc en
 * changer une périme les 312 images, ce qui est exactement ce que le bouton
 * « Appliquer » promet.
 *
 * Ce qui n'y est pas l'est tout aussi volontairement : format, transparence,
 * exposition fixe et absence de post-traitement sont les propriétés qui
 * garantissent que 312 voitures se comparent, donc elles ne se règlent pas.
 */
export interface GridTemplate {
  azimuth: number;
  elevation: number;
  fov: number;
  margin: number;
  key: number;
  fill: number;
  rim: number;
  shadow: number;
  height: number;
  floor: number;
  reflection: number;
  /** Fond cuit dans l'image, en pourcentage. Zéro laisse le cadre transparent —
   * la carte possède le fond, la règle sur laquelle tout le reste repose. Au-
   * dessus, le rendu porte le sien, seule façon qu'une vignette régénérée soit
   * **indiscernable** d'une `preview.png` d'origine posée à côté d'elle. */
  background: number;
  /** Les deux bouts du dégradé de la carte. Ils appartiennent à la carte et non
   * au rendu — **sauf** quand le fond les cuit, et c'est exactement à ce
   * moment-là qu'ils entrent dans l'empreinte, côté backend. */
  matHi: string;
  matLo: string;
}

/** Ce que la grille a déjà pour une voiture, sans rien convertir. */
export interface GridThumb {
  /** Nom d'entrée, à rendre tel quel pour ranger l'image ou noter un échec.
   * Le frontend ne le fabrique jamais : l'identité d'une vignette appartient au
   * backend, avec les empreintes dont elle est faite. */
  stem: string;
  /** Chemin du PNG quand il est déjà rendu. */
  path: string | null;
  /** Pourquoi cette voiture ne rendra pas — clé i18n, jamais une phrase. */
  failed: string | null;
}

export interface GridThumbStats {
  generated: number;
  failed: number;
  bytes: number;
}

/**
 * Ce qui existe pour cette voiture : l'image, la raison qu'il n'y en ait pas,
 * ou ni l'une ni l'autre — auquel cas il faut la produire.
 *
 * **Ne convertit rien** : quelques `stat` sur le `.kn5`, la livrée et les
 * `ext_config.ini`. C'est ce qui permet de la demander pour chaque carte sans
 * rien payer quand la réponse est « déjà là ».
 */
export function gridThumbnail(carId: string, skinId: string | null, template: GridTemplate): Promise<GridThumb> {
  return invoke<GridThumb>("grid_thumbnail", { carId, skinId, template });
}

/**
 * Convertit une voiture **hors du cache d'aperçus** et rend l'URL du modèle.
 *
 * La voie parallèle du §5.3 : réutiliser `prepareCarPreview` remplirait le cache
 * LRU de 312 voitures que personne n'ouvrira, en évinçant celles qu'on consulte
 * vraiment. Ici le modèle atterrit dans un brouillon vidé avant chaque
 * conversion, et jeté aussitôt l'image rendue.
 */
export function prepareGridModel(carId: string, skinId: string | null): Promise<string> {
  return invoke<string>("prepare_grid_model", { carId, skinId });
}

/** Range le PNG qu'on vient de rendre. */
export function saveGridThumbnail(stem: string, png: Uint8Array): Promise<string> {
  return invoke<string>("save_grid_thumbnail", { stem, png: Array.from(png) });
}

/** Mémorise qu'une voiture ne rendra pas, et pourquoi (§7). Elle ne sera plus
 * retentée tant que le mod lui-même n'aura pas changé — son empreinte est dans
 * le nom d'entrée. */
export function markGridThumbnailFailed(stem: string, reason: string): Promise<void> {
  return invoke<void>("mark_grid_thumbnail_failed", { stem, reason });
}

/**
 * Oublie une vignette — image et marqueur d'échec — pour que la prochaine
 * demande reconvertisse.
 *
 * La sortie de secours d'une image sortie de travers sans que le mod y soit
 * pour quelque chose : un contexte WebGL perdu en cours de rendu, une texture
 * qui n'est pas arrivée jusqu'à la page. Rien ne distingue une telle image
 * d'une bonne une fois écrite — le jugement est celui de l'utilisateur, et
 * c'est le bouton derrière.
 */
export function forgetGridThumbnail(stem: string): Promise<boolean> {
  return invoke<boolean>("forget_grid_thumbnail", { stem });
}

/** Jette le modèle du brouillon. */
export function releaseGridModel(): Promise<void> {
  return invoke<void>("release_grid_model");
}

/** Compteurs pour l'écran de réglages et le rapport de génération (§8.2). */
export function gridThumbnailStats(): Promise<GridThumbStats> {
  return invoke<GridThumbStats>("grid_thumbnail_stats");
}

/** Vide le magasin, échecs compris, et rend les octets libérés. */
export function clearGridThumbnails(): Promise<number> {
  return invoke<number>("clear_grid_thumbnails");
}

/** Efface les images qu'aucun preset vivant ne réclame plus. Rien d'autre ne le
 * ferait : le magasin n'a pas de passe d'éviction, par construction. **Tous**
 * les gabarits en usage, pas seulement le dernier appliqué — la grille en lie
 * un par densité, donc deux jeux d'images coexistent légitimement. */
export function sweepGridTemplates(templates: GridTemplate[]): Promise<number> {
  return invoke<number>("sweep_grid_templates", { templates });
}
