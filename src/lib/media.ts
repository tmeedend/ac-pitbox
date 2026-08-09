// Pont typé vers les commandes de l'onglet Médias (§6.1) : screenshots/
// replays personnels rattachés par nom de fichier, backgrounds officiels CSP,
// et fond photo de l'écran de réglages (§6.2/§9.3).
import { invoke } from "@tauri-apps/api/core";

export interface ScreenshotFile {
  path: string;
  file_name: string;
  modified_at: string | null;
  /** Id de l'autre entité (circuit pour une voiture, et inversement) trouvé
   * dans le nom de fichier, s'il y en a un. */
  matched_counterpart: string | null;
}

export interface ReplayFile {
  path: string;
  file_name: string;
  session_type: string | null;
  recorded_at: string | null;
  matched_counterpart: string | null;
}

export interface BackgroundFile {
  path: string;
  layout_id: string | null;
}

export type MediaKind = "SCREENSHOT" | "REPLAY";

/** Captures personnelles mettant en scène cette voiture/ce circuit (§6.1) —
 * rattachement automatique par nom de fichier, fusionné avec les
 * rattachements manuels de repli. */
export function listMediaScreenshots(id: string): Promise<ScreenshotFile[]> {
  return invoke<ScreenshotFile[]>("list_media_screenshots", { id });
}

/** Replays impliquant cette voiture/ce circuit (§6.1). */
export function listMediaReplays(id: string): Promise<ReplayFile[]> {
  return invoke<ReplayFile[]>("list_media_replays", { id });
}

/** Backgrounds officiels CSP pour un circuit (§6.1) — `layoutId` filtre sur
 * le layout sélectionné sur la fiche, `null` renvoie tout le circuit. */
export function listMediaBackgrounds(id: string, layoutId: string | null): Promise<BackgroundFile[]> {
  return invoke<BackgroundFile[]>("list_media_backgrounds", { id, layoutId });
}

/** Rattache manuellement un fichier (repli, §6.1) quand le matching
 * automatique par nom n'a pas trouvé l'entité. */
export function linkMediaManually(id: string, kind: MediaKind, filePath: string): Promise<void> {
  return invoke<void>("link_media_manually", { id, kind, filePath });
}

/** Ouvre `screens/` ou `replay/` (Documents AC) dans l'explorateur. */
export function openMediaFolder(kind: MediaKind): Promise<void> {
  return invoke<void>("open_media_folder", { kind });
}

/** Fond photo de l'écran de réglages (§6.2/§9.3) : combo exact → même
 * circuit → background officiel → `null` (fond neutre côté appelant). */
export function getSessionBackground(
  carId: string,
  trackId: string,
  layoutId: string | null,
): Promise<string | null> {
  return invoke<string | null>("get_session_background", { carId, trackId, layoutId });
}
