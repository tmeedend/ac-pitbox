import { describe, expect, it } from "vitest";
import { passwordMatches, type PasswordCheck } from "./password";

// A real server (CM's wrapper, 2026-10-03), its name in Cyrillic: the second
// checksum is the one of its empty admin password, sha1("apatosaur" + name).
const real: PasswordCheck = {
  salt: "GL-Team - Сервер",
  checksums: ["dbe24a8795b95c3559aa8a4caba5ea79da223078", "a6d90d3afadc8649ca7a6924e70a73992b43ed2f"],
};

describe("password checked before launch (SPEC-play-online, level 3)", () => {
  it("follows the wrapper's recipe, the name salted in UTF-8", async () => {
    // The admin's empty password, salted with the name: what proves the
    // recipe on a real server. Checked through a non-empty spelling of the
    // same input, since an empty password is refused on purpose.
    const { salt, checksums } = real;
    expect(await passwordMatches({ salt: "", checksums }, salt)).toBe(true);
  });

  it("refuses a wrong password, and an empty one", async () => {
    expect(await passwordMatches(real, "wrong")).toBe(false);
    expect(await passwordMatches(real, ""), "the empty admin checksum must not let it pass").toBe(false);
  });
});
