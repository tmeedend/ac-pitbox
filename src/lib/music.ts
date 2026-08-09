// Pont typé vers le backend Rust pour le module musique du mode Big Picture
// (docs/spec-module-musique_2.md).
import { invoke } from "@tauri-apps/api/core";

export interface MusicConfig {
  version: number;
  enabled: boolean;
  /** `null` = dossier par défaut (voir `getDefaultMusicFolders`). */
  menu_folder: string | null;
  grid_folder: string | null;
  shuffle: boolean;
  /** 0.0–1.0 */
  volume: number;
  crossfade_ms: number;
  fade_out_ms: number;
  fade_in_ms: number;
  session_behavior: "stop" | "duck";
  /** 0.0–1.0, volume absolu (pas un facteur du volume principal). */
  session_duck_volume: number;
}

export function emptyMusicConfig(): MusicConfig {
  return {
    version: 1,
    enabled: true,
    menu_folder: null,
    grid_folder: null,
    shuffle: true,
    volume: 0.45,
    crossfade_ms: 2500,
    fade_out_ms: 1500,
    fade_in_ms: 2000,
    session_behavior: "stop",
    session_duck_volume: 0.12,
  };
}

export interface DefaultMusicFolders {
  menu: string;
  grid: string;
}

export interface FolderInfo {
  track_count: number;
}

export function getMusicConfig(): Promise<MusicConfig> {
  return invoke<MusicConfig>("get_music_config");
}

export function saveMusicConfig(config: MusicConfig): Promise<void> {
  return invoke<void>("save_music_config", { config });
}

export function getDefaultMusicFolders(): Promise<DefaultMusicFolders> {
  return invoke<DefaultMusicFolders>("get_default_music_folders");
}

export function scanMusicFolder(path: string): Promise<FolderInfo> {
  return invoke<FolderInfo>("scan_music_folder", { path });
}

export function musicEnterBigPicture(): Promise<void> {
  return invoke<void>("music_enter_big_picture");
}

export function musicExitBigPicture(): Promise<void> {
  return invoke<void>("music_exit_big_picture");
}

export function musicEnterMenu(): Promise<void> {
  return invoke<void>("music_enter_menu");
}

export function musicEnterGrid(): Promise<void> {
  return invoke<void>("music_enter_grid");
}

export function musicPreviewStart(path: string, volume: number): Promise<void> {
  return invoke<void>("music_preview_start", { path, volume });
}

export function musicPreviewStop(): Promise<void> {
  return invoke<void>("music_preview_stop");
}
