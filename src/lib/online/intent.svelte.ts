// A way into the Online page from elsewhere: a track sheet opens it on its
// track (SPEC-play-online.md, v2), a notification on a server. The page takes
// what was asked as soon as it can — when it mounts, or at once if it is
// already on screen — and the request is spent once taken.
import { requestSection } from "$lib/shell/nav.svelte";
import type { ServerSummary } from "./online";

export type OnlineIntent =
  /** Track token values (`trackValue`), posed on the All tab. */
  | { kind: "track"; layouts: string[] }
  /** A server to open in the panel. */
  | { kind: "server"; server: ServerSummary };

let pending = $state.raw<OnlineIntent | null>(null);

/** Opens the Online page on `intent`. Posed only once the page is reached:
 * a navigation refused (unsaved settings) must not leave it waiting for the
 * next visit. The page watches it, so arriving after its mount is fine. */
export async function openOnline(intent: OnlineIntent): Promise<void> {
  if (await requestSection("online")) pending = intent;
}

/** The request waiting, if any. Reactive: the page watches it. */
export function pendingOnlineIntent(): OnlineIntent | null {
  return pending;
}

/** What the page was opened for, once. */
export function takeOnlineIntent(): OnlineIntent | null {
  const intent = pending;
  pending = null;
  return intent;
}
