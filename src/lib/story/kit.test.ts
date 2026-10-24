import { describe, expect, it } from "vitest";
import { besideStory, insideStory, kitFolder } from "./kit";

describe("where the starter kit goes", () => {
  // Beside the story, in a folder of its own, never inside the story: it is
  // a drawing aid, not part of what the story carries.
  it("opens beside the story and keeps out of it", () => {
    expect(besideStory("D:\\Stories\\My story")).toBe("D:\\Stories");
    expect(besideStory("D:/Stories/My story/")).toBe("D:/Stories");
    expect(kitFolder("D:\\Art")).toBe("D:\\Art\\starter-kit");
    expect(kitFolder("/home/me/art/")).toBe("/home/me/art/starter-kit");
    expect(insideStory("D:\\Stories\\My story", "D:/stories/my story")).toBe(true);
    expect(insideStory("D:\\Stories\\My story\\media", "D:\\Stories\\My story")).toBe(true);
    expect(insideStory("D:\\Stories\\My story 2", "D:\\Stories\\My story")).toBe(false);
    expect(insideStory("D:\\Stories", "D:\\Stories\\My story")).toBe(false);
  });
});
