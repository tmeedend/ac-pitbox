// Pont typé vers le lancement de session (L4, §8).
import { invoke } from "@tauri-apps/api/core";

export type SessionType = "practice" | "hotlap" | "race" | "trackday";

/** Départ en Practice (§8.4) : "pit"/"track"/"hotlap" → `StartType` du preset
 * Quick Drive ("PIT"/"TRACK"/"HOTLAP_START", voir `PracticeStart` côté Rust). */
export type PracticeStart = "pit" | "track" | "hotlap";

/** Saison optionnelle associée à une session (§8.6bis) — influence la
 * température recommandée et, best-effort côté CSP, le rendu (arbres,
 * neige). "" = aucune saison choisie. */
export type Season = "" | "spring" | "summer" | "autumn" | "winter";

/** Type de plateau d'adversaires (§8.6) : détermine le vivier où piocher.
 * "same_era" retiré au profit de la fourchette d'année (year_min/year_max),
 * disponible pour same_category/free. */
export type GridMode = "same_car" | "same_category" | "free";

/** Valeur spéciale de la catégorie du vivier « Par catégorie » (§8.6) : suit
 * automatiquement la catégorie de la voiture pilotée, plutôt qu'une catégorie
 * fixée à la main qui doit survivre à un changement de voiture. Vit ici (pas
 * dans un composant) parce que `savedSessions.ts` en a besoin comme valeur par
 * défaut pour les sauvegardes antérieures à ce champ. */
export const SAME_CATEGORY = "__same_category__";

export interface Opponent {
  car_id: string;
  ai_level: number;
  /** Skin de l'adversaire, choisi (auto ou via la popup de sélection). */
  car_skin: string | null;
}

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
  /** Fourchette de niveau IA (§8.6) : le plateau est réparti dedans. */
  ai_level_min: number;
  ai_level_max: number;
  laps: number;
  weather: string;
  time_hours: number;
  ambient_c: number | null;
  road_c: number | null;
  wind_speed_kmh: number | null;
  wind_direction_deg: number | null;
  /** Fourchette d'année du vivier d'adversaires (remplace « même ère »), comme
   * ai_level_min/max : toujours une valeur concrète (pas de « non réglé »). */
  year_min: number;
  year_max: number;
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
  abs_auto: boolean;
  traction_control_auto: boolean;
  ideal_line: boolean;
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

export function launchSession(setup: RaceSetup): Promise<void> {
  return invoke<void>("launch_session", { setup });
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
