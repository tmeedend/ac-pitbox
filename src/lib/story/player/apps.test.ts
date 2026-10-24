import { describe, expect, it } from "vitest";
import type { AppShown, SeenItem } from "../play";
import { appItems, appName, appOf, appsShown, homeApp, pendingIn } from "./apps";

function item(content: string, o: Partial<SeenItem> = {}): SeenItem {
  return {
    content,
    kind: "message",
    from: "friend",
    characters: [],
    picture: null,
    transition: "fade",
    scene: null,
    // A dialogue plays on the scene, as the engine says of it.
    staged_as: o.kind === "dialogue" ? "dialogue" : null,
    skin: null,
    date: "2001-03-01",
    clock: "09:00",
    steps: [],
    choices: [],
    open: false,
    seen: true,
    ...o,
  };
}

function app(id: string, template: AppShown["template"], shows: string[], o: Partial<AppShown> = {}): AppShown {
  return { id, name: null, icon: null, template, shows, available: true, skin: null, ...o };
}

const phone = [
  app("messages", "thread", ["message"]),
  app("mail", "inbox", ["mail"]),
  app("paper", "feed", ["article"], { name: "The Daily", available: false }),
];

describe("the phone's apps", () => {
  // A type goes to the app of the frame showing it; a type no app shows
  // has none, and a dialogue is the scene's.
  it("map each content type to the frame's app for it", () => {
    expect(appOf(phone, "mail")?.id).toBe("mail");
    expect(appOf(phone, "article")?.id).toBe("paper");
    expect(appOf(phone, "statement")).toBeNull();
    expect(appOf(phone, "dialogue")).toBeNull();
    expect(appOf([], "message"), "no frame, no app").toBeNull();
  });

  // The home screen shows what the engine says is there.
  it("show the apps the engine says are available", () => {
    expect(appsShown(phone).map((a) => a.id)).toEqual(["messages", "mail"]);
  });

  // The story's name first, then Pit Box's for its shipped apps, then the
  // template's.
  it("name an app as the story does, else as Pit Box does", () => {
    const t = (k: string) => `<${k}>`;
    expect(appName(phone[2], t)).toBe("The Daily");
    expect(appName(phone[1], t)).toBe("<story.play.appMail>");
    expect(appName(app("chat", "thread", ["message"]), t)).toBe("<story.play.messages>");
  });

  // The phone opens on its first messaging app; a phone without one opens
  // on its home screen.
  it("open on the first messaging app", () => {
    expect(homeApp(phone)?.id).toBe("messages");
    expect(homeApp([phone[1]])).toBeNull();
  });

  // The latest first; pending: unread, or waiting for a reply.
  it("list an app's items latest first and count what calls for the player", () => {
    const items = [
      item("a", { kind: "mail" }),
      item("b", { kind: "mail", seen: false }),
      item("c"),
      item("d", { kind: "mail", open: true }),
    ];
    expect(appItems(items, phone[1])).toEqual([3, 1, 0]);
    expect(pendingIn(items, phone[1])).toBe(2);
    expect(pendingIn(items, phone[0])).toBe(0);
  });
});

describe("appItems on the scene", () => {
  // A call taken on the landline stays out of a later phone's call log.
  it("leaves out what played on the scene", () => {
    const calls = app("calls", "call", ["call"]);
    const items = [item("a", { kind: "call", staged_as: "landline" }), item("b", { kind: "call" })];
    expect(appItems(items, calls)).toEqual([1]);
  });
});
