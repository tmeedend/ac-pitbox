// What the library does when the mod it shows, or the one in the session, is
// no longer there (SESSION§1, §6.3bis). Pure, so its cases are tested.
//
// Two ids to look after, and they are NOT the same one: `selected` is the last
// card clicked (Ctrl/Shift-clicks move it without touching the session), and
// `session` is what the session column shows and the launch uses. Watching
// only "selected, else session" let a deleted session track survive whenever
// the last card clicked still existed — real bug, two tracks deleted from a
// multiple selection, the session column still naming one of them.

export interface FallbackInput {
  /** Every mod of this kind, filters ignored: a mod hidden by a filter is not
   * gone, and must never make the session jump. */
  existing: string[];
  /** The cards as displayed, in order, that may join a session (not in the
   * showcase, ESPACE§6). */
  playable: string[];
  session: string | null;
  selected: string | null;
}

export interface FallbackAction {
  /** The last clicked card is gone: forget it. */
  clearSelected: boolean;
  /** The card to put in the session, or `null` to leave it as it is. */
  pick: string | null;
}

export function sessionFallback({ existing, playable, session, selected }: FallbackInput): FallbackAction {
  const gone = (id: string | null) => id != null && !existing.includes(id);
  const clearSelected = gone(selected);
  // A session mod in the showcase is still there: the column says so and
  // offers its files back (ESPACE§6) — it is not replaced behind the user's
  // back. Only a mod that no longer exists, or no session at all on a first
  // start, takes the first card that can be played. Never a showcase one:
  // it cannot enter a session, and picking it left the old choice in place.
  const needsPick = gone(session) || (session == null && (selected == null || clearSelected));
  return { clearSelected, pick: needsPick ? (playable[0] ?? null) : null };
}
