// The phone's apps, pure (SPEC-story-mode, Content types, templates and
// frames): which app shows which content type, and what each holds. Content
// is written by type, never by app; the frame up is what maps a type to one
// of its apps - the engine's list (`PlayView.apps`), the story's own phones'
// or the one Pit Box ships. A type the frame has no app for is listed by
// name in its messages (the shipped phone shows every type).
import type { AppShown, SeenItem } from "../play";

/** i18n keys written out in full, for the key check: the names of the apps
 * of the phone Pit Box ships, by id (`story_engine::frame::phone_apps`). */
const SHIPPED_LABEL: Record<string, string> = {
  messages: "story.play.messages",
  mail: "story.play.appMail",
  news: "story.play.appNews",
  social: "story.play.appSocial",
  bank: "story.play.appBank",
  calls: "story.play.appCalls",
  calendar: "story.play.appCalendar",
};

/** A name for an app the story leaves unnamed, by its template. */
const TEMPLATE_LABEL: Record<AppShown["template"], string> = {
  thread: "story.play.messages",
  inbox: "story.play.appMail",
  feed: "story.play.appNews",
  statement: "story.play.appBank",
  agenda: "story.play.appCalendar",
  call: "story.play.appCalls",
};

/** An app's name: the story's, else Pit Box's for that app or template. */
export function appName(app: AppShown, t: (key: string) => string): string {
  return app.name ?? t(SHIPPED_LABEL[app.id] ?? TEMPLATE_LABEL[app.template]);
}

/** The app showing content of `kind`, if the frame has one for it. */
export function appOf(apps: AppShown[], kind: string): AppShown | null {
  return apps.find((a) => a.shows.includes(kind)) ?? null;
}

/** The items app `app` shows, latest first, as indexes into `items`: those
 * of its types that did not play on the scene - a call taken on a landline
 * stays out of the phone's call log. */
export function appItems(items: SeenItem[], app: AppShown): number[] {
  const out: number[] = [];
  for (let i = items.length - 1; i >= 0; i--) {
    if (app.shows.includes(items[i].kind) && items[i].staged_as === null) out.push(i);
  }
  return out;
}

/** How many of an app's items call for the player: not opened yet, or
 * waiting for a reply. */
export function pendingIn(items: SeenItem[], app: AppShown): number {
  return appItems(items, app).filter((i) => !items[i].seen || items[i].open).length;
}

/** The apps the home screen shows: those the engine says are there - always,
 * unlocked by a Beat, or with an item. */
export function appsShown(apps: AppShown[]): AppShown[] {
  return apps.filter((a) => a.available);
}

/** The app the phone opens on, and comes back to when an event waits: the
 * first messaging app, which the next event heads; `null` without one. */
export function homeApp(apps: AppShown[]): AppShown | null {
  return apps.find((a) => a.template === "thread") ?? null;
}
