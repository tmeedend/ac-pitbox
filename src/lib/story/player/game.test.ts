import { describe, expect, it } from "vitest";
import type { SeenItem } from "../play";
import { currentDialogue, goals, phoneThreads, withoutTemplate } from "./game";

function item(content: string, o: Partial<SeenItem> = {}): SeenItem {
  return {
    content,
    kind: "message",
    from: "friend",
    characters: [],
    date: "2001-03-01",
    clock: "09:00",
    steps: [{ kind: "said", speaker: null, text: content, expression: null, position: null }],
    choices: [],
    open: false,
    seen: true,
    ...o,
  };
}

describe("phoneThreads", () => {
  // One thread per sender, the latest first; the open message's replies at
  // the bottom; unread messages counted.
  it("groups messages by sender, the latest thread first", () => {
    const threads = phoneThreads([
      item("a"),
      item("b", { from: "sponsor", clock: "10:00" }),
      item("c", { open: true, seen: false, clock: "11:00", choices: [{ label: "yes", text: "Yes" }] }),
      item("d", { kind: "dialogue" }),
    ]);
    expect(threads.map((t) => t.from)).toEqual(["friend", "sponsor"]);
    const friend = threads[0];
    expect(friend.steps.map((s) => (s.kind === "said" ? s.text : ""))).toEqual(["a", "c"]);
    expect(friend.replyTo).toBe("c");
    expect(friend.choices).toEqual([{ label: "yes", text: "Yes" }]);
    expect(friend.unseen).toEqual(["c"]);
  });
});

describe("currentDialogue", () => {
  // The scene plays the latest dialogue until it is answered and read;
  // once closed (seen), it is part of the past.
  it("is the latest dialogue while open or unread", () => {
    const talk = item("talk", { kind: "dialogue", seen: false });
    expect(currentDialogue([item("a"), talk, item("b")])).toBe(1);
    expect(currentDialogue([{ ...talk, seen: true }])).toBeNull();
    expect(currentDialogue([{ ...talk, seen: true, open: true }])).toBe(0);
    expect(currentDialogue([talk, { ...talk, seen: true }])).toBeNull();
    expect(currentDialogue([item("a")])).toBeNull();
  });
});

describe("withoutTemplate", () => {
  it("lists what the phone cannot show yet", () => {
    expect(withoutTemplate([item("a"), item("m", { kind: "mail" }), item("d", { kind: "dialogue" })])).toHaveLength(1);
  });
});

describe("goals", () => {
  // The summary before a race says what success takes, a rival by name;
  // with no criterion, finishing is the goal.
  it("say each criterion in the player's words", () => {
    expect(goals(["position <= 3", "ahead_of rival"], (id) => id.toUpperCase())).toEqual([
      { key: "story.play.goalPosition", values: { n: 3 } },
      { key: "story.play.goalAhead", values: { name: "RIVAL" } },
    ]);
    expect(goals([], (id) => id)).toEqual([{ key: "story.play.goalFinish", values: {} }]);
  });
});
