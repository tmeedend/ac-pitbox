import { describe, expect, it } from "vitest";
import { siteHost, webSearchUrl } from "./webSearch";

describe("the web search opened when no address is known", () => {
  it("searches the words as they are, without quotes", () => {
    expect(webSearchUrl("RSS_GTM_Lanzo_V10_v1.4.7z")).toBe("https://duckduckgo.com/?q=RSS_GTM_Lanzo_V10_v1.4.7z");
  });

  it("restricts to the site the archive came from", () => {
    expect(webSearchUrl("camtool-v3.zip", "https://www.overtake.gg/")).toBe(
      "https://duckduckgo.com/?q=site%3Aovertake.gg%20camtool-v3.zip",
    );
  });

  it("keeps a site that is not an address as it came", () => {
    expect(siteHost("overtake.gg")).toBe("overtake.gg");
  });
});
