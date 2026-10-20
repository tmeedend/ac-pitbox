import { describe, expect, it } from "vitest";
import { setupSentences } from "./preview";

describe("the preview's setup", () => {
  it("is applied as effect sentences, the same as everywhere else", () => {
    expect(setupSentences({ "rel.friend": "warm", "flag.met": "true", money: "500" })).toEqual([
      "rel.friend = warm",
      "flag.met = true",
      "money = 500",
    ]);
  });
});
