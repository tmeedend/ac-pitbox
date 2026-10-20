// What the player-side views show (SPEC-story-mode, Story Mode: player side):
// lines said and replies chosen, as the engine hands them over. The Content
// screen's preview and Story Mode (L3) feed the same views with them; nothing
// here knows the editor.

export type Position = "left" | "center" | "right";

/** A line said, or a reply the player chose (`story_engine::engine::Step`). */
export type PlayerStep =
  | {
      kind: "said";
      /** A character's id; none: the item's sender. */
      speaker: string | null;
      text: string;
      expression: string | null;
      position: Position | null;
    }
  | { kind: "chose"; label: string; text: string };

/** A choice offered, answered by its label. */
export interface PlayerChoice {
  label: string;
  text: string;
}

/** How a character shows: their name, and a picture per expression. */
export interface Speaker {
  name: string;
  /** Expression → picture URL. */
  portraits: Record<string, string>;
}
