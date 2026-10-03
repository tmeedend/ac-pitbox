import { describe, expect, it } from "vitest";
import { parseJoinLink } from "./link";

describe("pasted connection links (SPEC-play-online, case 1)", () => {
  it("reads CM's share link and its protocol form", () => {
    const expected = { ip: "87.106.245.215", httpPort: 8083, password: null };
    expect(parseJoinLink("https://acstuff.club/s/q:race/online/join?ip=87.106.245.215&httpPort=8083")).toEqual(
      expected,
    );
    expect(parseJoinLink("  acmanager://race/online/join?ip=87.106.245.215&httpPort=8083\n")).toEqual(expected);
  });

  it("keeps a plain password, and refuses what is not a link", () => {
    expect(parseJoinLink("acmanager://race/online?ip=1.2.3.4&httpPort=8081&plainPassword=abc")?.password).toBe("abc");
    expect(parseJoinLink("just some text")).toBeNull();
    expect(parseJoinLink("acmanager://race/online/join?ip=1.2.3.4&httpPort=99999")).toBeNull();
  });
});
