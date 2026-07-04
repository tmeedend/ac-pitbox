// Pont typé vers le lancement de session (L4, §8).
import { invoke } from "@tauri-apps/api/core";

export type SessionType = "practice" | "hotlap" | "race";

/** Type de plateau d'adversaires (§8.6) : détermine le vivier où piocher.
 * "same_era" retiré au profit de la fourchette d'année (year_min/year_max),
 * disponible pour same_category/free. */
export type GridMode = "same_car" | "same_category" | "free";

export interface Opponent {
  car_id: string;
  ai_level: number;
  /** Skin de l'adversaire, choisi (auto ou via la popup de sélection). */
  car_skin: string | null;
}

export interface RaceSetup {
  car_id: string;
  car_skin: string | null;
  track_id: string;
  track_layout: string | null;
  session_type: SessionType;
  /** Plateau d'adversaires (mode course uniquement), chacun avec son niveau IA. */
  opponents: Opponent[];
  /** Fourchette de niveau IA (§8.6) : le plateau est réparti dedans. */
  ai_level_min: number;
  ai_level_max: number;
  laps: number;
  duration_minutes: number;
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
  /** Date ISO (YYYY-MM-DD) associée à la saison choisie ; best-effort côté race.ini. */
  season_date: string | null;
  penalties: boolean;
  jump_start_penalty: number;
  grip: number;
  qualifying: boolean;
  qualify_minutes: number;
  ghost_car: boolean;
  damage: number;
  fuel_rate: number;
  tyre_wear: number;
  abs_auto: boolean;
  traction_control_auto: boolean;
  ideal_line: boolean;
}

export interface SkinItem {
  id: string;
  name: string;
  preview: string | null;
}

/** Skins de la version active d'un mod, lus dans la bibliothèque (fiche détail §6.3). */
export function listModSkins(id: string): Promise<SkinItem[]> {
  return invoke<SkinItem[]>("list_mod_skins", { id });
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

export function weatherConditions(intent: string, hour: number): Promise<ImplicitConditions> {
  return invoke<ImplicitConditions>("weather_conditions", { intent, hour });
}

export function launchSession(setup: RaceSetup): Promise<void> {
  return invoke<void>("launch_session", { setup });
}

/** Ouvre Content Manager sans argument (§12bis.5). */
export function openContentManager(): Promise<void> {
  return invoke<void>("open_content_manager");
}

/** Lance l'aperçu 3D natif (acShowroom.exe) ciblé sur une voiture (+ skin optionnel). */
export function openNativeShowroom(carId: string, skinId?: string | null): Promise<void> {
  return invoke<void>("open_native_showroom", { carId, skinId: skinId ?? null });
}

/** Intègre la fenêtre du showroom lancé dans la zone (x, y, width, height) — pixels physiques. */
export function attachNativeShowroom(x: number, y: number, width: number, height: number): Promise<void> {
  return invoke<void>("attach_native_showroom", { x, y, width, height });
}

/** Repositionne/redimensionne le showroom déjà intégré. */
export function repositionNativeShowroom(x: number, y: number, width: number, height: number): Promise<void> {
  return invoke<void>("reposition_native_showroom", { x, y, width, height });
}

/** Ferme proprement le showroom en cours (attaché ou flottant). */
export function closeNativeShowroom(): Promise<void> {
  return invoke<void>("close_native_showroom");
}
