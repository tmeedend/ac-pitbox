// Back/forward history for the app's screens (§7.2bis), driven by the two
// side buttons of the mouse.
//
// What a "screen" is: the section shown by `AppShell` plus the sheets stacked
// on top of it (`openFull`, then `openPack`). That triple is the whole visible
// address of the app — everything else in `nav` is either a transient request
// consumed by the screen it was meant for (`openMod`, `search`, `autoLaunch`,
// `settingsTab`) or state that belongs to a screen rather than naming one.
//
// **Screens are observed, not declared.** A dozen places set `nav.openFull` or
// call `requestSection` — a card's double-click, the context menu, the gamepad,
// a transversal view, the sidebar. Asking each of them to also push a history
// entry would mean the history is wrong the day someone adds a thirteenth. So
// the history watches the triple change (the effect lives in `AppShell`) and
// records what it sees, which no new caller can forget to do.
import { tick } from "svelte";
import { nav, requestSection } from "./nav.svelte";

export interface Screen {
  section: string;
  openFull: string | null;
  openPack: string | null;
}

/** Enough to walk back through a browsing session, small enough that the list
 * is never worth thinking about. Never persisted: an app reopened tomorrow has
 * no "previous screen", the same way a fresh browser tab has no back button. */
const MAX_ENTRIES = 60;

let entries: Screen[] = [];
let index = -1;
/** Raised while a back/forward is being applied, so the screens it walks
 * through are not recorded as new ones. It has to stay up until the observer
 * effect has run (hence the `tick()` in `apply`): `requestSection` clears the
 * open sheets before we restore them, and that intermediate state can be
 * flushed to the effect before we are done — recording it would push a screen
 * nobody ever saw and truncate the forward history. */
let applying = false;

const same = (a: Screen, b: Screen) =>
  a.section === b.section && a.openFull === b.openFull && a.openPack === b.openPack;

/** Notes the screen currently on display. Called from an effect watching the
 * triple, so it fires on every arrival whatever the path taken. */
export function recordScreen(screen: Screen): void {
  if (applying) return;
  const at = entries[index];
  if (at && same(at, screen)) return;
  // Anything ahead of the cursor is dropped, as a browser does: navigating
  // somewhere new after going back rewrites that future.
  entries = entries.slice(0, index + 1);
  entries.push(screen);
  if (entries.length > MAX_ENTRIES) entries.shift();
  index = entries.length - 1;
}

/** Puts a recorded screen back on display. Returns false when the move was
 * refused, which only the unsaved-changes guard of §10bis can do. */
async function apply(target: Screen): Promise<boolean> {
  applying = true;
  try {
    if (target.section !== nav.section) {
      // Through `requestSection` and not `nav.section` directly: leaving a
      // screen with unsaved changes must still offer to save, whether the user
      // left it by clicking the sidebar or by pressing the back button.
      if (!(await requestSection(target.section))) return false;
    }
    nav.openFull = target.openFull;
    nav.openPack = target.openPack;
    await tick();
    return true;
  } finally {
    applying = false;
  }
}

/**
 * The cursor moves BEFORE the screen does, and rolls back if the guard says
 * no. The other order looks more natural and is wrong: the observer effect can
 * run before an `await` gives us the chance to move the cursor, and it would
 * then compare the restored screen against the entry we are leaving, decide it
 * is new, and truncate the very future we are walking into.
 */
export async function goBack(): Promise<void> {
  if (index <= 0) return;
  const from = index;
  index -= 1;
  if (!(await apply(entries[index]))) index = from;
}

export async function goForward(): Promise<void> {
  if (index >= entries.length - 1) return;
  const from = index;
  index += 1;
  if (!(await apply(entries[index]))) index = from;
}
