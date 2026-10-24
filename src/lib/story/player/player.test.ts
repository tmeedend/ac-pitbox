import { describe, expect, it } from "vitest";
import { dateLabel, initials, whenLabel } from "./phone";
import { autoDelay, nextLine, previousLine, stageAt } from "./stage";
import { bubbles } from "./thread";
import type { PlayerStep, Position } from "./types";

const said = (
  speaker: string | null,
  text: string,
  expression: string | null = null,
  position: Position | null = null,
): PlayerStep => ({
  kind: "said",
  speaker,
  text,
  expression,
  position,
});

describe("the dialogue stage", () => {
  // SPEC-story-mode, In-person dialogues: everyone present stands from the
  // start, in order; an expression and a position stay once given.
  it("places the cast in order and keeps faces and places once given", () => {
    const steps: PlayerStep[] = [
      said("friend", "Hi.", "happy"),
      said("rival", "Hello.", null, "center"),
      said("friend", "Again."),
    ];
    const start = stageAt(["friend", "rival"], null, steps, -1);
    expect(start.cast.map((c) => [c.who, c.position, c.expression])).toEqual([
      ["friend", "left", "neutral"],
      ["rival", "right", "neutral"],
    ]);
    expect(start.line).toBeNull();
    const last = stageAt(["friend", "rival"], null, steps, 2);
    expect(last.cast.map((c) => [c.who, c.position, c.expression, c.speaking])).toEqual([
      ["friend", "left", "happy", true],
      ["rival", "center", "neutral", false],
    ]);
  });

  it("brings in an unlisted speaker, and gives a line without one to the sender", () => {
    const stage = stageAt(["friend"], "friend", [said(null, "Me."), said("stranger", "Who?")], 1);
    expect(stage.cast.map((c) => c.who)).toEqual(["friend", "stranger"]);
    expect(stageAt(["friend"], "friend", [said(null, "Me.")], 0).cast[0].speaking).toBe(true);
  });

  it("steps from line to line, past the replies already chosen", () => {
    const steps: PlayerStep[] = [said("a", "1"), { kind: "chose", label: "x", text: "X" }, said("a", "2")];
    expect(nextLine(steps, -1)).toBe(0);
    expect(nextLine(steps, 0)).toBe(2);
    expect(nextLine(steps, 2)).toBeNull();
    expect(previousLine(steps, 2)).toBe(0);
    expect(previousLine(steps, 0)).toBeNull();
  });

  // L3b.4: `#leave` - the speaker is on stage for that line, gone from the
  // next, and back when they speak again.
  it("takes a speaker off the stage after their leaving line", () => {
    const steps: PlayerStep[] = [
      { kind: "said", speaker: "friend", text: "Bye.", expression: null, position: null, leave: true },
      said("rival", "Gone."),
      said("friend", "Back."),
    ];
    const who = (upTo: number) => stageAt(["friend", "rival"], null, steps, upTo).cast.map((c) => c.who);
    expect(who(0)).toEqual(["friend", "rival"]);
    expect(who(1)).toEqual(["rival"]);
    expect(who(2)).toEqual(["rival", "friend"]);
  });

  // The journal pages through the replies too: a reply is the player's,
  // nobody on stage speaks it.
  it("shows a reply chosen as the player's, with nobody speaking", () => {
    const steps: PlayerStep[] = [said("a", "1"), { kind: "chose", label: "x", text: "X" }];
    const stage = stageAt(["a"], null, steps, 1);
    expect(stage.reply).toBe("X");
    expect(stage.line).toBeNull();
    expect(stage.cast.some((c) => c.speaking)).toBe(false);
    expect(stageAt(["a"], null, steps, 0).reply).toBeNull();
  });
});

describe("auto-advance", () => {
  // A longer line stays longer; a slower speed waits longer.
  it("waits longer for a longer line and a slower speed", () => {
    expect(autoDelay("a much longer line to read", "normal")).toBeGreaterThan(autoDelay("short", "normal"));
    expect(autoDelay("line", "slow")).toBeGreaterThan(autoDelay("line", "normal"));
    expect(autoDelay("line", "fast")).toBeLessThan(autoDelay("line", "normal"));
  });
});

describe("the thread", () => {
  it("names speakers only in a group, on the first bubble of a row", () => {
    const one = bubbles([said(null, "a"), said("friend", "b")], "friend", { friend: "A friend" });
    expect(one.map((b) => b.name)).toEqual([null, null]);
    const group = bubbles(
      [said("friend", "a"), said("friend", "b"), said("rival", "c"), { kind: "chose", label: "x", text: "Me" }],
      "friend",
      { friend: "A friend", rival: "A rival" },
    );
    expect(group.map((b) => [b.side, b.name])).toEqual([
      ["them", "A friend"],
      ["them", null],
      ["them", "A rival"],
      ["me", null],
    ]);
  });
});

describe("the phone's status bar", () => {
  const t = (key: string, v?: Record<string, string | number>) => `${key}:${JSON.stringify(v ?? {})}`;

  it("says the date as the story shows it", () => {
    expect(dateLabel("0003-05-01", "relative", t, "en")).toBe('story.player.yearMonth:{"year":3,"month":5}');
    expect(dateLabel("2001-12-15", "seasons", t, "en")).toBe('story.player.winter:{"year":2001}');
    expect(dateLabel("2001-03-01", "none", t, "en")).toBe("");
    expect(dateLabel("2001-03-01", "real", t, "en")).toContain("2001");
  });

  it("says when an item came, the time alone when the story shows no date", () => {
    expect(whenLabel("0003-05-01", "09:12", "relative", t, "en")).toBe(
      'story.player.yearMonth:{"year":3,"month":5} · 09:12',
    );
    expect(whenLabel("2001-03-01", "09:12", "none", t, "en")).toBe("09:12");
  });

  it("falls back on initials for an avatar", () => {
    expect(initials("A friend")).toBe("AF");
    expect(initials("rival")).toBe("RI");
  });
});
