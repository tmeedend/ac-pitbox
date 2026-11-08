// Pont typé vers le backend Rust pour le module musique (SPEC §16,
// docs/spec-module-musique_2.md).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { t } from "$lib/i18n/index.svelte";
import { emptyAssignments, type Assignments, type Resolved } from "./resolve";

/** Where music plays (§16.1). */
export type PlayMode = "off" | "big_picture" | "always";

export interface MusicConfig {
  version: number;
  play_mode: PlayMode;
  /** Décochée (défaut) : pack embarqué, sources et attributions ignorées. */
  use_custom_folders: boolean;
  /** A folder or a file; `null` = the default folder (`getDefaultMusicFolders`). */
  menu_folder: string | null;
  grid_folder: string | null;
  shuffle: boolean;
  /** 0.0–1.0 */
  music_volume: number;
  crossfade_ms: number;
  fade_out_ms: number;
  fade_in_ms: number;
  /** Now-playing notification (MUSIQUE§5.5), off by default. */
  show_now_playing: boolean;
  assignments: Assignments;
  /** Sources picked last, most recent first (§16.8, "Recent"). */
  recent_sources: string[];
}

export function emptyMusicConfig(): MusicConfig {
  return {
    version: 2,
    play_mode: "big_picture",
    use_custom_folders: false,
    menu_folder: null,
    grid_folder: null,
    shuffle: true,
    music_volume: 0.45,
    crossfade_ms: 2500,
    fade_out_ms: 1500,
    fade_in_ms: 2000,
    show_now_playing: false,
    assignments: emptyAssignments(),
    recent_sources: [],
  };
}

export interface DefaultMusicFolders {
  menu: string;
  grid: string;
}

/** What a source holds (§16.3). */
export interface SourceInfo {
  track_count: number;
  is_file: boolean;
  missing: boolean;
}

/** A Steam soundtrack folder (§16.7). */
export interface Soundtrack {
  name: string;
  path: string;
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

export function scanMusicSource(path: string): Promise<SourceInfo> {
  return invoke<SourceInfo>("scan_music_source", { path });
}

export function listSteamSoundtracks(): Promise<Soundtrack[]> {
  return invoke<Soundtrack[]>("list_steam_soundtracks");
}

export function musicEnterBigPicture(): Promise<void> {
  return invoke<void>("music_enter_big_picture");
}

export function musicExitBigPicture(): Promise<void> {
  return invoke<void>("music_exit_big_picture");
}

/** What the screen on display resolved (§16.5). */
export function musicSetContext(context: Resolved): Promise<void> {
  return invoke<void>("music_set_context", { context });
}

export function musicSetMinimized(minimized: boolean): Promise<void> {
  return invoke<void>("music_set_minimized", { minimized });
}

export function musicPreviewStart(path: string, volume: number): Promise<void> {
  return invoke<void>("music_preview_start", { path, volume });
}

export function musicPreviewStop(): Promise<void> {
  return invoke<void>("music_preview_stop");
}

/** A track that just started playing (MUSIQUE§5.5), as read from its tags by
 * `music::tags` — `title` is `null` on an untagged file. */
export interface TrackInfo {
  file_name: string;
  title: string | null;
  artist: string | null;
  album: string | null;
  /** JPEG `data:` URL, already thumbnailed on the Rust side. */
  cover: string | null;
}

/** What the notification shows. */
export interface NowPlaying {
  title: string;
  artist: string | null;
  album: string | null;
  cover: string | null;
}

export function onMusicTrack(cb: (track: TrackInfo) => void): Promise<UnlistenFn> {
  return listen<TrackInfo>("music:track", (e) => cb(e.payload));
}

/** The two tracks of the built-in pack (`music/config.rs`) carry no tags:
 * their title and author are the credits of the About screen, and the file
 * name — all an untagged track would otherwise show — says "menu-ambience". */
const BUNDLED: Record<string, { title: string; author: string }> = {
  "menu-ambience.mp3": { title: "about.musicMenuTitle", author: "about.musicMenuAuthor" },
  "grid-ambience.mp3": { title: "about.musicGridTitle", author: "about.musicGridAuthor" },
};

export function nowPlaying(track: TrackInfo): NowPlaying {
  const bundled = track.title ? undefined : BUNDLED[track.file_name.toLowerCase()];
  if (bundled) return { title: t(bundled.title), artist: t(bundled.author), album: null, cover: track.cover };
  return {
    // Untagged: the file name without its extension is still what the user
    // named it, and more telling than nothing.
    title: track.title ?? track.file_name.replace(/\.[^.]+$/, ""),
    artist: track.artist,
    album: track.album,
    cover: track.cover,
  };
}

/** The last segment of a source path: what a picker shows of it. */
export function sourceName(path: string): string {
  return path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path;
}
