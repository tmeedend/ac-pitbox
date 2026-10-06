// The shape of a user-facing backend error, apart from its translation (which
// `errors.ts` does): a key, optionally followed by the values of its
// placeholders as a JSON object - `errors.carModelsMissing{"name":"x"}`,
// written by `errors::with_values` on the Rust side.

export interface ErrorKey {
  key: string;
  values: Record<string, string>;
}

const KEY_PREFIX = "errors.";

/** The key and its values, or `null` for a raw diagnostic (I/O, SQLite...),
 * which is shown as is. A suffix that is not a JSON object of strings leaves
 * the key without values - the sentence still shows, with its placeholder. */
export function parseErrorKey(raw: string): ErrorKey | null {
  const trimmed = raw.trim();
  if (!trimmed.startsWith(KEY_PREFIX)) return null;
  const brace = trimmed.indexOf("{");
  if (brace < 0) return { key: trimmed, values: {} };
  const key = trimmed.slice(0, brace);
  try {
    const parsed: unknown = JSON.parse(trimmed.slice(brace));
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const values: Record<string, string> = {};
      for (const [k, v] of Object.entries(parsed)) if (typeof v === "string") values[k] = v;
      return { key, values };
    }
  } catch {
    // Falls through: the key alone.
  }
  return { key, values: {} };
}
