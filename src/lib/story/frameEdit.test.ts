import { describe, expect, it } from "vitest";
import type { EditedFile, FrameDecl, SkinFile, TemplateInfo } from "./studio";
import {
  appShownOf,
  filesOfSkin,
  sampleItem,
  sampleKind,
  lookOf,
  skinShownOf,
  newApp,
  newFrame,
  setColor,
  setSlice,
  setSlot,
  setTemplate,
  skinFile,
  skinIdOf,
  skinsFor,
  typesFor,
} from "./frameEdit";

const T = (name: string, shows: string[], app = true): TemplateInfo => ({ name, app, shows, slots: [] });
const thread = T("thread", ["message"]);
const feed = T("feed", ["article", "social_post"]);
const inbox = T("inbox", ["mail"]);

describe("skins", () => {
  it("are named by their folder, and offered to the template they dress", () => {
    expect(skinIdOf(skinFile("chat"))).toBe("chat");
    expect(skinIdOf("frames/frames.yaml")).toBeNull();
    const skins: EditedFile<SkinFile>[] = [
      { file: skinFile("chat"), stamp: "", value: { template: "thread" }, error: null },
      { file: skinFile("paper"), stamp: "", value: { template: "newspaper" }, error: null },
      { file: skinFile("broken"), stamp: "", value: null, error: "x" },
    ];
    expect(skinsFor(skins, "thread")).toEqual(["chat"]);
    expect(filesOfSkin(["frames/skins/chat/a.png", "frames/skins/chatty/b.png", "media/c.png"], "chat")).toEqual([
      "a.png",
    ]);
  });

  // A slot, a colour or margins left out keep Pit Box's: nothing empty is
  // written.
  it("leave out what is emptied", () => {
    const skin: SkinFile = { template: "thread" };
    setSlot(skin, "bubble_in", "in.png");
    setSlice(skin, "bubble_in", [40, 30, 40, 30]);
    expect(skin.slots).toEqual({ bubble_in: { file: "in.png", slice: [40, 30, 40, 30] } });
    setSlice(skin, "bubble_in", [0, 0, 0, 0]);
    expect(skin.slots?.bubble_in.slice, "no margins: stretched whole").toBeUndefined();
    setSlot(skin, "bubble_in", "");
    expect(skin.slots).toBeUndefined();
    setColor(skin, "accent", "#ff0000");
    setColor(skin, "accent", "");
    expect(skin.colors).toBeUndefined();
  });
});

describe("apps", () => {
  // A type goes to one app of a frame: a new app takes what its template
  // shows and no other app does.
  it("take the types no other app of the frame shows", () => {
    const frame: FrameDecl = newFrame("old", "Old phone");
    frame.apps.push({ id: "news", template: "feed", shows: ["article"] });
    expect(newApp(frame, "social", feed).shows).toEqual(["social_post"]);
    expect(typesFor(frame, frame.apps[1], [thread, feed])).toEqual(["article", "social_post"]);
  });

  // Another template keeps the types it can show, else takes all of them,
  // and drops a skin made for the old one.
  it("change template, keeping what still fits", () => {
    const app = { id: "x", template: "thread", shows: ["message"], skin: "chat" };
    setTemplate(app, inbox);
    expect(app).toEqual({ id: "x", template: "inbox", shows: ["mail"] });
  });
});

describe("the preview", () => {
  it("draws a skin with its files inside its folder", () => {
    const shown = skinShownOf("chat", { template: "thread", font: "f.ttf", slots: { reply: { file: "r.png" } } });
    expect(shown.font).toBe("frames/skins/chat/f.ttf");
    expect(shown.slots.reply).toEqual({ file: "frames/skins/chat/r.png", slice: null });
    expect(lookOf(newFrame("p", "P")).status_bar, "a full status bar when not said").toBe("full");
  });
});

describe("the preview's samples", () => {
  // An app wears its skin only when the story has one for its template.
  it("dress an app in its skin, and fill it with a sample", () => {
    const skins: EditedFile<SkinFile>[] = [
      { file: skinFile("chat"), stamp: "", value: { template: "thread", colors: { accent: "#123456" } }, error: null },
    ];
    const shown = appShownOf({ id: "m", template: "thread", shows: ["message"], skin: "chat" }, skins);
    expect(shown.skin?.colors.accent).toBe("#123456");
    expect(appShownOf({ id: "i", template: "inbox", shows: ["mail"], skin: "chat" }, skins).skin, "another template").toBeNull();
    const item = sampleItem("message", ["Hi.", "Coming?"], "Yes.");
    expect([item.steps.length, item.open, item.choices[0].text]).toEqual([2, true, "Yes."]);
    expect(sampleKind(feed)).toBe("article");
  });
});
