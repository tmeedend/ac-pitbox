// Pont typé vers le lancement de session (L4, §8).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { PAUSE_SESSION, pauseGridThumbs } from "./gridThumbs.svelte";

export type SessionType = "practice" | "hotlap" | "race" | "trackday";

/** Départ en Practice (§8.4) : "pit"/"track"/"hotlap" → `StartType` du preset
 * Quick Drive ("PIT"/"TRACK"/"HOTLAP_START", voir `PracticeStart` côté Rust). */
export type PracticeStart = "pit" | "track" | "hotlap";

/** Niveau d'une aide au pilotage — les trois états d'Assetto Corsa lui-même
 * (`0,1,2` en face de `Off,Factory,On` dans le launcher du jeu, voir
 * `AssistLevel` côté Rust). `factory` garde l'équipement réel de la voiture,
 * et c'est pourquoi `on` ne peut rien ajouter à une voiture qui n'a pas
 * l'aide. */
export type AssistLevel = "off" | "factory" | "on";

/** Les six états de piste du jeu, par grip de départ croissant — la liste que
 * Content Manager propose, et qui vient du même fichier (`tracks.ini`). Le
 * pourcentage de départ **est** l'identifiant de l'état : c'est le seul des
 * quatre paramètres que l'écran retient, `quickdrive.rs` retrouve les trois
 * autres à partir de lui. */
export const TRACK_GRIPS = [86, 89, 95, 96, 98, 100] as const;

/** « Auto » : l'état de piste est laissé à la météo — le `WeatherDefined` de
 * Content Manager, et la première entrée de sa liste. Sentinelle plutôt qu'un
 * second champ : l'écran n'offre qu'un choix parmi sept, et deux champs pour
 * une seule décision finissent toujours par se contredire. Aucun état réel ne
 * vaut 0 %. */
export const GRIP_WEATHER = 0;

/** Recale un grip enregistré sur la liste offerte.
 *
 * Un preset antérieur à l'alignement sur la table du jeu peut porter une
 * valeur qui n'y figure plus (92 % a existé, inventé) : sans ce recalage, le
 * segmenté n'en marquerait aucun comme actif. À égale distance le plus
 * adhérent gagne, comme côté Rust — une piste un peu plus roulante est le
 * repli indulgent. */
export function nearestGrip(grip: number): number {
  // « Auto » n'est pas un grip : le recalage ne doit pas le prendre pour une
  // valeur basse et le remplacer par la piste la plus glissante.
  if (grip === GRIP_WEATHER) return grip;
  let best: number = TRACK_GRIPS[0];
  for (const g of TRACK_GRIPS) {
    const d = Math.abs(g - grip);
    const bd = Math.abs(best - grip);
    if (d < bd || (d === bd && g > best)) best = g;
  }
  return best;
}

/**
 * Bornes de la force d'une IA, **celles du curseur de Content Manager**.
 *
 * 70 et non 60 : CM ne descend pas plus bas, et une valeur hors de sa plage
 * part dans un preset qu'il recalera lui-même — le réglage ne serait donc pas
 * celui qu'on affiche. Relevé sur son écran, et confirmé sur un preset de
 * grille réel dont la ligne la plus faible vaut exactement 70.
 *
 * Ici et pas dans un composant : l'écran dessine la fourchette, la ligne du
 * plateau borne sa propre valeur, et `savedSessions`/les presets recalent ce
 * qu'ils relisent. Trois copies d'un même nombre finissent par diverger.
 */
export const AI_LEVEL_MIN = 70;
export const AI_LEVEL_MAX = 100;

/** Recale une force dans la plage de CM. Une valeur enregistrée avant que le
 * plancher ne soit corrigé (60 était offert) remonte donc à 70 au lieu de
 * partir telle quelle. */
export function clampAiLevel(level: number): number {
  if (!Number.isFinite(level)) return AI_LEVEL_MAX;
  return Math.max(AI_LEVEL_MIN, Math.min(AI_LEVEL_MAX, Math.round(level)));
}

/** Saison optionnelle associée à une session (§8.6bis) — influence la
 * température recommandée et, best-effort côté CSP, le rendu (arbres,
 * neige). "" = aucune saison choisie. */
export type Season = "" | "spring" | "summer" | "autumn" | "winter";

/** Ce que Content Manager écrit dans un tableau numérique par ligne pour dire
 * « laissé au jeu » (§4.1). Relevé sur un preset de grille réel, où il voisine
 * un `"0"` explicite : les deux ne veulent pas dire la même chose. */
export const AUTO_CELL = null;

export interface Opponent {
  car_id: string;
  /** Force de l'IA, ou `null` pour **`Auto`** : la ligne n'a pas de surcharge
   * et le jeu tire dans la fourchette globale. Ce n'est donc pas une valeur
   * qu'on tire nous-mêmes — c'est son absence, et le format la connaît déjà. */
  ai_level: number | null;
  /** Skin de l'adversaire, choisi (auto ou via la popup de sélection). */
  car_skin: string | null;
  /** Nom de pilote et nationalité repris à la main, ou `null` pour `Auto` — le
   * jeu prend alors ce que le `ui_skin.json` de la livrée déclare. La
   * nationalité est un nom de pays anglais entier, jamais un code ISO. */
  driver_name: string | null;
  nationality: string | null;
  /** Lest et bride d'équilibrage, 0 à 100. Pas d'`Auto` : « aucun lest » se
   * dit par 0, et c'est ce que CM écrit. */
  ballast: number;
  restrictor: number;
}

/** Un adversaire neuf : une voiture, une livrée, et rien d'autre. Tout le reste
 * vaut `Auto`, ce qui est le bon défaut — poser une valeur est un geste. */
export function newOpponent(carId: string, skinId: string | null): Opponent {
  return {
    car_id: carId,
    ai_level: AUTO_CELL,
    car_skin: skinId,
    driver_name: AUTO_CELL,
    nationality: AUTO_CELL,
    ballast: 0,
    restrictor: 0,
  };
}

/** Position de départ du joueur (§4.4), **course uniquement**. Les trois
 * premières se résolvent côté Rust, où la taille du plateau est connue pour de
 * bon. `last` est le défaut, celui de Content Manager. */
export type StartMode = "last" | "first" | "random" | "custom";

/** Agressivité de l'IA (§4.4). Défaut **0**, celui de CM — à ne pas
 * « améliorer » : c'est la valeur avec laquelle des milliers d'heures de course
 * ont été réglées. Pas de 5, comme la tolérance de performance. */
export const AGGRESSION_MIN = 0;
export const AGGRESSION_MAX = 100;
export const AGGRESSION_STEP = 5;

/** Les quatre pièces telles que le backend les nomme — `model` est le corps,
 * comme `driver3d.ini` l'appelle. */
export interface DriverChoice {
  model: string | null;
  suit: string | null;
  gloves: string | null;
  helmet: string | null;
}

export interface RaceSetup {
  car_id: string;
  car_skin: string | null;
  /** Le pilote choisi pour cette voiture, posé dans son dossier juste avant le
   * lancement (`driverapply` côté Rust). `null` = cette voiture n'a rien de
   * particulier, et ce qui avait été posé pour elle est retiré. */
  driver: DriverChoice | null;
  track_id: string;
  track_layout: string | null;
  session_type: SessionType;
  /** Plateau d'adversaires (mode course uniquement), chacun avec son niveau IA. */
  opponents: Opponent[];
  /** Fourchette de force de l'IA (§8.6) : le jeu y répartit les lignes `Auto`.
   * Elle part maintenant vraiment dans le preset — `AiLevel`/`AiLevelMin` y
   * étaient codés en dur sur 95/85. */
  ai_level_min: number;
  ai_level_max: number;
  /** Agressivité de l'IA (§4.4), 0 à 100. */
  aggression: number;
  /** Position de départ du joueur (§4.4), course uniquement. */
  start_mode: StartMode;
  /** Rang saisi, lu seulement quand `start_mode` vaut `"custom"`. */
  start_position: number;
  laps: number;
  weather: string;
  time_hours: number;
  ambient_c: number | null;
  road_c: number | null;
  wind_speed_kmh: number | null;
  wind_direction_deg: number | null;
  /** Saison optionnelle (§8.6bis) — voir season_date pour la valeur réellement écrite. */
  season: string | null;
  /** Date ISO (YYYY-MM-DD) associée à la saison choisie ; best-effort côté preset Quick Drive (udt/dtv). */
  season_date: string | null;
  penalties: boolean;
  jump_start_penalty: number;
  grip: number;
  /** Essais libres avant la course (weekend Quick Drive) — indépendants de la qualification. */
  practice_enabled: boolean;
  practice_minutes: number;
  /** Qualification avant la course (§9.3). Décochée, le preset bascule sur le
   * mode course sèche de CM : son mode Weekend n'a pas d'état « pas de
   * qualif ». Les essais libres n'existant que dans Weekend, ils la suivent. */
  qualify_enabled: boolean;
  /** Durée qualif quand elle est demandée (mini 5 min, borne de CM). */
  qualify_minutes: number;
  ghost_car: boolean;
  /** Départ en Practice (mode Practice uniquement). */
  practice_start: PracticeStart;
  damage: number;
  fuel_rate: number;
  tyre_wear: number;
  tyre_blankets: boolean;
  abs: AssistLevel;
  traction_control: AssistLevel;
  ideal_line: boolean;
}

/** Reprise d'un réglage d'aide enregistré avant les trois états (§9.3).
 *
 * L'ancien champ était un booléen nommé `abs_auto`, et « auto » voulait dire
 * `Abs: 1` dans le preset Quick Drive, c'est-à-dire **exactement** le niveau
 * qui s'appelle aujourd'hui `factory`. Donc `true → factory`, pas `true → on` :
 * `on` vaut `2` et forcerait une aide là où l'utilisateur n'avait demandé que
 * l'équipement réel de la voiture. Une migration ne change pas ce qui part en
 * jeu — c'est la même raison qui fait que `false → off` plutôt que le nouveau
 * défaut, l'un et l'autre conservant à la lettre la valeur déjà envoyée.
 *
 * Absent (`undefined`, distinct de `false`) : sauvegarde plus ancienne encore,
 * ou preset neuf — `factory`, le défaut. */
export function assistLevelFrom(level: AssistLevel | undefined, legacy: boolean | undefined): AssistLevel {
  if (level) return level;
  if (legacy === undefined) return "factory";
  return legacy ? "factory" : "off";
}

/** Ce que la voiture avait d'usine — lu dans son `electronics.ini` (§9.3).
 * `null` quand elle ne le dit pas : l'écran n'affiche alors rien. */
export interface FactoryAssists {
  abs: boolean;
  tractionControl: boolean;
}

/** Aides d'usine de la voiture en session, pour dire ce que « Factory » vaut
 * **pour elle**. Jamais une erreur : une voiture muette rend `null`. */
export function carFactoryAssists(carId: string): Promise<FactoryAssists | null> {
  return invoke<FactoryAssists | null>("car_factory_assists", { carId });
}

export interface SkinItem {
  id: string;
  name: string;
  preview: string | null;
  /** `livery.png` (couleurs/motif du skin seul, sans la voiture) — `null` si absent. */
  livery: string | null;
}

/** Skins de la version active d'un mod, lus dans la bibliothèque (fiche détail §6.3). */
export function listModSkins(id: string): Promise<SkinItem[]> {
  return invoke<SkinItem[]>("list_mod_skins", { id });
}

/** Fonctionnalités CSP effectivement détectées pour un mod (§6.4bis) : config
 * propre au mod + config CSP "chargée" séparément par CSP (hors du mod — ce
 * qui manquait pour le contenu de base). Valeurs possibles : "grassfx",
 * "rainfx", "lightingfx", "season". Sert à griser les réglages non supportés
 * sur l'écran de session (saison, avertissement pluie). */
export function getModCspFeatures(id: string): Promise<string[]> {
  return invoke<string[]>("get_mod_csp_features", { id });
}

export interface WeatherStack {
  csp: boolean;
  sol: boolean;
  vanilla: boolean;
}

export interface WeatherOption {
  id: string;
  label: string;
  available: boolean;
  weather: string | null;
  backend: string | null;
  reason: string | null;
  wet: boolean;
}

/** Température + vent implicites (§8.5/§8.6) — jamais saisis manuellement. */
export interface ImplicitConditions {
  ambient: number;
  road: number;
  wind_speed_kmh: number;
  wind_direction_deg: number;
}

export function weatherOptions(): Promise<WeatherOption[]> {
  return invoke<WeatherOption[]>("weather_options");
}

export function weatherConditions(intent: string, hour: number, season: string | null): Promise<ImplicitConditions> {
  return invoke<ImplicitConditions>("weather_conditions", { intent, hour, season });
}

/** Course du soleil sur le circuit choisi (§8.6ter), telle que CSP la
 * calculera : coordonnées et fuseau de `data_track_params.ini`, date effective
 * décidée par `[SEASONS] ALLOW_ADJUSTMENTS`. Toutes les heures sont en heures
 * décimales sur l'horloge locale du circuit ; `null` quand le soleil ne passe
 * pas l'horizon ce jour-là (nuit polaire ou soleil de minuit). */
export interface TrackSun {
  latitude: number;
  longitude: number;
  timezone: string | null;
  utcOffsetHours: number;
  /** "csp" = data_track_params.ini (ce que le jeu lira), "geotags" = position
   * déclarée par le mod, fuseau approché d'après la longitude. */
  source: "csp" | "geotags";
  seasonalSetting: number;
  dateBasis: "session" | "today" | "midsummer";
  date: string;
  sunrise: number | null;
  sunset: number | null;
  dawn: number | null;
  dusk: number | null;
  solarNoon: number;
  polarNight: boolean;
}

/** `null` quand le circuit n'a de position nulle part : pas de bande jour/nuit
 * plutôt qu'une bande fausse. */
export function trackSun(
  trackId: string,
  layout: string | null,
  seasonDate: string | null,
): Promise<TrackSun | null> {
  return invoke<TrackSun | null>("track_sun", { trackId, layout, seasonDate });
}

/**
 * Lance la session, et **suspend la génération des vignettes** dès le clic.
 *
 * Ici plutôt que dans l'écran qui appelle : c'est le lancement lui-même qui
 * doit rendre la machine, quel que soit le bouton qui l'a déclenché.
 *
 * **Dès le clic, et non à l'apparition du process** — c'est la différence avec
 * la musique, qui, elle, continue de jouer pendant tout l'écran de chargement
 * et ne se coupe qu'une fois la voiture pilotable (`music/watch.rs`). Les deux
 * n'ont pas le même besoin : la musique accompagne l'attente, alors que trois
 * cents conversions pendant qu'Assetto Corsa charge sont exactement ce qui
 * rallonge cette attente.
 *
 * La reprise, elle, est bien celle de la musique : voir `onAcRunning`.
 */
export function launchSession(setup: RaceSetup): Promise<void> {
  pauseGridThumbs(PAUSE_SESSION);
  return invoke<void>("launch_session", { setup });
}

/**
 * S'abonne à la présence du process d'Assetto Corsa. Renvoie la fonction de
 * désabonnement.
 *
 * **Le fil qui l'annonce existait déjà** : c'est celui qui coupe et reprend la
 * musique de Big Picture (`music/watch.rs`, sondage de `acs.exe` toutes les
 * 500 ms). Le redécouvrir aurait été un second sondage pour la même question.
 */
export function onAcRunning(handler: (running: boolean) => void): Promise<() => void> {
  return listen<boolean>("ac://running", (event) => handler(event.payload));
}

/** Ouvre Content Manager sans argument (§12bis.5). */
export function openContentManager(): Promise<void> {
  return invoke<void>("open_content_manager");
}

/** Steam tourne-t-il ? (§9.2bis) Assetto Corsa est un jeu Steam : sans Steam,
 * le lancement échoue côté Content Manager, après que Pit Box a rendu la main
 * — donc sans erreur qu'on puisse afficher. D'où ce contrôle avant lancement. */
export function isSteamRunning(): Promise<boolean> {
  return invoke<boolean>("is_steam_running");
}

/** Lance l'aperçu 3D natif (acShowroom.exe) ciblé sur une voiture (+ skin
 * optionnel). Process indépendant, affiché par-dessus l'app : c'est
 * l'utilisateur qui ferme le showroom pour revenir à Pit Box. */
export function openNativeShowroom(carId: string, skinId?: string | null): Promise<void> {
  return invoke<void>("open_native_showroom", { carId, skinId: skinId ?? null });
}

/** Une scène de showroom installée dans AC (`content/showroom/<id>`). */
export interface ShowroomOption {
  id: string;
  name: string;
}

/** Showrooms installés, pour le choix de scène des réglages. */
export function listShowrooms(): Promise<ShowroomOption[]> {
  return invoke<ShowroomOption[]>("list_showrooms");
}
