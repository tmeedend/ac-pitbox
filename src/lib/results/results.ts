// The sessions driven (RESULTAT§): the bridge to `commands/sessionlog.rs`.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SessionResult } from "$lib/launch/launch";
import type { ReplayFile, ScreenshotFile } from "$lib/detail/media";

/** What the screen shown on return needs (RESULTAT§4). */
export interface ReturnView {
  launch_id: number;
  kind: "session" | "online";
  /** practice | hotlap | race | weekend | trackday | online | other */
  mode: string;
  started_at: number;
  ended_at: number | null;
  car_id: string | null;
  track_id: string | null;
  track_layout: string | null;
  /** `null` when the game wrote nothing for this session. */
  result: SessionResult | null;
  session_id: number | null;
  best_lap_ms: number | null;
  /** The record before this session, on this car and layout. */
  previous_best_ms: number | null;
  new_best: boolean;
  can_again: boolean;
}

export interface SessionMedia {
  replay: ReplayFile | null;
  screenshots: ScreenshotFile[];
}

export function sessionReturn(launchId: number): Promise<ReturnView | null> {
  return invoke<ReturnView | null>("session_return", { launchId });
}

export function sessionMedia(launchId: number): Promise<SessionMedia> {
  return invoke<SessionMedia>("session_media", { launchId });
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
