// The Scenes screen's logic and bridge (SPEC-story-mode, Scenes, frame and
// sound; Create Your Own: Scenes): `scenes/scenes.yaml` as the editor edits
// it - the types mirror `story_engine::scene`, keep them aligned - and the
// names the Beats and the variants use. Which variant shows is the engine's
// alone (`pickVariant`): the screen never works it out itself.
import { invoke } from "@tauri-apps/api/core";
import type { SceneShown } from "./play";
import type { Chapter } from "./studio";
import { mediaImages } from "./world";

export type Season = "winter" | "spring" | "summer" | "autumn";
export type Period = "morning" | "afternoon" | "evening" | "night";

export const SEASONS: Season[] = ["winter", "spring", "summer", "autumn"];
export const PERIODS: Period[] = ["morning", "afternoon", "evening", "night"];

/** A variant of a place; a criterion left out holds always. */
export interface SceneVariant {
  id: string;
  season?: Season[];
  time?: Period[];
  weather?: string[];
  mood?: string[];
  /** A still image or a video loop, a path inside `media/`. */
  file: string;
  /** Left out, the place's. */
  blur?: boolean;
  /** Left out, the place's. */
  ambient?: string;
}

export interface ScenePlace {
  id: string;
  name?: string;
  /** Left out, blurred. */
  blur?: boolean;
  ambient?: string;
  /** Tried in order: the first that holds shows, the last is the default. */
  variants: SceneVariant[];
}

/** `scenes/scenes.yaml`. */
export interface ScenesFile {
  scenes: ScenePlace[];
}

/** The moment the preview is set to; an empty weather or mood is none. */
export interface SceneMoment {
  season: Season;
  period: Period;
  weather: string;
  mood: string;
}

/** The variant of `place` that shows at `moment`, as the engine picks it;
 * `null` when the place has nothing to show. */
export const pickVariant = (place: ScenePlace, moment: SceneMoment) =>
  invoke<SceneShown | null>("story_scene_pick", { place, moment });

/** A new place, with one variant to fill: its file is picked next, and the
 * checks say so until it is. */
export function newPlace(id: string, name: string): ScenePlace {
  return { id, name, variants: [{ id: "default", file: "" }] };
}

/** What a variant can show: the images and video loops of `media/`. */
export function sceneFiles(media: string[]): string[] {
  return [...mediaImages(media), ...media.filter(isVideo)];
}

/** What an ambient sound can be: the sounds of `media/`. */
export function soundFiles(media: string[]): string[] {
  return media.filter((p) => /\.(ogg|mp3|wav)$/i.test(p));
}

export function isVideo(path: string): boolean {
  return /\.(webm|mp4)$/i.test(path);
}

/** Sets or removes a list criterion: an empty list holds always, so it is
 * not written. */
export function setCriterion<K extends "season" | "time" | "weather" | "mood">(
  v: SceneVariant,
  key: K,
  values: NonNullable<SceneVariant[K]>,
): void {
  if (values.length) v[key] = values;
  else delete v[key];
}

/** `values` with `value` in it, or out of it: a criterion's checkbox. */
export function toggled<T>(values: T[] | undefined, value: T, on: boolean, order: T[]): T[] {
  const set = new Set(values ?? []);
  if (on) set.add(value);
  else set.delete(value);
  return order.filter((x) => set.has(x));
}

/** The weathers (or moods) the story names: those its variants ask for and
 * those its Beats set, `none` aside - what the preview and a Beat offer. */
export function storyNames(
  scenes: ScenesFile | null,
  chapters: (Chapter | null)[],
  key: "weather" | "mood",
): string[] {
  const out = new Set<string>();
  for (const p of scenes?.scenes ?? []) for (const v of p.variants) for (const w of v[key] ?? []) out.add(w);
  for (const c of chapters) {
    for (const n of c?.nodes ?? []) {
      const w = n.beat?.[key];
      if (w && w !== "none") out.add(w);
    }
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

/** The variants a Beat can force, by id: those of the place it sets; with
 * none, of every place - the current one is known only by playing. */
export function variantIds(scenes: ScenesFile | null, place: string | undefined): string[] {
  const places = (scenes?.scenes ?? []).filter((p) => !place || p.id === place);
  return [...new Set(places.flatMap((p) => p.variants.map((v) => v.id)))];
}
