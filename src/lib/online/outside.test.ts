import { describe, expect, it } from "vitest";
import type { Fetch } from "./online";
import { launcherGate, unsourced, webSearchUrl } from "./outside";

const fetch = (over: Partial<Fetch> = {}): Fetch => ({
  kept_archive: false,
  server_url: null,
  cup: false,
  server_version: null,
  installed_version: null,
  needed: false,
  ...over,
});

describe("what Pit Box cannot fetch or join (SPEC-play-online, Contenu manquant)", () => {
  it("searches the web generally, by the mod's id", () => {
    expect(webSearchUrl("simtraxx_tf_dn7c_100km_ls-hc")).toBe(
      "https://duckduckgo.com/?q=simtraxx_tf_dn7c_100km_ls-hc%20assetto%20corsa",
    );
  });

  it("keeps only the content to download that no source covers", () => {
    const items = [
      { id: "none", name: "None", level: "download" as const, fetch: fetch() },
      { id: "link", name: "Link", level: "download" as const, fetch: fetch({ server_url: "https://x/y.zip" }) },
      { id: "cup", name: "Cup", level: "download" as const, fetch: fetch({ cup: true }) },
      { id: "kept", name: "Kept", level: "download" as const, fetch: fetch({ kept_archive: true }) },
      { id: "dlc", name: "DLC", level: "blocked" as const, fetch: fetch() },
      { id: "here", name: "Here", level: "ready" as const, fetch: fetch() },
    ];
    expect(unsourced(items).map((w) => w.id)).toEqual(["none"]);
  });

  // Measured on the lobby: only the official servers link to nohesi.gg.
  it("knows No Hesi's servers by the link to their site, not by their name", () => {
    expect(launcherGate(["discord.gg/nohesitation", "https://www.nohesi.gg/subscriptions"])?.name).toBe("No Hesi");
    expect(launcherGate(["https://nohesi.gg"])?.name).toBe("No Hesi");
    expect(launcherGate(["https://discord.gg/nohesi"]), "a Discord invite is anyone's").toBeNull();
    expect(launcherGate(["https://notnohesi.gg.example.com/x"])).toBeNull();
    expect(launcherGate([])).toBeNull();
  });
});
