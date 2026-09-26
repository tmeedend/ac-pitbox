// Typed bridge to the game folder commands (DOSSIER§9). Every type mirrors its
// Rust twin in `src-tauri/src/gamestate/` - keep them aligned.
import { invoke } from "@tauri-apps/api/core";

export type NodeId = number;
export type EntryKind = "dir" | "file" | "link";
export type Population = "link" | "modFolder" | "sound" | "extra" | "origin" | "unmanaged" | "rest";
export type State = "posed" | "replacesGame" | "waiting" | "drift" | "nobody";
export type Drift = "modified" | "missing" | "orphan" | "brokenLink";
export type OwnerKind = "car" | "track" | "app" | "skin" | "trackSkin" | "sound" | "other" | "pack";
export type Presence = "inGame" | "disabled" | "unmanaged" | "origin";
/** The values of the State chip: the five states and the two marks. */
export type StateValue = State | "cmZone" | "shared";

export interface Counts {
  posed: number;
  replacesGame: number;
  waiting: number;
  drift: number;
  nobody: number;
  cmZone: number;
  shared: number;
}

export interface Owner {
  kind: OwnerKind;
  id: string;
  name: string;
  /** The car or track a livery or a sound belongs to. */
  parent: string | null;
}

export interface OwnerRef {
  kind: OwnerKind;
  id: string;
}

export interface Filters {
  states: StateValue[];
  excluded: StateValue[];
  owner: OwnerRef | null;
}

export interface Row {
  id: NodeId;
  name: string;
  kind: EntryKind;
  present: boolean;
  hasChildren: boolean;
  /** The root of a deployed mod folder. */
  modFolder: boolean;
  population: Population;
  state: State;
  drift: Drift | null;
  provider: string | null;
  /** Other mods claiming the path ("+N"). */
  others: number;
  cmZone: boolean;
  counts: Counts;
}

export interface GroupRow {
  count: number;
  label: "originCars" | "originTracks" | "nobody";
}

export interface ChildrenPage {
  /** The folder's own counts under the filters. */
  counts: Counts;
  rows: Row[];
  total: number;
  group: GroupRow | null;
}

export type Mechanism =
  | { type: "library"; copy: string }
  | { type: "link"; target: string; broken: boolean }
  | { type: "replaced"; backup: string }
  | { type: "missing" }
  | { type: "real" };

export interface ClaimView {
  owner: Owner;
  provided: boolean;
  forced: boolean;
  copyDate: string | null;
  reason: "older" | "sameDate" | "waiting" | null;
}

export interface Detail {
  id: NodeId;
  name: string;
  relPath: string;
  kind: EntryKind;
  present: boolean;
  population: Population;
  state: State;
  drift: { kind: Drift; expectedSize: number | null; actualSize: number | null } | null;
  cmZone: boolean;
  mechanism: Mechanism | null;
  owner: Owner | null;
  claims: ClaimView[];
  revert: Owner[];
  counts: Counts;
  contributors: { owner: Owner; files: number }[];
  moreContributors: number;
  folder: { owner: Owner; version: string | null; layers: string[] } | null;
}

export interface ModHit {
  owner: Owner;
  presence: Presence;
  node: NodeId | null;
  path: string | null;
  drift: boolean;
}

export interface PathHit {
  row: Row;
  parent: string;
}

export interface Group<T> {
  total: number;
  items: T[];
}

export interface SearchResults {
  mods: Group<ModHit>;
  dirs: Group<PathHit>;
  files: Group<PathHit>;
}

export interface SearchLimits {
  mods: number;
  dirs: number;
  files: number;
}

export interface Progress {
  phase: "disk" | "classify";
  entries: number;
}

export interface Status {
  running: Progress | null;
  error: string | null;
  index: {
    root: string;
    entries: number;
    scannedAt: string;
    classified: boolean;
    /** A Pit Box operation wrote into the game since the scan. */
    stale: boolean;
    summary: Counts;
  } | null;
}

export interface OwnerOption {
  owner: Owner;
  files: number;
}

export function gameFolderStatus(): Promise<Status> {
  return invoke<Status>("game_folder_status");
}

/** Resolves when the scan is over - or at once when one is already running:
 * the caller follows the `gamefolder://progress` / `gamefolder://done` events
 * either way. */
export function scanGameFolder(): Promise<void> {
  return invoke<void>("scan_game_folder");
}

export function gameFolderChildren(
  node: NodeId,
  filters: Filters,
  members: boolean,
  offset: number,
  limit: number,
): Promise<ChildrenPage> {
  return invoke<ChildrenPage>("game_folder_children", { node, filters, members, offset, limit });
}

export function gameFolderDetail(node: NodeId): Promise<Detail | null> {
  return invoke<Detail | null>("game_folder_detail", { node });
}

export function gameFolderSearch(query: string, filters: Filters, limits: SearchLimits): Promise<SearchResults> {
  return invoke<SearchResults>("game_folder_search", { query, filters, limits });
}

/** The chain of nodes down to `path` (relative to the game folder), `null`
 * when the current index does not have it. */
export function gameFolderReveal(path: string): Promise<NodeId[] | null> {
  return invoke<NodeId[] | null>("game_folder_reveal", { path });
}

export function gameFolderOwners(): Promise<OwnerOption[]> {
  return invoke<OwnerOption[]>("game_folder_owners");
}

/** "Show in Explorer": the path itself, never a junction's target. */
export function showGamePath(node: NodeId): Promise<void> {
  return invoke<void>("show_game_path", { node });
}
