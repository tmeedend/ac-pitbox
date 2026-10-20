import { describe, expect, it } from "vitest";
import { allLabels, conditionsText, freshLabel, newItem, parseConditions } from "./script";
import type { ScriptItem } from "./studio";

const tree: ScriptItem[] = [
  { kind: "line", speaker: "friend", text: "Coming?" },
  {
    kind: "choices",
    choices: [
      { label: "reply_1", text: "Yes.", body: [{ kind: "choices", choices: [{ label: "sure", text: "Sure.", body: [] }] }] },
      { label: "no", text: "No.", body: [] },
    ],
  },
];

describe("labels", () => {
  it("are collected from every choice point, nested ones included", () => {
    expect(allLabels(tree)).toEqual(["reply_1", "sure", "no"]);
  });

  // A label is unique in the whole conversation, not only its choice point.
  it("a new choice gets the first label free in the conversation", () => {
    expect(freshLabel(tree)).toBe("reply_2");
    const added = newItem("choices", tree);
    expect(added.kind === "choices" && added.choices[0].label).toBe("reply_2");
  });
});

describe("conditions", () => {
  it("read and write as sentences joined by `and`", () => {
    expect(parseConditions("rel.friend >= warm and not flag.met")).toEqual(["rel.friend >= warm", "not flag.met"]);
    expect(parseConditions("  ")).toEqual([]);
    expect(conditionsText(["a", "b"])).toBe("a and b");
    expect(conditionsText(undefined)).toBe("");
  });
});
