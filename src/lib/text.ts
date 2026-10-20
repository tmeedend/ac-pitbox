// Comparing names the way a person reads them: shared by the searches and the
// matchers of several screens (Classement, Online, the library, the game
// folder), which each had their own copy.

/** Lowercase, accents gone: `Škoda` → `skoda`. What every comparison starts
 * from; each caller then strips what it does not compare (spaces, digits…). */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

/** Levenshtein distance: the fewest insertions, deletions and substitutions
 * of one character that turn `a` into `b`. */
export function editDistance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let prev = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const cur = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = cur;
    }
  }
  return row[b.length];
}
