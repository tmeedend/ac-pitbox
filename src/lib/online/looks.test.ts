import { describe, expect, it } from "vitest";
import { carName, carsOwnedFirst, layoutName, searchText, trackTitle, type Looks } from "./looks";
import type { ServerSummary } from "./online";

const looks: Looks = {
  cars: { ks_mazda_miata: "Mazda MX-5 NA" },
  tracks: {
    ks_nordschleife: {
      name: "Nürburgring Nordschleife",
      categories: [],
      layouts: {
        touristenfahrten: { name: "Nordschleife - Tourist", preview: null, outline: null },
      },
    },
    bathurst: {
      name: "Mount Panorama",
      categories: [],
      layouts: { "": { name: "Mount Panorama", preview: null, outline: null } },
    },
  },
};

const track = (id: string, layout: string | null) => ({ kunos_id: id, id, layout, csp_min_build: null });

describe("online looks (SPEC-play-online, Liste des serveurs)", () => {
  it("names a track by its layout, then the track, then the folder", () => {
    expect(trackTitle(looks, track("ks_nordschleife", "touristenfahrten"))).toBe("Nordschleife - Tourist");
    expect(trackTitle(looks, track("ks_nordschleife", "endurance"))).toBe("Nürburgring Nordschleife");
    expect(trackTitle(looks, track("rt_suzuka", null))).toBe("rt_suzuka");
  });

  it("reads car ids without case, and falls back on the id", () => {
    expect(carName(looks, "KS_Mazda_Miata")).toBe("Mazda MX-5 NA");
    expect(carName(looks, "rss_gtm_lanzo_v8")).toBe("rss_gtm_lanzo_v8");
  });

  it("lists the cars one has first, each group in the server's order", () => {
    expect(carsOwnedFirst(looks, ["a_car", "ks_mazda_miata", "b_car"])).toEqual(["ks_mazda_miata", "a_car", "b_car"]);
  });

  it("searches a server by the names the library knows", () => {
    const s = { name: "Aussie", track: track("bathurst", null), cars: ["ks_mazda_miata"] } as ServerSummary;
    expect(searchText(looks, s)).toContain("mount panorama");
    expect(searchText(looks, s)).toContain("mx-5");
  });
  it("reads a layout under its track's name, without repeating it", () => {
    const shuto: Looks = {
      cars: {},
      tracks: {
        shuto: {
          name: "Shutoko Revival Project 0.9.3",
          categories: [],
          layouts: { main_layout: { name: "Shutoko Revival Project 0.9.3 - Main Layout", preview: null, outline: null } },
        },
      },
    };
    expect(layoutName(shuto, track("shuto", "main_layout"))).toBe("Main Layout");
    expect(layoutName(looks, track("ks_nordschleife", "touristenfahrten")), "no repetition to drop").toBe(
      "Nordschleife - Tourist",
    );
    expect(layoutName(looks, track("bathurst", null)), "a single layout").toBeNull();
    expect(layoutName(looks, track("lac", "freeroam")), "unknown here: its folder").toBe("freeroam");
  });
});
