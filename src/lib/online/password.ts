// A server's password checked before the game is launched
// (SPEC-play-online.md, "Ce qu'un serveur AC expose": "on évite le chargement
// de 40 s pour se faire refuser"). The recipe and why it is trusted are in
// `online/extended.rs` (`PasswordCheck`). Pure, for Vitest.

export interface PasswordCheck {
  /** The server's name, as `/api/details` gives it. */
  salt: string;
  /** Lowercase hex SHA-1: the player's password and the admin's. */
  checksums: string[];
}

async function sha1Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-1", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Whether `password` is one the server accepts. An empty one never is: a
 * server without an admin password publishes the checksum of an empty one,
 * which would otherwise pass for the player's. */
export async function passwordMatches(check: PasswordCheck, password: string): Promise<boolean> {
  if (!password) return false;
  return check.checksums.includes(await sha1Hex(`apatosaur${check.salt}${password}`));
}
