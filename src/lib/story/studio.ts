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

export interface Diagnostic {
  severity: "error" | "warning";
  file: string;
  node: string | null;
  /** The engine's own words, in English while the editor is reserved. */
  message: string;
}

export interface OpenStory {
  path: string;
  title: string | null;
  /** The node the story starts on. */
  start: string | null;
  chapters: ChapterFile[];
  content: ContentRef[];
  events: string[];
  diagnostics: Diagnostic[];
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

export const checkStory = (folder: string) => invoke<Diagnostic[]>("story_check", { folder });

/** Writes one chapter; `expected` is the stamp it was read with, `null` for
 * a new chapter. Resolves to the new stamp. */
export const saveChapter = (folder: string, file: string, expected: string | null, chapter: Chapter) =>
  invoke<string>("story_save_chapter", { folder, file, expected, chapter });
