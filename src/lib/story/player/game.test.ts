import { describe, expect, it } from "vitest";
import type { AppShown, SeenItem } from "../play";
import {
  dialogueLog,
  goals,
  onStage,
  pastDialogues,
  phoneThreads,
  readBefore,
  withoutTemplate,
} from "./game";

function item(content: string, o: Partial<SeenItem> = {}): SeenItem {
  return {
    content,
    kind: "message",
    from: "friend",
    characters: [],
    picture: null,
    transition: "fade",
    scene: null,
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

describe("onStage", () => {
  // The scene plays a dialogue until it is answered and read; once closed
  // (seen), it is part of the past. L3b.3: dialogues and illustrations
  // queue, the first one not closed yet plays.
  it("plays the first dialogue or illustration still open or unread", () => {
    const talk = item("talk", { kind: "dialogue", seen: false });
    expect(onStage([item("a"), talk, item("b")])).toBe(1);
    expect(onStage([{ ...talk, seen: true }])).toBeNull();
    expect(onStage([{ ...talk, seen: true, open: true }])).toBe(0);
    expect(onStage([item("a")])).toBeNull();
    const picture = item("big", { kind: "illustration", seen: false });
    expect(onStage([{ ...talk, seen: true }, picture, talk])).toBe(1);
    expect(onStage([{ ...talk, seen: true }, { ...picture, seen: true }, talk])).toBe(2);
  });
});

describe("pastDialogues", () => {
  // L3.3, the journal: every dialogue played, latest first, never the one
  // the scene plays now, nor one that said nothing.
  it("lists the dialogues played, latest first, without the one under way", () => {
    const talk = (content: string, o: Partial<SeenItem> = {}) => item(content, { kind: "dialogue", ...o });
    const items = [talk("first"), item("a"), talk("silent", { steps: [] }), talk("second"), talk("now", { seen: false })];
    expect(pastDialogues(items)).toEqual([3, 0]);
    expect(pastDialogues(items.slice(0, 4))).toEqual([3, 0]);
    expect(pastDialogues([item("a")])).toEqual([]);
  });
});

describe("withoutTemplate", () => {
  it("lists what the phone cannot show yet", () => {
    const apps: AppShown[] = [
      { id: "messages", name: null, icon: null, template: "thread", shows: ["message"], available: true },
      { id: "mail", name: null, icon: null, template: "inbox", shows: ["mail"], available: true },
    ];
    const kinds = withoutTemplate(
      [
        item("a"),
        item("m", { kind: "mail" }),
        item("d", { kind: "dialogue" }),
        item("c", { kind: "call" }),
        item("v", { kind: "video" }),
      ],
      apps,
    ).map((i) => i.kind);
    expect(kinds).toEqual(["call", "video"]);
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

describe("reading a dialogue", () => {
  const said = (text: string) => ({ kind: "said" as const, speaker: "friend", text, expression: null, position: null });

  // L3b: fast-forward goes through what an earlier delivery of the same
  // dialogue already said, nothing else.
  it("knows the lines an earlier delivery already said", () => {
    const items = [
      item("talk", { kind: "dialogue", steps: [said("Hi."), said("Again?")] }),
      item("other", { kind: "dialogue", steps: [said("Elsewhere.")] }),
      item("talk", { kind: "dialogue", steps: [said("Hi."), said("New.")] }),
    ];
    const read = readBefore(items, 2);
    expect(read.size).toBe(2);
    expect([...read].some((k) => k.endsWith("Elsewhere."))).toBe(false);
    expect(readBefore(items, 0).size).toBe(0);
  });

  // The log: every dialogue played, the one under way up to the line shown.
  it("logs the dialogues played, the one under way up to its line", () => {
    const items = [
      item("a", { kind: "dialogue", steps: [said("1"), { kind: "chose", label: "x", text: "X" }] }),
      item("m"),
      item("b", { kind: "dialogue", steps: [said("2"), said("3")] }),
    ];
    expect(dialogueLog(items, 2, 0).map((l) => l.step.text)).toEqual(["1", "X", "2"]);
    expect(dialogueLog(items, null, 0)).toHaveLength(4);
  });
});
