import { describe, expect, it } from "vitest";
import type { SeenItem } from "../play";
import { appItems, appOf, appsShown, pendingIn } from "./apps";

function item(content: string, o: Partial<SeenItem> = {}): SeenItem {
  return {
    content,
    kind: "message",
    from: "friend",
    characters: [],
    date: "2001-03-01",
    clock: "09:00",
    steps: [],
    choices: [],
    open: false,
    seen: true,
    ...o,
  };
}

describe("the phone's apps", () => {
  // L3.5: a type goes to its app; a type with no app has none.
  it("map each content type to its app", () => {
    expect(appOf("mail")).toBe("mail");
    expect(appOf("article")).toBe("news");
    expect(appOf("social_post")).toBe("social");
    expect(appOf("statement")).toBe("bank");
    expect(appOf("video")).toBeNull();
    expect(appOf("dialogue")).toBeNull();
  });

  // An app appears with its first item; the messages are always there.
  it("show the messages always and the others once they have something", () => {
    expect(appsShown([])).toEqual(["messages"]);
    expect(appsShown([item("s", { kind: "statement" }), item("m", { kind: "mail" })])).toEqual([
      "messages",
      "mail",
      "bank",
    ]);
  });

  // The latest first; pending: unread, or waiting for a reply.
  it("list an app's items latest first and count what calls for the player", () => {
    const items = [
      item("a", { kind: "mail" }),
      item("b", { kind: "mail", seen: false }),
      item("c"),
      item("d", { kind: "mail", open: true }),
    ];
    expect(appItems(items, "mail")).toEqual([3, 1, 0]);
    expect(pendingIn(items, "mail")).toBe(2);
    expect(pendingIn(items, "messages")).toBe(0);
  });
});
