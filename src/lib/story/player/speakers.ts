// Who speaks in a story, as the player-side views show them: a name, and a
// picture per expression from the story's `media/` folder. Shared by the
// Content screen's preview and Story Mode.
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Character } from "../studio";
import type { Speaker } from "./types";

/** `characters` as speakers; `folder` is the story's, `null` for a story
 * without media on disk (the sample Pit Box carries), whose characters show
 * as initials. */
export function speakersOf(characters: Character[], folder: string | null): Record<string, Speaker> {
  const out: Record<string, Speaker> = {};
  for (const c of characters) {
    const portraits: Record<string, string> = {};
    if (folder) {
      for (const [expression, file] of Object.entries(c.portraits ?? {})) {
        portraits[expression] = convertFileSrc(`${folder}/media/${file}`);
      }
    }
    out[c.id] = { name: c.name ?? c.id, portraits };
  }
  return out;
}
