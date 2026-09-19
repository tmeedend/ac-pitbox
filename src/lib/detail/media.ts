// Pont typé vers les commandes de l'onglet Médias (§6.1) : screenshots/
// replays personnels rattachés par nom de fichier, backgrounds officiels CSP,
// et fond photo de l'écran de réglages (§6.2/SESSION§3).
import { invoke } from "@tauri-apps/api/core";
import { onAcRunning } from "$lib/launch/launch";

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
  /** Lettre écrite par AC dans le nom d'un autosave : `R` course, `Q` qualif,
   * `O` le reste. `null` dès que le fichier a été renommé. */
  session_type: string | null;
  recorded_at: string | null;
  matched_counterpart: string | null;
  size_bytes: number;
  /** Le fichier porte encore le nom donné par le jeu, donc il est dans la
   * rotation d'autosave d'AC ; le renommer est précisément ce qui le garde. */
  autosave: boolean;
  /** Rang parmi les autosaves du même type, le plus récent en 1. */
  autosave_rank: number | null;
  /** Combien AC en garde pour ce type de session (`cfg/replay.ini`). */
  autosave_limit: number | null;
  car_id: string | null;
  driver_name: string | null;
  track_id: string | null;
  track_layout: string | null;
  cars_number: number | null;
  duration_s: number | null;
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

/** Envoie un screenshot/replay à la corbeille Windows (§6.1) — récupérable
 * depuis la corbeille, d'où l'absence de confirmation. Retire aussi son
 * éventuel rattachement manuel, qui survivrait sinon au fichier. */
export function trashMediaFile(path: string): Promise<void> {
  return invoke<void>("trash_media_file", { path });
}

/** Ouvre `screens/` ou `replay/` (Documents AC) dans l'explorateur. */
export function openMediaFolder(kind: MediaKind): Promise<void> {
  return invoke<void>("open_media_folder", { kind });
}

/** Lance un replay dans Content Manager (§6.1). */
export function launchReplay(replayPath: string): Promise<void> {
  return invoke<void>("launch_replay", { replayPath });
}

/** Fond photo de l'écran de réglages (§6.2/SESSION§3) : combo exact → même
 * circuit → background officiel → `null` (fond neutre côté appelant). */
export function getSessionBackground(
  carId: string,
  trackId: string,
  layoutId: string | null,
): Promise<string | null> {
  return invoke<string | null>("get_session_background", { carId, trackId, layoutId });
}

/** Content Manager écrit sa copie du replay une poignée de secondes après la
 * fermeture du jeu, pas pendant. Un seul rechargement à la fermeture arrive
 * donc parfois trop tôt. */
const CM_SETTLE_MS = 6000;

/**
 * Rappelle `handler` quand une session vient de se terminer (§6.1) — c'est à
 * ce moment-là, et à ce moment-là seulement, que de nouveaux screenshots et
 * replays apparaissent sur le disque.
 *
 * **Le signal existait déjà** : `ac://running`, le sondage du process du jeu
 * qui coupe la musique de Big Picture et suspend les vignettes de la grille.
 * On écoute sa retombée, et non le statut « en piste » (`is_live`), qui
 * redescend à chaque retour aux stands : c'est la fermeture du jeu qui écrit
 * les fichiers.
 *
 * Deux passages, parce qu'il y a deux écrivains : Assetto Corsa pose son
 * autosave avant de rendre la main, mais Content Manager renomme ou recopie
 * le fichier ensuite, une fois le jeu parti. Sans le second passage, un replay
 * conservé par CM n'apparaissait qu'à la prochaine ouverture de la fiche.
 */
export function onSessionEnd(handler: () => void): Promise<() => void> {
  let settle: ReturnType<typeof setTimeout> | null = null;
  return onAcRunning((running) => {
    if (running) return;
    handler();
    if (settle) clearTimeout(settle);
    settle = setTimeout(handler, CM_SETTLE_MS);
  }).then((stop) => () => {
    if (settle) clearTimeout(settle);
    stop();
  });
}
