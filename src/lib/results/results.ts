// The sessions driven (RESULTAT§): the bridge to `commands/sessionlog.rs`.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SessionResult } from "$lib/launch/launch";
import type { ReplayFile, ScreenshotFile } from "$lib/detail/media";
import type { SavedSession } from "$lib/launch/savedSessions";

/** What every screen says of a session (RESULTAT§7). */
export interface SessionHead {
  launch_id: number | null;
  session_id: number | null;
  /** `session` | `online` (Pit Box's) | `cm` (started elsewhere). */
  kind: "session" | "online" | "cm";
  /** practice | hotlap | race | weekend | trackday | online | other */
  mode: string;
  started_at: number;
  ended_at: number | null;
  car_id: string | null;
  car_name: string | null;
  car_known: boolean;
  track_id: string | null;
  track_name: string | null;
  track_known: boolean;
  track_layout: string | null;
  best_lap_ms: number | null;
  can_again: boolean;
  can_edit: boolean;
  server: { ip: string; http_port: number } | null;
}

/** What a session was driven in, when known (RESULTAT§7.4). */
export interface Conditions {
  weather: string | null;
  time_hours: number | null;
  ambient_c: number | null;
  road_c: number | null;
  track_state: string | null;
}

/** The screen of a session (RESULTAT§4). */
export interface ReturnView extends SessionHead {
  /** `null` when the game wrote nothing for this session. */
  result: SessionResult | null;
  /** The record before this session, on this car and layout. */
  previous_best_ms: number | null;
  new_best: boolean;
  conditions: Conditions | null;
}

/** A line of the history (RESULTAT§7). */
export interface HistoryLine extends SessionHead {
  valid_laps: number;
  race_position: number | null;
}

export interface SessionMedia {
  replay: ReplayFile | null;
  screenshots: ScreenshotFile[];
}

/** One car on one layout: the history's only filter (RESULTAT§7.5). */
export interface Combo {
  car_id: string;
  track_id: string;
  track_layout: string | null;
}

export function sessionReturn(launchId: number): Promise<ReturnView | null> {
  return invoke<ReturnView | null>("session_return", { launchId });
}

export function sessionView(sessionId: number): Promise<ReturnView | null> {
  return invoke<ReturnView | null>("session_view", { sessionId });
}

export function sessionHistory(combo: Combo | null): Promise<HistoryLine[]> {
  return invoke<HistoryLine[]>("session_history", {
    carId: combo?.car_id ?? null,
    trackId: combo?.track_id ?? null,
    trackLayout: combo?.track_layout ?? null,
  });
}

export function sessionMedia(carId: string, trackId: string, fromMs: number, toMs: number): Promise<SessionMedia> {
  return invoke<SessionMedia>("session_media", { carId, trackId, fromMs, toMs });
}

export function outingPicture(carIds: string[], trackId: string, fromMs: number, toMs: number): Promise<string | null> {
  return invoke<string | null>("outing_picture", { carIds, trackId, fromMs, toMs });
}

/** What Edit loads: a saved session, and what could not be carried. */
export function sessionEdit(sessionId: number): Promise<{ session: SavedSession; notes: string[] } | null> {
  return invoke<{ session: SavedSession; notes: string[] } | null>("session_edit", { sessionId });
}

export function personalBest(combo: Combo): Promise<{ best_lap_ms: number; at_ms: number } | null> {
  return invoke<{ best_lap_ms: number; at_ms: number } | null>("personal_best", {
    carId: combo.car_id,
    trackId: combo.track_id,
    trackLayout: combo.track_layout,
  });
}

/** "My times" (RESULTAT§10): one line per car and layout. */
export interface ComboTimes {
  car_id: string;
  car_name: string | null;
  track_id: string;
  track_name: string | null;
  track_layout: string | null;
  best_lap_ms: number | null;
  best_at: number;
  last_at: number;
  sessions: number;
}

export function myTimes(modId: string, isCar: boolean): Promise<ComboTimes[]> {
  return invoke<ComboTimes[]>("my_times", { modId, isCar });
}

/** The result of the session a replay was recorded in (RESULTAT§11). */
export interface ReplayResult {
  session_id: number;
  race_position: number | null;
  best_lap_ms: number | null;
}

export function replayResults(
  replays: { path: string; car_id: string; track_id: string; at_ms: number }[],
): Promise<Record<string, ReplayResult>> {
  return invoke<Record<string, ReplayResult>>("replay_results", { replays });
}
/** The same session, started again. */
export function launchAgain(launchId: number): Promise<void> {
  return invoke<void>("launch_again", { launchId });
}

/** A session of the user's has ended and the game has gone. */
export function onSessionEnded(handler: (launchId: number) => void): Promise<() => void> {
  return listen<{ launch_id: number }>("session://ended", (e) => handler(e.payload.launch_id));
}

/** One of CM's copies that looks like a thumbnail session (RESULTAT§5.2). */
export interface ThumbnailCopy {
  file: string;
  at_ms: number;
  cars: string[];
}

export function listThumbnailCopies(): Promise<ThumbnailCopy[]> {
  return invoke<ThumbnailCopy[]>("list_thumbnail_copies");
}

/** To the recycle bin; returns the files that went. */
export function trashThumbnailCopies(files: string[]): Promise<string[]> {
  return invoke<string[]>("trash_thumbnail_copies", { files });
}
