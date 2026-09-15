import { describe, it, expect } from "vitest";
import { layerDisplayName } from "./layerName";

// A layer's derived name (REFONTE§8.3). Derived, therefore fallible, therefore
// correctable by hand — but the derivation still has to be conservative: every
// extra rule is a chance to drop a word that mattered.
describe("layerDisplayName", () => {
  it("drops the archive extension and the underscores", () => {
    expect(layerDisplayName("spa2022-release_V1-03.rar", null)).toBe("spa2022-release V1-03");
  });

  it("drops the host prefix when the name repeats it", () => {
    expect(layerDisplayName("ks_nordschleife_touristenfahrten.7z", "ks_nordschleife")).toBe("touristenfahrten");
  });

  it("never cuts inside a word", () => {
    // "ks_nords" is not the host: the name keeps everything.
    expect(layerDisplayName("ks_nordschleife_extra.zip", "ks_nords")).toBe("ks nordschleife extra");
  });

  it("keeps the whole name when removing the host would leave nothing", () => {
    expect(layerDisplayName("Spa.rar", "Spa")).toBe("Spa");
  });

  it("leaves a name that has nothing to remove alone", () => {
    expect(layerDisplayName("Cameras CamTool v3", "Nordschleife")).toBe("Cameras CamTool v3");
  });

  it("does not mistake a version number for an extension", () => {
    expect(layerDisplayName("pack_v1.2", null)).toBe("pack v1.2");
  });

  it("falls back on the source when nothing readable is left", () => {
    expect(layerDisplayName(".rar", null)).toBe(".rar");
    expect(layerDisplayName("   ", null)).toBe("   ");
  });
});
