// Typed bridge to Story Mode's commands (`commands/story.rs`, over
// `src-tauri/src/story/play.rs` and `story_engine::view`). Every type mirrors
// its Rust twin - keep them aligned.
import { invoke } from "@tauri-apps/api/core";
import type { PlayerChoice, PlayerStep } from "./player/types";
import type { EventResult } from "./race";
import type { Character, ContentType, DateDisplay, Driver, EventType, Session, Transition } from "./studio";

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

/** An app of the frame up (`story_engine::view::AppShown`). */
export interface AppShown {
  id: string;
  /** As the story names it; `null`, Pit Box names it (`appName`). */
  name: string | null;
  /** A path inside the story's `media/`; `null`, its template's. */
  icon: string | null;
  template: "thread" | "inbox" | "feed" | "statement";
  /** The content types it shows. */
  shows: string[];
  /** On the home screen now. */
  available: boolean;
}

/** A place as it shows now (`story_engine::scene::Shown`). */
export interface SceneShown {
  place: string;
  variant: string;
  /** A path inside the story's `media/`. */
  file: string;
  /** A video loop rather than a still image. */
  video: boolean;
  blur: boolean;
  ambient: string | null;
}

/** One delivery of a content item and what was said in it since. */
export interface SeenItem {
  content: string;
  kind: ContentType;
  from: string | null;
  characters: string[];
  /** An illustration's picture, a path inside the story's `media/`. */
  picture: string | null;
  /** How a dialogue or an illustration comes on the scene. */
  transition: Transition;
  /** A dialogue's own place; `null`, the current one. */
  scene: SceneShown | null;
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
  /** Its apps, in the order the home screen shows them. */
  apps: AppShown[];
  /** The place behind the frame; `null`, a plain background. */
  scene: SceneShown | null;
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
  /** Where the dialogue under way was being read, when last seen. */
  reading: Reading | null;
}

/** The line a dialogue is read at: `item` into the view's items, `at` into
 * its steps (`story::play::Reading`). */
export interface Reading {
  item: number;
  at: number;
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

/** Keeps, beside the save, the line the dialogue under way is read at. */
export const keepReadingAt = (source: Source, reading: Reading) =>
  invoke<void>("story_play_reading", { source, reading });
