// The scene behind the frame and the dialogues, pure (SPEC-story-mode, Scenes,
// frame and sound): which place shows, how blurred, and whether a change of
// it fades or cuts. Shared by Story Mode and the Content screen's preview.
import type { SceneShown, SeenItem } from "../play";
import type { Transition } from "../studio";

/** The blur behind the frame, and while the scene holds the stage (a
 * dialogue, the journal): nearer, not sharp, so the stage stays in front. */
export const BLUR_PX = { framed: 6, staged: 1 } as const;

/** The place the scene shows: the one the dialogue on stage plays in, else
 * the current one; `null`, a plain background. */
export function sceneShown(current: SceneShown | null, onStage: SeenItem | null): SceneShown | null {
  return onStage?.scene ?? current;
}

/** What tells one picture of the scene from another: a change of place, of
 * variant or of file. The empty string is the plain background. */
export function sceneKey(scene: SceneShown | null): string {
  return scene ? `${scene.place}\n${scene.variant}\n${scene.file}` : "";
}

/** The scene's blur in pixels. A variant that says `blur: false` is never
 * blurred; the plain background is, like a place. */
export function sceneBlur(scene: SceneShown | null, staged: boolean): number {
  if (scene && !scene.blur) return 0;
  return staged ? BLUR_PX.staged : BLUR_PX.framed;
}

/** Follows the stage to tell how the scene changes: a cut when the dialogue
 * or illustration that comes on stage with the change says `cut`, or the one
 * that leaves it does; a fade otherwise - a Beat moving the story elsewhere
 * between two dialogues included. */
export class SceneChanges {
  #key: string | null = null;
  #transition: Transition | null = null;

  /** Called with every new state of the stage: the scene shown and the
   * transition of the item on stage (`null` without one). Says whether the
   * picture changes, and how; `null` when it stays. */
  next(scene: SceneShown | null, transition: Transition | null): "fade" | "cut" | null {
    const key = sceneKey(scene);
    const before = this.#transition;
    this.#transition = transition;
    if (key === this.#key) return null;
    const first = this.#key === null;
    this.#key = key;
    // The first picture is there when the screen opens: nothing to fade from.
    if (first) return "cut";
    return transition === "cut" || before === "cut" ? "cut" : "fade";
  }
}
