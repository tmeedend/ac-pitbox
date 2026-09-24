// Where Pit Box lives outside the application: the About screen, the
// controller report and the Rules screen all send the user there. One place,
// so a moved repository or profile is changed once.
import { openUrl } from "@tauri-apps/plugin-opener";

export const SOURCE_URL = "https://github.com/tmeedend/ac-pitbox";
export const ISSUE_URL = `${SOURCE_URL}/issues/new`;
export const CHANGELOG_URL = `${SOURCE_URL}/commits/main`;
export const OVERTAKE_URL = "https://www.overtake.gg/members/ktulu77.1266672/";
export const DISCORD_URL = "https://discord.gg/hgWTC2s49M";
export const DONATE_URL = "https://paypal.me/ktulu77";

/** A new GitHub issue, pre-filled. What goes in it is shown to the user in
 * the browser before anything is sent - nothing leaves without his click. */
export function newIssueUrl(title: string, body: string): string {
  return `${ISSUE_URL}?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}`;
}

/** Opens a link in the system browser. A link that fails to open is not an
 * error worth a message: logged, the button stays. */
export function openExternal(url: string): void {
  openUrl(url).catch((e) => console.warn("open url", url, e));
}
