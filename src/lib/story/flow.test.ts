import { describe, expect, it } from "vitest";
import type { Chapter } from "./studio";
import {
  freshChapterId,
  freshNodeId,
  insertNode,
  isNodeId,
  kindOf,
  linksOf,
  moveNode,
  newNode,
  NODE_KINDS,
  nextChapterFile,
  parseLines,
  renameNode,
} from "./flow";

const chapter = (): Chapter => ({
  id: "ch01",
  nodes: [
    { id: "start", beat: {} },
    { id: "race", event: "race_01", on_failure: "again" },
    { id: "pick", branch: [{ if: ["money >= 100"], goto: "race" }], otherwise: "again" },
    { id: "again", beat: {}, next: "race" },
  ],
});

describe("flow nodes", () => {
  // Every node the editor creates has its kind's keys: the engine reads one
  // key per kind, and `kindOf` must find it again.
  it("a new node of each kind reads back as that kind", () => {
    for (const kind of NODE_KINDS) {
      expect(kindOf(newNode(kind, "x")), kind).toBe(kind);
    }
  });

  it("a fresh id is the first one free in the whole story", () => {
    expect(freshNodeId("beat", ["beat_1", "beat_2", "event_3"])).toBe("beat_3");
    expect(freshNodeId("event", [])).toBe("event_1");
  });

  it("links are the arrows a node draws, the plain order excepted", () => {
    const [, race, pick, again] = chapter().nodes;
    expect(linksOf(race)).toEqual([{ role: "failure", target: "again" }]);
    expect(linksOf(pick).map((l) => l.role)).toEqual(["case", "otherwise"]);
    expect(linksOf(again)).toEqual([{ role: "next", target: "race" }]);
  });

  it("inserting goes after the selected node, or at the end", () => {
    const nodes = chapter().nodes;
    const [after, at] = insertNode(nodes, 0, { id: "n", beat: {} });
    expect(after.map((n) => n.id)).toEqual(["start", "n", "race", "pick", "again"]);
    expect(at).toBe(1);
    const [end, last] = insertNode(nodes, null, { id: "n", beat: {} });
    expect(end.at(-1)?.id).toBe("n");
    expect(last).toBe(4);
  });

  it("moving stops at the ends of the chapter", () => {
    const nodes = chapter().nodes;
    expect(moveNode(nodes, 0, -1)).toBe(nodes);
    expect(moveNode(nodes, 0, 1).map((n) => n.id)).toEqual(["race", "start", "pick", "again"]);
  });

  // A rename that left a link behind would break the story silently far from
  // the node renamed: every link follows, in every chapter.
  it("renaming a node moves every link to it, across chapters", () => {
    const one = chapter();
    const two: Chapter = { id: "ch02", nodes: [{ id: "later", beat: {}, next: "again" }] };
    const three: Chapter = { id: "ch03", nodes: [{ id: "alone", end: "end" }] };
    expect(renameNode([one, two, three, null], "again", "retry")).toEqual([0, 1]);
    expect(one.nodes[3].id).toBe("retry");
    expect(one.nodes[1].on_failure).toBe("retry");
    expect(one.nodes[2].otherwise).toBe("retry");
    expect(two.nodes[0].next).toBe("retry");
    expect("next" in one.nodes[0], "no key created where there was none").toBe(false);
  });

  it("node ids keep to letters, digits and underscores", () => {
    expect(isNodeId("race_01")).toBe(true);
    expect(isNodeId("")).toBe(false);
    expect(isNodeId("a b")).toBe(false);
    expect(isNodeId("a.b")).toBe(false);
  });
});

describe("chapters", () => {
  // Chapters play in file-name order, so a new one sorts after the others.
  it("a new chapter takes the next number", () => {
    expect(nextChapterFile([])).toEqual({ file: "flow/01-chapter.yaml", number: 1 });
    expect(nextChapterFile(["flow/01-first-race.yaml", "flow/09-x.yaml", "flow/notes.yaml"])).toEqual({
      file: "flow/10-chapter.yaml",
      number: 10,
    });
  });

  it("a new chapter's id is free in the story", () => {
    expect(freshChapterId(3, [])).toBe("ch03");
    expect(freshChapterId(2, [{ id: "ch02", nodes: [] }, null, { id: "ch03", nodes: [] }])).toBe("ch04");
  });

  it("a sentence list is one sentence per line, blanks dropped", () => {
    expect(parseLines(" money += 5 \n\n rel.friend += 1\n")).toEqual(["money += 5", "rel.friend += 1"]);
  });
});
