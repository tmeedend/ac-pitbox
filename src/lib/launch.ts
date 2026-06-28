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
