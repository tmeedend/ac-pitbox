// Mise en forme du texte d'article (SPEC-wikipedia-fiche-detail.md WIKI§7.3).
//
// Module séparé de `wiki.ts` **et sans import Tauri** : c'est de la logique
// pure, donc testable par Vitest, et `wiki.ts` traîne `@tauri-apps/api`.
//
// L'API rend l'article en texte brut, mais les titres de section y gardent leur
// balisage wiki : `== Overview ==`, `=== First generation – NA (1989–1997) ===`
// (mesuré sur `Mazda MX-5`). Affichés tels quels, ces `=` sont du bruit ; les
// retirer sans rien en faire écraserait la structure d'un article de 17 000
// caractères en un seul pavé.
//
// **Rien n'est réécrit ici** (WIKI§2) : on reconnaît une ligne de titre et on la
// balise, le texte lui-même n'est jamais touché.

export type WikiBlockKind = "heading" | "paragraph";

export interface WikiTextBlock {
  kind: WikiBlockKind;
  /** 2 pour `==`, 3 pour `===`… Sans objet pour un paragraphe. */
  level: number;
  text: string;
}

/** `== Titre ==` → niveau et libellé. `null` si la ligne n'est pas un titre. */
function asHeading(line: string): { level: number; text: string } | null {
  const m = /^(={2,6})\s*(.+?)\s*\1$/.exec(line.trim());
  if (!m) return null;
  const text = m[2].trim();
  return text ? { level: m[1].length, text } : null;
}

/**
 * Découpe l'article en blocs affichables.
 *
 * Les lignes vides séparent les paragraphes ; une ligne de titre en ouvre un
 * nouveau. Le texte de chaque paragraphe est rendu **tel quel**, sauts de ligne
 * internes compris.
 */
export function parseExtract(extract: string): WikiTextBlock[] {
  const blocks: WikiTextBlock[] = [];
  let buffer: string[] = [];

  const flush = () => {
    const text = buffer.join("\n").trim();
    if (text) blocks.push({ kind: "paragraph", level: 0, text });
    buffer = [];
  };

  for (const line of extract.split(/\r?\n/)) {
    const heading = asHeading(line);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", level: heading.level, text: heading.text });
      continue;
    }
    if (!line.trim()) {
      flush();
      continue;
    }
    buffer.push(line);
  }
  flush();
  return blocks;
}
