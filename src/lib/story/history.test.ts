import { describe, expect, it } from "vitest";
import { backupDate, historyKey } from "./history";

const press = (key: string, mods: Partial<{ ctrl: boolean; shift: boolean; alt: boolean; meta: boolean }> = {}) => ({
  key,
  ctrlKey: mods.ctrl ?? false,
  shiftKey: mods.shift ?? false,
  altKey: mods.alt ?? false,
  metaKey: mods.meta ?? false,
});

describe("history keys", () => {
  it("Ctrl+Z undoes, Ctrl+Y and Ctrl+Shift+Z redo", () => {
    expect(historyKey(press("z", { ctrl: true }), false)).toBe("undo");
    expect(historyKey(press("y", { ctrl: true }), false)).toBe("redo");
    expect(historyKey(press("Z", { ctrl: true, shift: true }), false)).toBe("redo");
  });

  // Inside a text field, Ctrl+Z undoes the typing, as everywhere else.
  it("leave a text field its own undo", () => {
    expect(historyKey(press("z", { ctrl: true }), true)).toBeNull();
  });

  it("ignore other keys and chords", () => {
    expect(historyKey(press("z"), false)).toBeNull();
    expect(historyKey(press("z", { ctrl: true, alt: true }), false)).toBeNull();
    expect(historyKey(press("s", { ctrl: true }), false)).toBeNull();
  });
});

describe("a copy's date", () => {
  it("is read from its id, in local time", () => {
    const d = backupDate("2026-10-19_08-05-09")!;
    expect([d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()]).toEqual([
      2026, 9, 19, 8, 5, 9,
    ]);
  });

  it("is null for anything else", () => {
    expect(backupDate("../x")).toBeNull();
  });
});
