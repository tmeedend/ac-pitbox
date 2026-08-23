// Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
//
// Partagés par l'écran Réglages (où on les change) et par `CarPreview3D`
// (qui les applique) : d'où un `$state` de module plutôt qu'un état par
// composant, pour qu'un aperçu déjà ouvert suive le réglage sans être remonté.
//
// Persistance via `ui_prefs.json` (`uiPrefs.svelte.ts`), jamais `localStorage`
// — règle d'or n°6 : un réglage qui doit survivre à un redémarrage n'a rien à
// faire dans un stockage que WebView2 n'écrit pas forcément sur disque. Aucun
// fichier Rust dédié : une poignée de nombres et trois choix ne le justifient
// pas.
//
// Un seul de ces réglages intéresse le backend — le plafond du cache — et
// c'est **ce module qui le lui pousse** (`setPreviewCacheCap`), au chargement
// et à chaque changement. Le backend ne lit jamais `ui_prefs.json` : le schéma
// de ce fichier appartient au frontend (voir l'en-tête de `ui_prefs.rs`).
import { setPreviewCacheCap } from "./preview";
import { getUiPrefs, setUiPrefs } from "./uiPrefs.svelte";

/** Clés de `ui_prefs.json`. `preview3d` seule existait avant les autres : elle
 * porte la bascule photo/3D de la zone héros, et garde donc son nom. */
const KEYS = {
  enabled: "pitbox.preview3d",
  zoom: "pitbox.preview3d.zoom",
  azimuth: "pitbox.preview3d.azimuth",
  elevation: "pitbox.preview3d.elevation",
  height: "pitbox.preview3d.height",
  spin: "pitbox.preview3d.spin",
  fov: "pitbox.preview3d.fov",
  exposure: "pitbox.preview3d.exposure",
  light: "pitbox.preview3d.light",
  reflection: "pitbox.preview3d.reflection",
  reflectionBlur: "pitbox.preview3d.reflectionBlur",
  reflectionReach: "pitbox.preview3d.reflectionReach",
  pool: "pitbox.preview3d.pool",
  shadow: "pitbox.preview3d.shadow",
  cacheMb: "pitbox.preview3d.cacheMb",
  intro: "pitbox.preview3d.intro",
  quality: "pitbox.preview3d.quality",
} as const;

/**
 * Bornes et valeurs par défaut, en unités lisibles par l'utilisateur : des
 * degrés pour les angles, des pourcentages pour ce qui n'a pas d'unité
 * naturelle.
 *
 * Les défauts de cadrage sont ceux **choisis par l'utilisateur** sur l'aperçu
 * de l'écran Réglages, et non plus ceux mesurés sur les `preview.jpg` Kunos :
 * une vue plus basse et plus proche, tournant moitié moins vite. Ils ne
 * s'appliquent qu'aux installations neuves — une préférence déjà écrite dans
 * `ui_prefs.json` a la priorité, et le bouton « rétablir » du groupe est ce
 * qui les fait apparaître chez quelqu'un qui y a déjà touché.
 */
export const PREVIEW3D_RANGES = {
  /** 100 % = la distance calculée sur la taille du modèle. Bornes choisies pour
   * rester dans les limites de zoom des contrôles souris (`minDistance` /
   * `maxDistance`), sinon le réglage serait annulé au premier rendu. */
  zoom: { min: 50, max: 200, step: 5, default: 110 },
  /** Rotation de la caméra autour de l'axe vertical, en degrés. Le défaut est
   * l'angle des `preview.jpg` Kunos : trois-quarts avant **gauche**, comme
   * toutes les photos du jeu (§15 point 7). */
  azimuth: { min: 0, max: 359, step: 1, default: 318 },
  /** Plongée de la caméra, en degrés au-dessus de l'horizon. Plafonnée sous
   * l'angle polaire maximal des contrôles. */
  elevation: { min: 0, max: 80, step: 1, default: 6 },
  /** Hauteur de la caméra, en pourcentage du rayon du modèle. Monte ou
   * descend le point visé **sans toucher à la plongée** : c'est ce qui décide
   * de la place de la voiture dans le cadre, là où l'angle décide de ce qu'on
   * voit de son toit. 0 vise le centre du modèle. */
  height: { min: -60, max: 60, step: 1, default: -8 },
  /** Vitesse du plateau tournant. 0 % = plateau à l'arrêt. */
  spin: { min: 0, max: 200, step: 5, default: 50 },
  /** Focale, exprimée en champ de vision vertical (degrés). 20° reproduit le
   * téléobjectif des `preview.jpg` Kunos ; descendre allonge encore la focale,
   * monter dramatise. La **distance est recalculée** avec, pour que la voiture
   * garde sa taille dans le cadre : c'est le zoom qui décide de la taille, la
   * focale de la perspective. Sans ça les deux curseurs se marcheraient
   * dessus. */
  fov: { min: 10, max: 50, step: 1, default: 20 },
  /** Exposition du rendu (`toneMappingExposure`). Le réglage le plus utile de
   * ce groupe : les mods sortent avec des textures d'éclat très inégal, et
   * certains rendent sombres sans que rien d'autre soit en cause. */
  exposure: { min: 50, max: 200, step: 5, default: 100 },
  /** Intensité de l'éclairage du studio (`scene.environmentIntensity`).
   * **Pas** `material.envMapIntensity`, qui n'a aucun effet quand
   * l'environnement vient de la scène — vérifié au banc, voir
   * `docs/SPEC-preview-3d-kn5.md`. */
  light: { min: 0, max: 300, step: 5, default: 100 },
  /** Intensité du reflet de la voiture au sol. 0 % retire la passe miroir
   * entière, pas seulement son opacité : c'est un second rendu de la scène,
   * autant ne pas le payer quand on n'en veut pas. */
  reflection: { min: 0, max: 100, step: 5, default: 85 },
  /** Flou du reflet, en **dixièmes** — la mécanique de ce module travaille sur
   * des entiers (`clamp` arrondit), et un dixième se voit à l'œil. 5 = 0,5.
   * C'est ce qui sépare le sol de salon du sol mouillé de jeu vidéo.
   * Plafonné à 4 : au-delà le reflet n'est plus qu'une tache, la plage haute
   * ne servait qu'à rendre le curseur imprécis là où il compte (retour
   * utilisateur). */
  reflectionBlur: { min: 0, max: 40, step: 1, default: 5 },
  /** Portée du reflet, en pourcentage de la demi-largeur du sol. Le défaut
   * initial (30 %) éteignait le reflet **avant** d'atteindre la voiture : la
   * borne basse est donc haute exprès. */
  reflectionReach: { min: 20, max: 150, step: 5, default: 75 },
  /** Flaque de lumière peinte sous la voiture. Existait pour signaler qu'il y
   * a un sol ; le reflet le fait désormais mieux, d'où un défaut abaissé. */
  pool: { min: 0, max: 200, step: 5, default: 85 },
  /** Opacité de l'ombre portée. */
  shadow: { min: 0, max: 100, step: 5, default: 50 },
  /** Plafond du cache d'aperçus, **en mégaoctets**. En Mo et non en Go parce
   * que toute la mécanique de ce module travaille sur des entiers (`clamp`
   * arrondit) : un pas d'un demi-gigaoctet s'exprime en 512 Mo sans flottant.
   * Bornes reprises telles quelles côté Rust, qui borne à son tour — la
   * validation ne dépend pas de l'écran qui envoie la valeur. */
  cacheMb: { min: 512, max: 20480, step: 512, default: 2048 },
} as const;

/**
 * Les groupes de réglages, et l'ordre dans lequel ils se présentent.
 *
 * Un groupe est ce qu'un bouton « rétablir » remet à zéro, et ce qu'un écran
 * affiche d'un bloc. Le plafond de cache n'en fait partie d'aucun : il partage
 * la mécanique des curseurs mais touche au disque, et le remettre à 2 Go au
 * passage évincerait des entrées pour un bouton qui parle de cadrage ou de sol.
 */
export const PREVIEW3D_GROUPS = {
  /** Réglé sur la fiche, où le résultat est sous les yeux — c'est le seul
   * groupe que porte aussi le panneau compact posé sur l'aperçu. */
  framing: ["zoom", "azimuth", "elevation", "height", "fov", "spin"],
  light: ["exposure", "light"],
  floor: ["reflection", "reflectionBlur", "reflectionReach", "pool", "shadow"],
} as const satisfies Record<string, readonly NumericKey[]>;

export type Preview3dGroup = keyof typeof PREVIEW3D_GROUPS;

/** Effet appliqué au plateau tournant quand un modèle s'affiche.
 * `ramp` monte en douceur jusqu'à la vitesse réglée ; `launch` part vite et
 * ralentit jusqu'à elle. */
export const INTRO_EFFECTS = ["none", "ramp", "launch"] as const;
export type IntroEffect = (typeof INTRO_EFFECTS)[number];

/** Qualité de **rendu** — et d'elle seule : rien ici ne touche à la conversion
 * du modèle, donc changer de niveau n'invalide aucune entrée de cache et
 * s'applique à l'image suivante. */
export const PREVIEW_QUALITIES = ["standard", "high", "ultra"] as const;
export type PreviewQuality = (typeof PREVIEW_QUALITIES)[number];

type NumericKey = keyof typeof PREVIEW3D_RANGES;

/** Type écrit à la main plutôt qu'inféré de l'objet : sans lui, TypeScript
 * réduit la cible d'un `values[key] = …` (clé prise dans une union) à `never`. */
export type Preview3dPrefs = {
  enabled: boolean;
  intro: IntroEffect;
  quality: PreviewQuality;
} & Record<NumericKey, number>;

function clamp(key: NumericKey, value: number): number {
  const range = PREVIEW3D_RANGES[key];
  if (!Number.isFinite(value)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(value)));
}

// `$state` de module : lu par les composants, écrit par les setters ci-dessous.
const values: Preview3dPrefs = $state({
  enabled: true,
  // Le plus discret des deux effets par défaut : une montée en douceur se
  // remarque à peine, là où un départ lancé est un parti pris.
  intro: "ramp",
  quality: "high",
  zoom: PREVIEW3D_RANGES.zoom.default,
  azimuth: PREVIEW3D_RANGES.azimuth.default,
  elevation: PREVIEW3D_RANGES.elevation.default,
  height: PREVIEW3D_RANGES.height.default,
  spin: PREVIEW3D_RANGES.spin.default,
  fov: PREVIEW3D_RANGES.fov.default,
  exposure: PREVIEW3D_RANGES.exposure.default,
  light: PREVIEW3D_RANGES.light.default,
  reflection: PREVIEW3D_RANGES.reflection.default,
  reflectionBlur: PREVIEW3D_RANGES.reflectionBlur.default,
  reflectionReach: PREVIEW3D_RANGES.reflectionReach.default,
  pool: PREVIEW3D_RANGES.pool.default,
  shadow: PREVIEW3D_RANGES.shadow.default,
  cacheMb: PREVIEW3D_RANGES.cacheMb.default,
});

/** Compteur de remises à zéro de la vue. Ce n'est pas un réglage : il n'est ni
 * lu ni écrit sur disque, il ne sert qu'à ce qu'un aperçu monté ailleurs voie
 * passer la demande. Un compteur et non un booléen — deux remises à zéro
 * successives doivent produire deux événements distincts. */
let resets = $state(0);

export function preview3dResets(): number {
  return resets;
}

/** Replace la voiture selon les réglages et la remet à tourner. */
export function resetPreview3dView(): void {
  resets += 1;
}

// Chargé une fois pour toute la session, dès l'import du module : un aperçu
// monté tout de suite ne doit pas s'ouvrir sur les valeurs par défaut puis
// sauter sur celles de l'utilisateur.
let loaded: Promise<void> | null = null;

/** Relit un choix dans sa liste d'options. Une valeur inconnue — fichier
 * édité à la main, réglage retiré d'une version à l'autre — retombe sur le
 * défaut plutôt que de se propager dans une comparaison qui ne matchera
 * jamais. */
function oneOf<T extends string>(raw: string | null, allowed: readonly T[], fallback: T): T {
  return allowed.includes(raw as T) ? (raw as T) : fallback;
}

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((stored) => {
    if (stored[KEYS.enabled] !== null) values.enabled = stored[KEYS.enabled] === "1";
    values.intro = oneOf(stored[KEYS.intro], INTRO_EFFECTS, values.intro);
    values.quality = oneOf(stored[KEYS.quality], PREVIEW_QUALITIES, values.quality);
    for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
      const raw = stored[KEYS[key]];
      if (raw !== null) values[key] = clamp(key, Number(raw));
    }
    // `stored` suit `values` : ce qui vient d'être lu **est** ce qui est sur
    // disque, donc rien n'est en attente au démarrage.
    Object.assign(stored, $state.snapshot(values));
    // Le backend part sur son propre défaut tant que personne ne lui a rien
    // dit : c'est ici, et seulement ici, qu'il apprend le réglage enregistré.
    pushCacheCap();
  });
  return loaded;
}

/** Dernier plafond réellement transmis au backend. Appliquer un plafond
 * déclenche une éviction — la répéter à l'identique reparcourrait le dossier
 * du cache pour rien. */
let pushedCacheMb = 0;

function pushCacheCap(): void {
  if (values.cacheMb === pushedCacheMb) return;
  pushedCacheMb = values.cacheMb;
  setPreviewCacheCap(values.cacheMb * 1024 * 1024).catch((e) => {
    // Best-effort : le plafond précédent reste en vigueur, rien ne casse.
    // Tracé quand même — un cache qui ignore son réglage sans un mot serait
    // indiagnosticable.
    console.error("set_preview_cache_cap", e);
    pushedCacheMb = 0;
  });
}

void ensureLoaded();

/** Les réglages courants, réactifs. Lire `preview3dPrefs().zoom` et non une
 * copie déstructurée : la réactivité se perd à la déstructuration. */
export function preview3dPrefs() {
  return values;
}

/** Attend que les réglages enregistrés soient là — pour un appelant qui ne peut
 * pas se permettre de construire une scène sur les valeurs par défaut. */
export function preview3dReady(): Promise<void> {
  return ensureLoaded();
}

// --- Persistance : appliquée à l'écran, écrite seulement sur demande ------
//
// **Deux états, et c'est tout l'intérêt** : `values` est ce qu'on voit, `stored`
// est ce qui est sur disque. Bouger un curseur ne touche que le premier —
// l'aperçu suit à l'image suivante — et rien ne part sur disque avant que
// l'utilisateur ne l'ait demandé.
//
// La version précédente écrivait toute seule, sur minuterie et au démontage des
// curseurs. Conséquences relevées par l'utilisateur, et elles étaient justes :
// le bouton Enregistrer ne décidait de rien, quitter l'écran validait en
// silence, et **il n'y avait aucun moyen de revenir en arrière** — le réglage
// d'avant était perdu dès le premier mouvement de souris.
//
// Même modèle que l'onglet Général (`Settings.svelte`), qui compare `config` à
// `savedConfig` : la garde de navigation propose d'enregistrer ou d'annuler, et
// annuler revient sur l'aperçu déjà appliqué.

/** Les valeurs telles qu'elles sont **sur disque**. Toute différence avec
 * `values` est un changement en attente. */
const stored: Preview3dPrefs = $state({ ...values });

const pending = $derived(JSON.stringify(values) !== JSON.stringify(stored));

/** Vrai tant qu'un réglage bougé n'a pas été enregistré. */
export function preview3dDirty(): boolean {
  return pending;
}

/** Écrit tous les réglages et rend la main quand c'est **sur disque** — ce que
 * le bouton Enregistrer a besoin de savoir pour annoncer « Enregistré » sans
 * mentir. Tout est réécrit, pas seulement ce qui a bougé : le fichier de
 * préférences est relu et réécrit en entier de toute façon, et suivre les
 * clés modifiées une par une n'achèterait rien. */
export async function savePreview3dPrefs(): Promise<void> {
  const entries: Record<string, string> = {
    [KEYS.enabled]: values.enabled ? "1" : "0",
    [KEYS.intro]: values.intro,
    [KEYS.quality]: values.quality,
  };
  for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
    entries[KEYS[key]] = String(values[key]);
  }
  await setUiPrefs(entries);
  Object.assign(stored, $state.snapshot(values));
  // Le plafond de cache ne part au backend qu'ici : l'appliquer déclenche une
  // éviction, et une éviction est **irréversible** — la faire à chaque pas de
  // curseur effacerait des entrées pour un réglage que l'utilisateur peut
  // encore annuler.
  pushCacheCap();
}

/** Revient sur les valeurs enregistrées, y compris à l'écran : c'est le
 * « annuler » de la garde de navigation. */
export function revertPreview3dPrefs(): void {
  Object.assign(values, $state.snapshot(stored));
}

export function setPreview3dEnabled(enabled: boolean): void {
  values.enabled = enabled;
}

export function setPreview3dValue(key: NumericKey, value: number): void {
  values[key] = clamp(key, value);
}

export function setPreview3dIntro(intro: IntroEffect): void {
  values.intro = intro;
  // Et on le **rejoue** aussitôt : un effet d'entrée ne se voit qu'à l'entrée,
  // donc le choisir sans le déclencher revient à le régler à l'aveugle.
  resetPreview3dView();
}

export function setPreview3dQuality(quality: PreviewQuality): void {
  values.quality = quality;
}

/** Remet un groupe à ses valeurs d'origine — et lui seul : chaque bouton
 * « rétablir » est posé à côté de ce qu'il remet à zéro (voir
 * `PREVIEW3D_GROUPS`). */
export function resetPreview3dGroup(group: Preview3dGroup): void {
  for (const key of PREVIEW3D_GROUPS[group]) {
    setPreview3dValue(key, PREVIEW3D_RANGES[key].default);
  }
}
