import { describe, expect, it } from "vitest";
import { setOptional } from "./fields";

describe("an optional field", () => {
  it("writes the trimmed text, and nothing at all when blank", () => {
    const target: { clock?: string } = {};
    setOptional(target, "clock", " 09:00 ");
    expect(target.clock).toBe("09:00");
    setOptional(target, "clock", "  ");
    expect("clock" in target, "the key goes").toBe(false);
  });
});
