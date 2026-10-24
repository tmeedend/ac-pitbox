import { describe, expect, it } from "vitest";
import { layoutFlow } from "./flowGraph";
import type { FlowNode } from "./studio";

// A chapter shaped like the sample's first: a question, a branch whose
// first case jumps ahead and whose other way asks again, both joining on the
// race; then a garage visited until a counter says enough, and the next
// chapter.
const chapter: FlowNode[] = [
  { id: "intro", beat: {} },
  { id: "invite", message: "invite_01" },
  { id: "answer", branch: [{ if: ["content.invite_01.reply == accept"], goto: "race" }], otherwise: "ask_again" },
  { id: "ask_again", message: "invite_02" },
  { id: "race", event: "race_01" },
  { id: "garage", action: "visit" },
  { id: "enough", branch: [{ if: ["counter.visits >= 2"], goto: "chapter_2" }], otherwise: "garage" },
];

describe("the graph of a chapter", () => {
  const g = layoutFlow(chapter, "chapter_2");
  const box = (id: string) => g.boxes.find((b) => b.id === id)!;

  // L3b.5: a node sits below everything leading to it; a branch's two ways
  // stand side by side until they join.
  it("puts a node below what leads to it, and a branch's ways side by side", () => {
    expect(box("invite").row).toBe(1);
    expect(box("ask_again").row).toBe(3);
    expect(box("race").row, "below both ways in").toBe(4);
    const jump = g.edges.find((e) => e.from === "answer" && e.to === "race")!;
    expect(jump.points).toHaveLength(3);
    expect(jump.points[1].x, "the jump goes beside the other way, not through it").not.toBe(
      box("ask_again").x + 84,
    );
  });

  // A loop is a way back up, and does not push the rows down.
  it("draws a loop as a way back, beside the boxes", () => {
    const loop = g.edges.find((e) => e.from === "enough" && e.to === "garage")!;
    expect(loop.back).toBe(true);
    expect(box("enough").row).toBe(box("garage").row + 1);
  });

  // A way out of the chapter ends on its own box, named after the node it
  // goes to.
  it("ends a way out of the chapter on a box of its own", () => {
    const exit = g.boxes.find((b) => b.kind === "exit")!;
    expect(exit.id).toBe("chapter_2");
    expect(g.edges.some((e) => e.to === "exit:chapter_2" && e.role === "case")).toBe(true);
    expect(g.width).toBeGreaterThan(0);
  });

  // A chain stays straight: one column for nodes that only follow each
  // other; an empty target or a twice-written id draws nothing more.
  it("keeps a chain in one column, and draws nothing for what is not there", () => {
    const line = layoutFlow([
      { id: "a", beat: {} },
      { id: "b", action: "x" },
      { id: "b", action: "y" },
      { id: "c", branch: [], otherwise: "" },
    ]);
    expect(new Set(line.boxes.map((b) => b.x)).size).toBe(1);
    expect(line.boxes.map((b) => b.id)).toEqual(["a", "b", "c"]);
  });
});
