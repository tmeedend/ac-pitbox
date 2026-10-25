// Typed bridge to Story Mode's commands (`commands/story.rs`, over
// `src-tauri/src/story/play.rs` and `story_engine::view`). Every type mirrors
// its Rust twin - keep them aligned.
import { invoke } from "@tauri-apps/api/core";
import type { PlayerChoice, PlayerStep } from "./player/types";
import type { EventResult } from "./race";
import type {
  AcKind,
  Character,
  ContentType,
  DateDisplay,
  Driver,
  EventType,
  Holding,
  Session,
  Transition,
} from "./studio";

/** Where a story is read from: the sample Pit Box carries, a story folder
 * of Create Your Own, or a package the player installed. */
export type Source = { kind: "sample" } | { kind: "folder"; path: string } | { kind: "installed"; id: string };

/** A story the hub offers, and the game under way in it. */
export interface Playable {
  source: Source;
  /** `null` when the story does not read. */
  title: string | null;
  /** As its `story.yaml` says; `null` when the story does not read. */
  version: string | null;
  /** Errors the checks find: a story with any does not start. */
  errors: number;
  /** The game under way; `ended` once the story reached an ending,
   * `problem` (an i18n key) when it cannot resume. */
  game: { ended: boolean; problem: string | null } | null;
  /** What it races on that this machine lacks - a story missing any does
   * not start - or holds in another version than the package's. */
  content: { missing: Holding[]; other_version: [AcKind, string][] };
}

/** An app of the frame up (`story_engine::view::AppShown`). */
export interface AppShown {
  id: string;
  /** As the story names it; `null`, Pit Box names it (`appName`). */
  name: string | null;
  /** A path inside the story's `media/`; `null`, its template's. */
  icon: string | null;
  template: "thread" | "inbox" | "feed" | "statement" | "agenda" | "call";
  /** The content types it shows. */
  shows: string[];
  /** On the home screen now. */
  available: boolean;
  /** Its look now; `null`, Pit Box's theme. */
  skin: SkinShown | null;
}

/** A skin as the screen draws it (`story_engine::view::SkinShown`): its
 * files as paths inside the story. */
export interface SkinShown {
  id: string;
  /** `background`, `text`, `muted`, `accent` -> `#rrggbb`. */
  colors: Record<string, string>;
  font: string | null;
  slots: Record<string, { file: string; slice?: [number, number, number, number] | null }>;
}

/** The phone up, as its frame and the Beats make it look
 * (`story_engine::view::PhoneLook`). */
export interface PhoneLook {
  /** Its picture, a path inside `media/`, and the screen's rectangle in its
   * pixels; `null`, Pit Box draws one. */
  body: { file: string; screen: { x: number; y: number; width: number; height: number; radius?: number } } | null;
  wallpaper: string | null;
  status_bar: "full" | "simple" | "none";
  status: { date: string | null; battery: number; signal: number; wifi: boolean; airplane: boolean };
  /** Paths inside `media/`. */
  sounds: { tap?: string; send?: string };
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
  /** How it played on the scene (`dialogue`, `newspaper`, `letter`, `tv`,
   * `landline`), with the frame up when it came; `null` in the frame, or
   * an illustration. */
  staged_as: string | null;
  /** Its skin, for what plays on the scene. */
  skin: SkinShown | null;
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
  /** The phone's look; `null` without a frame. */
  phone: PhoneLook | null;
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

/** What installing a package did (`story::packages::Outcome`). */
export type InstallOutcome =
  | {
      kind: "installed";
      id: string;
      title: string;
      version: string;
      previous: string | null;
      /** The cars and tracks the player lacked, as archives for the ordinary import. */
      imports: string[];
    }
  /** The game under way would not resume on this version: nothing changed. */
  | { kind: "loses_game"; title: string; version: string; previous: string };

/** Installs a `.pitstory`, or updates the story it carries; `loseGame` goes
 * ahead when the game under way would not resume on it. */
export const installPackage = (file: string, loseGame: boolean) =>
  invoke<InstallOutcome>("story_package_install", { file, loseGame });

/** Installs the story whose package's manifest is at `url`. */
export const installFromUrl = (url: string, loseGame: boolean) =>
  invoke<InstallOutcome>("story_package_install_url", { url, loseGame });

/** Updates an installed story from the URL it came from. */
export const updatePackage = (id: string, loseGame: boolean) =>
  invoke<InstallOutcome>("story_package_update", { id, loseGame });

/** An update an installed story can take (`story::packages::Update`). */
export interface PackageUpdate {
  id: string;
  version: string;
  /** What it would download, in bytes. */
  size: number;
}

/** The updates of the stories installed from a URL: one request each. */
export const packageUpdates = () => invoke<PackageUpdate[]>("story_package_updates");

/** Stops the package download under way. */
export const cancelDownload = () => invoke<void>("story_package_cancel");

/** An archive an install set aside, once the import is done with it. */
export const forgetImported = (archive: string) => invoke<void>("story_package_imported", { archive });

/** Removes an installed story; its game is kept for a new install. */
export const uninstallStory = (id: string) => invoke<void>("story_package_uninstall", { id });
