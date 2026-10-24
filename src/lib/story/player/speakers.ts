// Who speaks in a story, as the player-side views show them: a name, and a
// picture per expression from the story's `media/` folder. Shared by the
// Content screen's preview and Story Mode.
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Character } from "../studio";
import type { Speaker } from "./types";

/** `characters` as speakers; `folder` is the story's, `null` for the
 * sample Pit Box carries. A character without a portrait shows as initials. */
export function speakersOf(characters: Character[], folder: string | null): Record<string, Speaker> {
  const out: Record<string, Speaker> = {};
  for (const c of characters) {
    const portraits: Record<string, string> = {};
    for (const [expression, file] of Object.entries(c.portraits ?? {})) {
      const url = mediaUrl(folder, file);
      if (url) portraits[expression] = url;
    }
    out[c.id] = { name: c.name ?? c.id, portraits };
  }
  return out;
}

/** A file of the story's own skins (`frames/skins/<id>/<file>`, `path`
 * inside the story) as a URL the page can show; `null` without a path. */
export function storyFileUrl(folder: string | null, path: string | null | undefined): string | null {
  if (!path) return null;
  if (folder) return convertFileSrc(`${folder}/${path}`);
  return `http://storymedia.localhost/file/${path.split("/").map(encodeURIComponent).join("/")}`;
}

/** A file of the story's `media/` (`path` inside it) as a URL the page can
 * show; `null` without a path. `folder` is the story's, read through the
 * asset protocol; `null` for the sample Pit Box carries, whose media are
 * served from the bytes it carries (`storymedia`, `story/media.rs`). */
export function mediaUrl(folder: string | null, path: string | null | undefined): string | null {
  if (!path) return null;
  if (folder) return convertFileSrc(`${folder}/media/${path}`);
  return `http://storymedia.localhost/sample/${path.split("/").map(encodeURIComponent).join("/")}`;
}
