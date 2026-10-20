// Typed bridge to the Create Your Own commands (`commands/story.rs`, over
// `src-tauri/src/story/studio.rs`). Every type mirrors its Rust twin - keep
// them aligned. A chapter travels in the shape of its YAML file
// (`story_engine::model`): what the editor shows is what the file says.
import { invoke } from "@tauri-apps/api/core";

/** What shows the story (SPEC-story-mode, Content types, templates and
 * frames): `none` (a visual novel) or a frame's name. There is no default:
 * the author always picks it. */
export type Frame = string;

/** A flow node, one key naming its kind (SPEC-story-mode, Flow nodes). */
export interface FlowNode {
  id: string;
  /** `frame`: the frame from this beat on (`none` removes it); left out,
   * it does not change. */
  beat?: { effects?: string[]; advance?: string; clock?: string; frame?: Frame };
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
  | "dialogue";

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
  /** Dialogue only: a name, until the story's scenes exist. */
  scene?: string;
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

export type NameKind = "character" | "flag" | "counter" | "step" | "event";

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
}

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

export const openStory = (folder: string) => invoke<OpenStory>("story_open", { folder });

/** What the studio re-reads after every save, from one reading of the
 * folder. */
export interface Checked {
  diagnostics: Diagnostic[];
  usages: NameUsages[];
  media: string[];
  history: HistoryStatus;
}

export const checkStory = (folder: string) => invoke<Checked>("story_check", { folder });

/** Writes one file - a chapter, an event, `story.yaml` or a world file - in the shape
 * of that file; `expected` is the stamp it was read with, `null` for a file
 * the story does not have yet. Resolves to the new stamp. */
export const saveFile = (folder: string, file: string, expected: string | null, value: unknown) =>
  invoke<string>("story_save_file", { folder, file, expected, value });

/** Deletes an event's file, read with stamp `expected`. */
export const removeFile = (folder: string, file: string, expected: string) =>
  invoke<void>("story_remove_file", { folder, file, expected });

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
