// The pictures of the tracks the library does not hold (SPEC-play-online.md,
// "Vignette d'un circuit non installé"; `online/picture.rs`). Asked one at a
// time, the open server's first, then the rows in view - a page of thirty
// tracks must not send thirty matchings at Wikipedia at once. Each track is
// asked once per session; the backend's cache keeps the answers between two.
import { invoke } from "@tauri-apps/api/core";

export interface TrackPicture {
  /** The thumbnail in the cache, an absolute path (`previewSrc`). */
  file: string | null;
  artist: string | null;
  licence: string | null;
  /** Its page on Commons. */
  page: string | null;
  /** ISO 3166-1 alpha-2 of the track's country. */
  country: string | null;
}

/** Lowercase track id → its picture; `null` when asked and nothing came
 * (no entity, no network): not asked again this session. */
const pictures = $state<{ byId: Record<string, TrackPicture | null> }>({ byId: {} });

const queue: string[] = [];
let running = false;

/** Reactive: the picture of a track, `undefined` until asked and answered. */
export function pictureOf(trackId: string): TrackPicture | null | undefined {
  return pictures.byId[trackId.toLowerCase()];
}

/** Asks for these tracks' pictures; `first` puts them ahead of the queue
 * (the open server's track). */
export function wantPictures(trackIds: string[], first = false): void {
  const fresh = trackIds
    .map((id) => id.toLowerCase())
    .filter((id, i, all) => all.indexOf(id) === i && !(id in pictures.byId) && !queue.includes(id));
  if (first) queue.unshift(...fresh);
  else queue.push(...fresh);
  void pump();
}

async function pump(): Promise<void> {
  if (running) return;
  running = true;
  try {
    for (let id = queue.shift(); id !== undefined; id = queue.shift()) {
      try {
        pictures.byId[id] = await invoke<TrackPicture | null>("online_track_picture", { trackId: id });
      } catch (e) {
        console.error("online_track_picture", id, e);
        pictures.byId[id] = null;
      }
    }
  } finally {
    running = false;
  }
}
