// Highlighting of what matched in a search result (DOSSIER§7.3). The backend
// compares case- and accent-insensitively; the highlight must find the same
// places in the text as displayed, accents included.

/** One character folded the way the search folds: lowercase, no accent. A
 * character whose folded form is not a single character stays as it is, so
 * that indexes in the folded text are indexes in the original. */
function foldChar(c: string): string {
  const f = c.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  return f.length === 1 ? f : c.toLowerCase().length === 1 ? c.toLowerCase() : c;
}

export interface Part {
  text: string;
  hit: boolean;
}

/** `text` cut into parts, the ones matching a word of `query` marked. A query
 * with a separator is one pattern (a path); otherwise every word is. */
export function highlightParts(text: string, query: string): Part[] {
  const q = [...query.trim()].map(foldChar).join("").replace(/\\/g, "/");
  const words = q.includes("/") ? [q] : q.split(/\s+/).filter(Boolean);
  const chars = [...text];
  const folded = chars.map(foldChar).join("").replace(/\\/g, "/");
  const marks: boolean[] = new Array(chars.length).fill(false);
  for (const w of words) {
    let from = 0;
    for (;;) {
      const at = folded.indexOf(w, from);
      if (at < 0) break;
      for (let i = at; i < at + w.length && i < marks.length; i++) marks[i] = true;
      from = at + Math.max(1, w.length);
    }
  }
  const parts: Part[] = [];
  chars.forEach((c, i) => {
    const last = parts[parts.length - 1];
    if (last && last.hit === marks[i]) last.text += c;
    else parts.push({ text: c, hit: marks[i] });
  });
  return parts;
}
