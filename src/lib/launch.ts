// Pont typé vers le lancement de session (L4, §8).
import { invoke } from "@tauri-apps/api/core";

export type SessionType = "practice" | "hotlap" | "race";

export interface InstalledItem {
  id: string;
  name: string;
  layouts: string[];
  preview: string | null;
}

export interface RaceSetup {
  car_id: string;
  car_skin: string | null;
  track_id: string;
  track_layout: string | null;
  session_type: SessionType;
  ai_count: number;
  ai_level: number;
  laps: number;
  duration_minutes: number;
  weather: string;
  time_hours: number;
  ambient_c: number | null;
  road_c: number | null;
  penalties: boolean;
  jump_start_penalty: number;
  grip: number;
  qualifying: boolean;
  qualify_minutes: number;
  ghost_car: boolean;
  damage: number;
  fuel_rate: number;
  tyre_wear: number;
}

export interface SkinItem {
  id: string;
  name: string;
  preview: string | null;
}

export function listSkins(carId: string): Promise<SkinItem[]> {
  return invoke<SkinItem[]>("list_skins", { carId });
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

export interface ImplicitTemp {
  ambient: number;
  road: number;
}

export function weatherOptions(): Promise<WeatherOption[]> {
  return invoke<WeatherOption[]>("weather_options");
}

export function weatherTemp(intent: string, hour: number): Promise<ImplicitTemp> {
  return invoke<ImplicitTemp>("weather_temp", { intent, hour });
}

export function listInstalled(kind: "car" | "track"): Promise<InstalledItem[]> {
  return invoke<InstalledItem[]>("list_installed", { kind });
}

export function listWeather(): Promise<string[]> {
  return invoke<string[]>("list_weather");
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
