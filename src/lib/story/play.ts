// Typed bridge to Story Mode's commands (`commands/story.rs`, over
// `src-tauri/src/story/play.rs` and `story_engine::view`). Every type mirrors
// its Rust twin - keep them aligned.
import { invoke } from "@tauri-apps/api/core";
import type { PlayerChoice, PlayerStep } from "./player/types";
import type { EventResult } from "./race";
import type { Character, ContentType, DateDisplay, Driver, EventType, Session } from "./studio";

/** Where a story is read from. */
export type Source = { kind: "sample" } | { kind: "folder"; path: string };

/** A story the hub offers, and the game under way in it. */
export interface Playable {
  source: Source;
  /** `null` when the story does not read. */
  title: string | null;
  /** Errors the checks find: a story with any does not start. */
  errors: number;
  /** The game under way; `ended` once the story reached an ending,
   * `problem` (an i18n key) when it cannot resume. */
  game: { ended: boolean; problem: string | null } | null;
}

/** One delivery of a content item and what was said in it since. */
export interface SeenItem {
  content: string;
  kind: ContentType;
  from: string | null;
  characters: string[];
  date: string;
  clock: string;
  steps: PlayerStep[];
  /** What its conversation offers now. */
  choices: PlayerChoice[];
  /** Its conversation waits at a choice point. */
  open: boolean;
  seen: boolean;
}

export type Waiting =
  | {
      kind: "event";
      event: string;
      name: string | null;
      event_type: EventType;
      session: Session;
      /** Its success criteria, as written. */
      success: string[];
      attempt: number;
      fee: number | null;
      difficulty_step: number;
    }
  | { kind: "reply"; content: string }
  | { kind: "action"; action: string }
  | { kind: "ended"; ending: string };

/** The game as the screen shows it (`story_engine::view::PlayView`). */
export interface PlayView {
  title: string;
  /** `none`, or a frame's name. */
  frame: string;
  date: string;
  clock: string;
  date_display: DateDisplay;
  /** Only when the story has an economy. */
  money: number | null;
  reputation: number;
  followers: number;
  /** In the order they arrived. */
  items: SeenItem[];
  waiting: Waiting;
  /** The event a session was launched for, until its result comes back. */
  launched: string | null;
}

export interface Game {
  view: PlayView;
  characters: Character[];
  drivers: Driver[];
  /** The story's folder, where its media are; `null` for the sample. */
  folder: string | null;
  /** When the session under way was launched (ms since the epoch). */
  launched_at_ms: number | null;
}

/** What the player did. */
export type Move =
  | { kind: "reply"; content: string; reply: string }
  | { kind: "action_done"; action: string }
  | { kind: "seen"; content: string }
  /** A session starts: the entry fee is paid now. */
  | { kind: "launch"; event: string }
  | { kind: "result"; event: string; result: EventResult };

export const listPlayable = () => invoke<Playable[]>("story_play_list");

/** Starts a new game, replacing the one under way. */
export const newGame = (source: Source) => invoke<Game>("story_play_new", { source });

export const resumeGame = (source: Source) => invoke<Game>("story_play_resume", { source });

/** Plays a move and saves; resolves to where the game now stands. */
export const playMove = (source: Source, input: Move) => invoke<Game>("story_play_move", { source, input });
