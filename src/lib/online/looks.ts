// Names and pictures of what the servers reference, as the library knows them
// (`online/looks.rs`). Pure, for Vitest: every function falls back on the id,
// since most of what the lobby names is not here.
import type { ServerSummary, TrackRef } from "./online";

export interface LayoutLook {
  name: string;
  /** Absolute paths, for `previewSrc`. */
  preview: string | null;
  outline: string | null;
}

export interface TrackLook {
  name: string;
  /** Its track categories in the library, what the category token reads. */
  categories: string[];
  /** Keyed by lowercase layout folder, `""` for a single layout. */
  layouts: Record<string, LayoutLook>;
}

/** Keyed by lowercase id. */
export interface Looks {
  cars: Record<string, string>;
  tracks: Record<string, TrackLook>;
}

export const NO_LOOKS: Looks = { cars: {}, tracks: {} };

export function layoutLook(looks: Looks, track: TrackRef): LayoutLook | null {
  const t = looks.tracks[track.id.toLowerCase()];
  return t?.layouts[(track.layout ?? "").toLowerCase()] ?? null;
}

/** The layout's own name ("Nordschleife - Tourist"), else the track's, else
 * the folder. */
export function trackTitle(looks: Looks, track: TrackRef): string {
  return layoutLook(looks, track)?.name ?? looks.tracks[track.id.toLowerCase()]?.name ?? track.id;
}

/** The track's own name, else its folder. */
export function trackName(looks: Looks, track: TrackRef): string {
  return looks.tracks[track.id.toLowerCase()]?.name ?? track.id;
}

/**
 * The layout as one reads it under the track's name — "Main Layout", not
 * `main_layout`. A layout's `ui_track.json` usually repeats the track
 * ("Shutoko Revival Project 0.9.3 - Main Layout"): the repetition goes. A
 * single-layout track has none; a layout the library does not know keeps its
 * folder, the only name there is.
 */
export function layoutName(looks: Looks, track: TrackRef): string | null {
  if (!track.layout) return null;
  const name = layoutLook(looks, track)?.name;
  if (!name) return track.layout;
  const prefix = trackName(looks, track);
  if (!name.toLowerCase().startsWith(prefix.toLowerCase())) return name;
  const rest = name.slice(prefix.length).replace(/^[\s\-–—:·|]+/, "");
  return rest || name;
}

export function isKnownCar(looks: Looks, id: string): boolean {
  return id.toLowerCase() in looks.cars;
}

export function carName(looks: Looks, id: string): string {
  return looks.cars[id.toLowerCase()] ?? id;
}

/** The cars one has first (SPEC-play-online.md, "Voitures"), each group in
 * the server's own order. */
export function carsOwnedFirst(looks: Looks, cars: string[]): string[] {
  return [...cars.filter((c) => isKnownCar(looks, c)), ...cars.filter((c) => !isKnownCar(looks, c))];
}

/** What a search reads for a server: ids and the names they have here — a
 * track is searched as "Mount Panorama" as much as `bathurst`. */
export function searchText(looks: Looks, s: ServerSummary): string {
  return [s.name, s.track.kunos_id, trackTitle(looks, s.track), ...s.cars, ...s.cars.map((c) => carName(looks, c))]
    .join(" ")
    .toLowerCase();
}
