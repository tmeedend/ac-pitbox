// The phone's apps, pure (SPEC-story-mode, Content types, templates and
// frames): which app shows which content type, and what each holds. Content
// is written by type, never by app; the phone frame is what maps a type to
// one of its apps. A type with no app yet (a call, a video, a listing, a
// calendar entry) is listed by name in the messages.
import type { SeenItem } from "../play";
import type { ContentType } from "../studio";

export type PhoneApp = "messages" | "mail" | "news" | "social" | "bank";

/** The apps in the order the home screen shows them. */
export const PHONE_APPS: PhoneApp[] = ["messages", "mail", "news", "social", "bank"];

/** i18n keys written out in full, for the key check. */
export const APP_LABEL: Record<PhoneApp, string> = {
  messages: "story.play.messages",
  mail: "story.play.appMail",
  news: "story.play.appNews",
  social: "story.play.appSocial",
  bank: "story.play.appBank",
};

const APP_OF: Partial<Record<ContentType, PhoneApp>> = {
  message: "messages",
  mail: "mail",
  article: "news",
  social_post: "social",
  statement: "bank",
};

/** The app showing content of `kind`, if the phone has one for it. */
export function appOf(kind: ContentType): PhoneApp | null {
  return APP_OF[kind] ?? null;
}

/** The items app `app` shows, latest first, as indexes into `items`. */
export function appItems(items: SeenItem[], app: PhoneApp): number[] {
  const out: number[] = [];
  for (let i = items.length - 1; i >= 0; i--) if (appOf(items[i].kind) === app) out.push(i);
  return out;
}

/** How many of an app's items call for the player: not opened yet, or
 * waiting for a reply. */
export function pendingIn(items: SeenItem[], app: PhoneApp): number {
  return appItems(items, app).filter((i) => !items[i].seen || items[i].open).length;
}

/** The apps the home screen shows: the messages always, the others once
 * they have something - an app appears with its first item. */
export function appsShown(items: SeenItem[]): PhoneApp[] {
  return PHONE_APPS.filter((app) => app === "messages" || appItems(items, app).length > 0);
}
