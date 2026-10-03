// A connection link, pasted on the Online page or copied from it to a friend
// (SPEC-play-online.md, case 1: "Coller un lien de connexion … ouvre
// directement le détail du serveur"). Pure, for Vitest.
//
// The shapes measured in CM's own log: its share link
// `https://acstuff.club/s/q:race/online/join?ip=…&httpPort=…`, the protocol
// form `acmanager://race/online/join?ip=…&httpPort=…`, and Pit Box's own
// `acmanager://race/online?…`. A plain password travels as `plainPassword`;
// CM's encrypted `password` cannot be read here, so it is left to the user.

export interface JoinLink {
  ip: string;
  httpPort: number;
  password: string | null;
}

/** The server a pasted text points to, or `null` when it is not a link. */
export function parseJoinLink(text: string): JoinLink | null {
  const match = text.trim().match(/race\/online(?:\/join)?\?([^\s#]+)/i);
  if (!match) return null;
  const params = new URLSearchParams(match[1]);
  const ip = params.get("ip")?.trim();
  const httpPort = Number(params.get("httpPort"));
  if (!ip || !Number.isInteger(httpPort) || httpPort <= 0 || httpPort > 65535) return null;
  return { ip, httpPort, password: params.get("plainPassword") || null };
}

/** The link to send a friend: Content Manager's own share link, which opens
 * CM on the server from a browser or a chat, and which Pit Box reads back
 * when pasted. No password in it: whoever shares a locked server says it
 * aside, as CM's encrypted one cannot be written here. */
export function shareLink(ip: string, httpPort: number): string {
  return `https://acstuff.club/s/q:race/online/join?ip=${encodeURIComponent(ip)}&httpPort=${httpPort}`;
}
