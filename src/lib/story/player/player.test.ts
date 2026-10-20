import { describe, expect, it } from "vitest";
import { dateLabel, initials } from "./phone";
import { nextLine, stageAt } from "./stage";
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

  it("falls back on initials for an avatar", () => {
    expect(initials("A friend")).toBe("AF");
    expect(initials("rival")).toBe("RI");
  });
});
