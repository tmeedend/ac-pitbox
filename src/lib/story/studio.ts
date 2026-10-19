// Typed bridge to the Create Your Own commands (`commands/story.rs`, over
// `src-tauri/src/story/studio.rs`). Every type mirrors its Rust twin - keep
// them aligned. A chapter travels in the shape of its YAML file
// (`story_engine::model`): what the editor shows is what the file says.
import { invoke } from "@tauri-apps/api/core";

/** A flow node, one key naming its kind (SPEC-story-mode, Flow nodes). */
export interface FlowNode {
  id: string;
  beat?: { effects?: string[]; advance?: string; clock?: string };
  message?: string;
  wait_reply?: boolean;
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

export interface ContentRef {
  id: string;
  file: string;
  placement: "flow" | "reactive" | "pool";
  hasReplies: boolean;
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

export type NameKind = "character" | "flag" | "counter" | "step";

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
  events: string[];
  diagnostics: Diagnostic[];
  usages: NameUsages[];
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

export const createStory = (folder: string, title: string, language: string, chapterTitle: string) =>
  invoke<void>("story_create", { folder, title, language, chapterTitle });

export const openStory = (folder: string) => invoke<OpenStory>("story_open", { folder });

/** What the studio re-reads after every save, from one reading of the
 * folder. */
export interface Checked {
  diagnostics: Diagnostic[];
  usages: NameUsages[];
  history: HistoryStatus;
}

export const checkStory = (folder: string) => invoke<Checked>("story_check", { folder });

/** Writes one file - a chapter, `story.yaml` or a world file - in the shape
 * of that file; `expected` is the stamp it was read with, `null` for a file
 * the story does not have yet. Resolves to the new stamp. */
export const saveFile = (folder: string, file: string, expected: string | null, value: unknown) =>
  invoke<string>("story_save_file", { folder, file, expected, value });

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
