import { describe, expect, it } from "vitest";
import type { SceneShown, SeenItem } from "../play";
import { BLUR_PX, SceneChanges, sceneBlur, sceneShown } from "./scene";

function place(place: string, variant = "default", o: Partial<SceneShown> = {}): SceneShown {
  return { place, variant, file: `scenes/${place}_${variant}.webp`, video: false, blur: true, ambient: null, ...o };
}

function dialogue(scene: SceneShown | null): SeenItem {
  return {
    content: "talk",
    kind: "dialogue",
    from: null,
    characters: [],
    picture: null,
    transition: "fade",
    scene,
    staged_as: "dialogue",
    skin: null,
    date: "2001-03-01",
    clock: "09:00",
    steps: [],
    choices: [],
    open: true,
    seen: false,
  };
}

describe("sceneShown", () => {
  // A dialogue with a place of its own plays in front of it; one without
  // plays in front of the current place (SPEC-story-mode, Scenes).
  it("is the dialogue's own place, else the current one", () => {
    const bedroom = place("bedroom");
    const garage = place("garage");
    expect(sceneShown(bedroom, dialogue(garage))).toBe(garage);
    expect(sceneShown(bedroom, dialogue(null))).toBe(bedroom);
    expect(sceneShown(bedroom, null)).toBe(bedroom);
    expect(sceneShown(null, null), "no place: a plain background").toBeNull();
  });
});

describe("sceneBlur", () => {
  // Blurred behind the frame, nearer while the scene holds the stage, never
  // for a variant that says `blur: false`.
  it("follows the stage, unless the variant is sharp", () => {
    expect(sceneBlur(place("garage"), false)).toBe(BLUR_PX.framed);
    expect(sceneBlur(place("garage"), true)).toBe(BLUR_PX.staged);
    expect(sceneBlur(place("garage", "tense", { blur: false }), false)).toBe(0);
    expect(sceneBlur(null, false), "the plain background, like a place").toBe(BLUR_PX.framed);
  });
});

describe("SceneChanges", () => {
  // A change of place or variant fades, or cuts when the dialogue coming on
  // stage with it, or leaving it, says `cut`; a Beat's change between two
  // dialogues fades.
  it("fades, or cuts with the dialogue that brings or ends the change", () => {
    const changes = new SceneChanges();
    const bedroom = place("bedroom");
    const garage = place("garage");
    expect(changes.next(bedroom, null), "the first picture is there at once").toBe("cut");
    expect(changes.next(bedroom, null), "the same place: no change").toBeNull();
    expect(changes.next(garage, "cut"), "a dialogue that cuts, in its own place").toBe("cut");
    expect(changes.next(bedroom, null), "and cuts back when it ends").toBe("cut");
    expect(changes.next(garage, null), "a Beat after it: a fade").toBe("fade");
    expect(changes.next(garage, "fade"), "a dialogue in the current place: no change").toBeNull();
    expect(changes.next(garage, null)).toBeNull();
    expect(changes.next(place("garage", "night"), null), "another variant fades").toBe("fade");
    expect(changes.next(null, null), "back to plain").toBe("fade");
  });

  // A dialogue that cuts but plays in the current place changes nothing;
  // its cut is not carried over to a later change it has no part in.
  it("forgets the cut of a dialogue once it has left", () => {
    const changes = new SceneChanges();
    const bedroom = place("bedroom");
    changes.next(bedroom, null);
    expect(changes.next(bedroom, "cut")).toBeNull();
    expect(changes.next(bedroom, null)).toBeNull();
    expect(changes.next(place("garage"), null)).toBe("fade");
  });
});
