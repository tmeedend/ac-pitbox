// Typed bridge to the Create Your Own commands (`commands/story.rs`, over
// `src-tauri/src/story/studio/`). Every type mirrors its Rust twin - keep
// them aligned. A chapter travels in the shape of its YAML file
// (`story_engine::model`): what the editor shows is what the file says.
import { invoke } from "@tauri-apps/api/core";
import type { PlayerChoice, PlayerStep } from "./player/types";
import type { AppShown, PhoneLook, SceneShown, SkinShown } from "./play";
import type { ScenesFile } from "./scenes";

/** What shows the story (SPEC-story-mode, Content types, templates and
 * frames): `none` (a visual novel) or a frame's name. There is no default:
 * the author always picks it. */
export type Frame = string;

/** A flow node, one key naming its kind (SPEC-story-mode, Flow nodes). */
export interface FlowNode {
  id: string;
  /** `frame`: the frame from this beat on (`none` removes it); left out,
   * it does not change. */
  beat?: {
    effects?: string[];
    advance?: string;
    clock?: string;
    frame?: Frame;
    /** A place of `scenes/scenes.yaml`, from this beat on. */
    scene?: string;
    /** A variant of the place forced until a place is set again. */
    variant?: string;
    /** `none` clears them. */
    weather?: string;
    mood?: string;
    /** Apps that show from this beat on, by id. */
    unlock?: string[];
    /** What the phone's status bar shows from this beat on; what is left
     * out stays as it was, a `date` of `""` gives the story's back. */
    status?: { date?: string; battery?: number; signal?: number; wifi?: boolean; airplane?: boolean };
    /** App id -> the skin it wears from this beat on. */
    skins?: Record<string, string>;
  };
  message?: string;
  wait_reply?: boolean;
  /** Reply label → the node it leads to; a label left out takes `next`. */
  on_reply?: Record<string, string>;
  event?: string;
  on_failure?: string;
  branch?: BranchCase[];
  otherwise?: string;
  action?: string;
  side_events?: { open?: string[]; close?: string[] };
  end?: string;
  /** Without it, the node leads to the one after it, then to the next
   * chapter. */
  next?: string;
}

export interface BranchCase {
  if: string[];
  goto: string;
}

export interface Chapter {
  id: string;
  title?: string;
  nodes: FlowNode[];
}

export interface ChapterFile {
  file: string;
  /** What the file held when read: a save is refused once it changed. */
  stamp: string;
  /** `null` when the file does not read; `error` says why. */
  chapter: Chapter | null;
  error: string | null;
}

/** How a dialogue or an illustration comes on the scene, and how a
 * dialogue's portraits come, go and change face. */
export type Transition = "fade" | "cut";

export type ContentType =
  | "message"
  | "call"
  | "mail"
  | "article"
  | "social_post"
  | "video"
  | "listing"
  | "statement"
  | "calendar_entry"
  | "dialogue"
  | "illustration"
  | "document";

/** A `.story` item's header, as written (`story_engine::model::ContentHeader`):
 * a key left out is absent from the file. Its placement is told by which keys
 * are there (`placementOf`). */
export interface ContentHeader {
  id: string;
  type: ContentType;
  from?: string;
  /** `HH:MM`. */
  clock?: string;
  /** Dialogue only. */
  characters?: string[];
  /** Dialogue only: the place it plays in, a scene of `scenes/scenes.yaml`. */
  scene?: string;
  /** Illustration only: its picture, a path inside `media/`. */
  picture?: string;
  /** Document only: how it is laid out; a letter when absent. */
  layout?: "newspaper" | "letter";
  /** What plays on the scene only: its look, a skin of `frames/skins/`. */
  skin?: string;
  /** Dialogue or illustration only: how it comes on the scene; a fade when
   * absent. */
  transition?: Transition;
  pool?: string;
  /** Sentences, all of which must hold. */
  when?: string[];
  /** `3 days`. */
  delay?: string;
  priority?: number;
  repeat?: "once" | "each_time";
}

/** A content item as the editor edits it: its header and its Ink text. */
export interface ContentDoc {
  header: ContentHeader;
  body: string;
}

/** A `.story` file: its items, in file order. */
export interface ContentFile {
  items: ContentDoc[];
}

export interface ContentRef {
  id: string;
  file: string;
  placement: "flow" | "reactive" | "pool";
  /** Its content type, as a `.story` header names it (`message`, `dialogue`…). */
  kind: string;
  /** Its reply labels, in the order written. */
  labels: string[];
  /** The pool a variant belongs to. */
  pool: string | null;
  from: string | null;
  /** Its conditions, as written. */
  when: string[];
  /** The first line of its text, to recognise it. */
  firstLine: string | null;
}

export type EventType = "race" | "hot_lap" | "time_attack" | "test_drive" | "track_day";

export type GridStart = "random" | "first" | "second" | "last";

/** What the author sets of an event's session; a key left out is Pit Box's
 * default, and what is not here at all stays the player's. */
export interface Session {
  /** Folder ids, from the library. */
  track?: string;
  layout?: string;
  car?: string;
  /** Race only, like the three below. */
  laps?: number;
  /** Cars on the grid, the player's included. */
  grid?: number;
  ai_level?: number;
  start?: GridStart;
  /** `HH:MM`. */
  time?: string;
  /** A weather intent (`clear`, `rain`…), not a weather folder. */
  weather?: string;
  /** Race only: the opponents the author places, in order. */
  opponents?: StoryOpponent[];
}

/** One opponent of a race's grid (`story_engine::model::Opponent`); a key
 * left out is the event's (`car`, `ai_level`), the driver's, or the
 * launcher's to pick. */
export interface StoryOpponent {
  /** A driver of the roster; none: the livery's name. */
  driver?: string;
  /** Folder ids, from the library. */
  car?: string;
  skin?: string;
  ai_level?: number;
  /** 0 to 100; 0 is left out. */
  ballast?: number;
  restrictor?: number;
}

export type DriverRole = "friend" | "rival" | "star" | "filler";

/** A driver of `drivers/roster.yaml` (`story_engine::model::Driver`). */
export interface Driver {
  id: string;
  /** Display name; a driver who is a character and has none takes the
   * character's. */
  name?: string;
  /** The character this driver is on track. */
  character?: string;
  /** A country as the game names it in full. */
  nationality?: string;
  /** Age at the start of the story. */
  age?: number;
  role?: DriverRole;
  /** 0 to 100. */
  ai_level?: number;
  aggression?: number;
}

export interface Roster {
  drivers: Driver[];
}

/** `events/<id>.yaml` (`story_engine::model::Event`), every default spelt
 * out. */
export interface StoryEvent {
  id: string;
  /** Display name, in the story's language. */
  name: string | null;
  type: EventType;
  session: Session;
  /** Sentences, all of which must hold: `position <= 3`, `finished`,
   * `best_lap < 1:18.200`, `ahead_of <driver>`. */
  success: string[];
  on_success: { effects: string[]; advance: string | null };
  on_failure: { effects: string[]; pool: string | null; pick: number };
  entry_fee: number | null;
}

/** `story.yaml` (`story_engine::model::Manifest`), every default spelt out:
 * the editor never has to know them, the engine leaves them out of the
 * file. */
export interface Manifest {
  format: number;
  id: string;
  title: string;
  authors: string[];
  version: string;
  /** The first one is the source language. */
  languages: string[];
  requires: { pitbox?: string; csp?: string };
  start: Start;
  settings: Settings;
}

export interface Start {
  node: string;
  /** Required: the platform never picks a frame for the author. */
  frame: Frame;
  /** The place at first; none, a plain background. */
  scene?: string;
  /** `YYYY-MM-DD`; a relative story starts on `0001-01-01`. */
  date: string;
  /** `HH:MM`. */
  clock: string;
  reputation: number;
  followers: number;
  money: number;
  cars: string[];
}

export type DateDisplay = "relative" | "real" | "seasons" | "none";

/** The settings the engine interprets (SPEC-story-mode, Story settings). */
export interface Settings {
  date: DateDisplay;
  failure: "retry" | "final";
  /** `null`: retry forever. */
  max_attempts: number | null;
  retry_cost: "free" | "paid";
  adaptive_difficulty: { every: number; cap: number } | null;
  saves: { autosave: "per_day" | "manual"; reload: boolean };
  money: boolean;
  protagonist: "fixed" | "chosen";
  /** From worst to best. */
  relations: { scale: string[]; start: string };
}

export interface Character {
  id: string;
  name?: string;
  /** Starting step, when it is not the scale's start. */
  relation?: string;
  /** Expression → picture, as a path inside the story's `media/`; at
   * least `neutral` when there are any. */
  portraits?: Record<string, string>;
}

export interface Characters {
  characters: Character[];
}

export interface Variables {
  flags?: string[];
  /** Name → starting value. */
  counters?: Record<string, number>;
}

/** A story-wide file the editor writes. `stamp` is empty for a world file
 * the story does not have yet; `value` is `null` when the file does not
 * read, and `error` says why. */
export interface EditedFile<T> {
  file: string;
  stamp: string;
  value: T | null;
  error: string | null;
}

/** `frames/frames.yaml`, as far as the editor reads it: the frames' ids and
 * names (`story_engine::frame::Frames`). */
export interface FramesFile {
  frames: FrameDecl[];
}

/** A frame a story declares (`story_engine::frame::FrameDecl`): a key
 * left out is absent from the file. */
export interface FrameDecl {
  id: string;
  kind: "phone";
  name?: string;
  body?: { file: string; screen: { x: number; y: number; width: number; height: number; radius?: number } };
  wallpaper?: string;
  status_bar?: "full" | "simple" | "none";
  sounds?: { tap?: string; send?: string };
  apps: FrameApp[];
}

export interface FrameApp {
  id: string;
  name?: string;
  icon?: string;
  template: string;
  shows: string[];
  appears?: "first_item" | "always" | "beat";
  skin?: string;
}

/** A skin's `skin.yaml` (`story_engine::skin::Skin`); its id is its
 * folder's. */
export interface SkinFile {
  template: string;
  colors?: Record<string, string>;
  font?: string;
  slots?: Record<string, { file: string; slice?: [number, number, number, number] }>;
}

/** A template as the engine knows it (`studio::TemplateInfo`). */
export interface TemplateInfo {
  name: string;
  /** An app can be it; otherwise it plays on the scene. */
  app: boolean;
  shows: string[];
  slots: string[];
}

export type NameKind = "character" | "flag" | "counter" | "step" | "event" | "driver" | "scene";

/** A line citing a name. */
export interface Usage {
  file: string;
  line: number;
  /** The flow node, in a chapter file. */
  node: string | null;
  /** The content item, in a `.story` file. */
  item: string | null;
  text: string;
}

export interface NameUsages {
  kind: NameKind;
  id: string;
  usages: Usage[];
}

/** Whether the editor can undo or redo (`story/history.rs`). */
export interface HistoryStatus {
  canUndo: boolean;
  canRedo: boolean;
}

/** A copy of a story (`story/backups.rs`). */
export interface Backup {
  /** When it was taken, `YYYY-MM-DD_HH-MM-SS`, local time. */
  id: string;
  files: number;
}

export interface Diagnostic {
  severity: "error" | "warning";
  file: string;
  node: string | null;
  /** The engine's own words, in English while the editor is reserved. */
  message: string;
}

export interface OpenStory {
  path: string;
  manifest: EditedFile<Manifest>;
  characters: EditedFile<Characters>;
  variables: EditedFile<Variables>;
  /** `drivers/roster.yaml`: read as empty until its first save. */
  drivers: EditedFile<Roster>;
  /** `scenes/scenes.yaml`: read as empty until its first save. */
  scenes: EditedFile<ScenesFile>;
  /** `frames/frames.yaml`: the story's own frames, edited by hand so far. */
  frames: EditedFile<FramesFile>;
  /** The skins of `frames/skins/`, and the template each dresses. */
  /** One per `frames/skins/<id>/skin.yaml`. */
  skins: EditedFile<SkinFile>[];
  /** The skins' own files, as paths inside the story. */
  skin_files: string[];
  templates: TemplateInfo[];
  chapters: ChapterFile[];
  content: ContentRef[];
  /** One per file of `events/`, in file-name order. */
  events: EditedFile<StoryEvent>[];
  /** One per `.story` file of `content/`, in file-name order. */
  contentFiles: EditedFile<ContentFile>[];
  diagnostics: Diagnostic[];
  usages: NameUsages[];
  /** The files under `media/`, as paths inside it. */
  media: string[];
  history: HistoryStatus;
  /** The permission register and what it says. */
  package: PackageView;
}

/** Whether the author of a file let it travel in a package
 * (`story_engine::credits::Permission`). */
export type Permission = "granted" | "requested" | "not_asked" | "not_needed";

/** One line of a `credits.yaml` (`story_engine::credits::Credit`). */
export interface Credit {
  /** Paths inside the register's folder; `*` stands for any part of a name. */
  files: string[];
  source?: string;
  author?: string;
  licence?: string;
  permission: Permission;
  proof?: string;
}

/** A file that needs a credit, and the line giving it one. */
export interface Coverage {
  /** Inside the story. */
  file: string;
  /** The register it belongs in. */
  register: string;
  /** Index of the first line naming it; `null`, no credit. */
  line: number | null;
}

/** A kind of game content a story names by folder id. */
export type AcKind = "car" | "track";

/** One line of `package/manifest.yaml`: a car or track the package carries
 * (`story_engine::credits::ContentCredit`). */
export interface ContentCredit {
  kind: AcKind;
  id: string;
  /** A layer of that car or track, by name: it travels with the story, and
   * is on during the story's sessions only. */
  layer?: string;
  source?: string;
  author?: string;
  licence?: string;
  permission: Permission;
  proof?: string;
}

/** A car or track a story races on, as this machine holds it
 * (`story::holdings::Holding`). */
export interface Holding {
  kind: AcKind;
  id: string;
  /** `null`: not here. */
  folder: string | null;
  /** The game's own content: never carried by a package. */
  official: boolean;
  /** The DLC it comes from; `null` for the base game or a mod. */
  dlc: string | null;
  /** Its layers in the library, which the story can carry. */
  layers: { name: string; folder: string | null }[];
}

/** The story's registers and what they say (`story::studio::PackageView`). */
export interface PackageView {
  /** The registers the story has; one a file needs but that does not exist
   * yet is created by its first line. */
  registers: EditedFile<Credit[]>[];
  /** `package/manifest.yaml`: empty, its stamp empty, while the story has none. */
  content_register: EditedFile<ContentCredit[]>;
  /** Every car and track the story races on, as the author's machine holds it. */
  content: Holding[];
  register: {
    coverage: Coverage[];
    /** The cars and tracks the package carries, and their line. */
    content: { kind: AcKind; id: string; layer: string | null; line: number | null }[];
    diagnostics: Diagnostic[];
  };
}

/** Writes into `dir` what a host serves - the items and `manifest.json`;
 * resolves to the items' size in bytes. */
export const publishPackage = (folder: string, dir: string) =>
  invoke<number>("story_publish_package", { folder, dir });

/** Builds the story's package into `out`; resolves to its size in bytes. */
export const buildPackage = (folder: string, out: string) =>
  invoke<number>("story_build_package", { folder, out });

export interface StorySummary {
  path: string;
  title: string | null;
  chapters: number;
  errors: number;
  warnings: number;
  missing: boolean;
}

export const listStories = () => invoke<StorySummary[]>("story_list");

export const registerStory = (folder: string) => invoke<void>("story_register", { folder });

export const forgetStory = (folder: string) => invoke<void>("story_forget", { folder });

export const createStory = (folder: string, title: string, language: string, frame: Frame, chapterTitle: string) =>
  invoke<void>("story_create", { folder, title, language, frame, chapterTitle });

/** Copies the sample story Pit Box carries into an empty folder, as a story
 * of the author's. */
export const copySample = (folder: string) => invoke<void>("story_copy_sample", { folder });

export const openStory = (folder: string) => invoke<OpenStory>("story_open", { folder });

/** What the studio re-reads after every save, from one reading of the
 * folder. */
export interface Checked {
  diagnostics: Diagnostic[];
  usages: NameUsages[];
  media: string[];
  skin_files: string[];
  history: HistoryStatus;
  package: PackageView;
}

export const checkStory = (folder: string) => invoke<Checked>("story_check", { folder });

/** Writes one file - a chapter, an event, `story.yaml`, a world file or the
 * roster - in the shape
 * of that file; `expected` is the stamp it was read with, `null` for a file
 * the story does not have yet. Resolves to the new stamp. */
export const saveFile = (folder: string, file: string, expected: string | null, value: unknown) =>
  invoke<string>("story_save_file", { folder, file, expected, value });

/** Deletes an event's file, read with stamp `expected`. */
export const removeFile = (folder: string, file: string, expected: string) =>
  invoke<void>("story_remove_file", { folder, file, expected });

/** Copies a file the author picked into the story, at `into` (a folder of
 * `media/`, or a skin's `frames/skins/<id>`); resolves to the path the
 * story names it by. */
export const importFile = (folder: string, source: string, into: string) =>
  invoke<string>("story_import_file", { folder, source, into });

/** Renames a name and every sentence citing it, in every file. */
export const renameName = (folder: string, kind: NameKind, from: string, to: string) =>
  invoke<void>("story_rename", { folder, kind, from, to });

/** Ctrl+Z: puts back the files of the last edit; resolves to the files
 * written, empty when there was nothing to undo. */
export const undoEdit = (folder: string) => invoke<string[]>("story_undo", { folder });

/** Ctrl+Y: writes again the files of the last edit undone. */
export const redoEdit = (folder: string) => invoke<string[]>("story_redo", { folder });

/** The story's copies, newest first. */
export const listBackups = (folder: string) => invoke<Backup[]>("story_backups", { folder });

/** Puts the story back as it was in copy `id`; resolves to the files
 * written or removed. */
export const restoreBackup = (folder: string, id: string) => invoke<string[]>("story_restore", { folder, id });

/** A content item's text as the Form view edits it (`story_engine::script`):
 * the tree the engine reads, which it alone parses and writes. */
export type ScriptItem =
  | {
      kind: "line";
      /** Sentences, all of which must hold. */
      when?: string[];
      /** A character's id; none: the item's sender. */
      speaker?: string;
      text: string;
      expression?: string;
      position?: "left" | "center" | "right";
      /** Dialogue only: the speaker leaves the stage after this line. */
      leave?: boolean;
      line?: number;
    }
  | { kind: "effect"; effect: string; line?: number }
  | { kind: "choices"; choices: ScriptChoice[] }
  | { kind: "gather" }
  | { kind: "end" };

export interface ScriptChoice {
  label: string;
  when?: string[];
  text: string;
  body: ScriptItem[];
  line?: number;
}

export interface Script {
  items: ScriptItem[];
}

export interface ScriptRead {
  script: Script;
  /** What does not read, by line of the text. */
  errors: { line: number; message: string }[];
}

/** Reads a text into the tree the Form view edits. */
export const readScript = (text: string) => invoke<ScriptRead>("story_script_read", { text });

/** Writes the tree back as Ink, in canonical form; refused when a sentence
 * does not read. */
export const writeScript = (script: Script) => invoke<string>("story_script_write", { script });

/** What the Content screen's preview shows for one item (`studio::PreviewView`). */
export interface PreviewView {
  /** A dialogue on the scene, an item in its frame, or nothing to show. */
  view: "scene" | "frame" | "nowhere";
  frame: string | null;
  /** The app of that frame showing it; `null` when it is listed by name. */
  app: AppShown | null;
  /** That frame's look, as the story starts it. */
  phone: PhoneLook | null;
  /** How it plays on the scene (`dialogue`, `newspaper`, `tv`…); `null`
   * in a frame, or an illustration. */
  staged_as: string | null;
  skin: SkinShown | null;
  /** The checks' warning, for `nowhere`. */
  nowhere: string | null;
  kind: ContentType;
  from: string | null;
  characters: string[];
  /** An illustration's picture, a path inside `media/`. */
  picture: string | null;
  transition: Transition;
  /** A dialogue's own place, with the values set to preview with; `null`
   * without one. */
  scene: SceneShown | null;
  steps: PlayerStep[];
  choices: PlayerChoice[];
  ended: boolean;
  sent: string[];
  /** The variables it reads, settable for the preview, with their start value. */
  knobs: { var: string; kind: "relation" | "flag" | "number"; start: string }[];
  /** What the conversation changed. */
  changes: { var: string; from: string; to: string }[];
  date: string;
  clock: string;
}

/** Plays content `content` for the preview, with `setup` (effect sentences)
 * applied first and `replies` replayed. */
export const previewContent = (folder: string, content: string, setup: string[], replies: string[]) =>
  invoke<PreviewView>("story_preview", { folder, content, setup, replies });
