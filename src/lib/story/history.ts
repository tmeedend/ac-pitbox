// Undo, redo and copies in the studio, the parts that can be tested without
// a screen: which key presses mean undo or redo, and when a copy was taken.

export type HistoryKey = "undo" | "redo";

/** What a key press asks of the studio's history, if anything. Ctrl+Z
 * undoes; Ctrl+Y and Ctrl+Shift+Z redo. Inside a text field the field keeps
 * the keys: there they undo the typing, as everywhere else. */
export function historyKey(
  e: { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean },
  inField: boolean,
): HistoryKey | null {
  if (inField || e.altKey || !(e.ctrlKey || e.metaKey)) return null;
  const key = e.key.toLowerCase();
  if (key === "z") return e.shiftKey ? "redo" : "undo";
  if (key === "y" && !e.shiftKey) return "redo";
  return null;
}

/** Whether a key press lands in something the author types in. */
export function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el?.tagName) return false;
  return ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName) || el.isContentEditable;
}

/** When a copy was taken, from its id (`YYYY-MM-DD_HH-MM-SS`, local time);
 * `null` for an id of another shape. */
export function backupDate(id: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/.exec(id);
  if (!m) return null;
  const [y, mo, d, h, mi, s] = m.slice(1).map(Number);
  return new Date(y, mo - 1, d, h, mi, s);
}
