import { describe, expect, it } from "vitest";
import { defaultPrefs, moveColumn, reconcilePrefs, toggleVisible, visibleColumns, type TableColumn } from "./tableColumns";

const col = (key: string, over: Partial<TableColumn> = {}): TableColumn => ({
  key,
  labelKey: key,
  sortable: true,
  defaultVisible: true,
  ...over,
});

const defs = [col("track", { fixed: true, locked: true }), col("name"), col("players"), col("ip", { defaultVisible: false })];

describe("table columns (SPEC §7.4)", () => {
  it("starts from the visible-by-default columns, in definition order", () => {
    expect(defaultPrefs(defs)).toEqual({ visible: ["track", "name", "players"], order: ["track", "name", "players", "ip"], widths: {} });
  });

  // A file written by an older or newer version must never break nor freeze.
  it("drops a removed column and appends a new one when reading saved prefs", () => {
    const saved = { visible: ["name", "gone"], order: ["players", "gone", "name"], widths: { gone: 80, name: 200 } };
    expect(reconcilePrefs(saved, defs)).toEqual({
      visible: ["name"],
      order: ["track", "players", "name", "ip"],
      widths: { name: 200 },
    });
  });

  it("keeps a locked column in its place, whatever was saved", () => {
    expect(reconcilePrefs({ order: ["name", "players", "track", "ip"] }, defs).order[0]).toBe("track");
  });

  it("moves a column before or after another, never a locked one nor before it", () => {
    const order = defaultPrefs(defs).order;
    expect(moveColumn(order, defs, "players", "name", true)).toEqual(["track", "players", "name", "ip"]);
    expect(moveColumn(order, defs, "name", "ip", false)).toEqual(["track", "players", "ip", "name"]);
    expect(moveColumn(order, defs, "track", "ip", false), "locked column stays").toBe(order);
    expect(moveColumn(order, defs, "ip", "track", true), "nothing goes before it").toBe(order);
  });

  it("shows fixed columns even when not listed, in the saved order", () => {
    const prefs = toggleVisible({ visible: ["players"], order: ["track", "players", "name", "ip"], widths: {} }, "ip");
    expect(visibleColumns(defs, prefs).map((c) => c.key)).toEqual(["track", "players", "ip"]);
  });
});
