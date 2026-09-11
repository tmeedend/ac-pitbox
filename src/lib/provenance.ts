// D'où vient une entrée de la bibliothèque (§4.5.2, §7.3).
//
// `source_archive` ne contient pas la même chose selon le type d'entrée. Une
// voiture y porte le nom de l'archive qui la livrait. Un mod « autre » issu
// d'un **reste** (§7.3) y porte la chaîne que l'import a fabriquée pour le
// nommer — `<archive>__<chemin dans l'archive>` —, parce que c'est elle qui
// sert aussi à former son identifiant :
//
//   SOME1_NSX_2026-02-15.7z__SOME1_NSX_2026-02-15\content\fonts
//
// Affichée telle quelle sous « Provenance », elle prétend être un nom
// d'archive et n'en est pas un. Les deux moitiés sont utiles, mais elles
// répondent à deux questions : « quelle archive ? » et « où dedans ? ».

export interface Provenance {
  /** Le nom de l'archive (ou du dossier) d'origine, seul. */
  archive: string;
  /** Là où ce reste se trouvait **dans** l'archive, séparateurs normalisés en
   * `/` comme les chemins de pose. `null` quand la livraison était l'archive
   * entière — le cas de tous les mods reconnus. */
  inside: string | null;
}

/** La convention de nommage des restes, et elle seule : un simple `_` sépare
 * des mots à l'intérieur d'un nom de fichier, un double `_` sépare l'archive
 * de ce qu'on en a tiré. */
const SEPARATOR = "__";

export function splitProvenance(source: string | null | undefined): Provenance | null {
  const raw = source?.trim();
  if (!raw) return null;
  const cut = raw.indexOf(SEPARATOR);
  // Séparateur absent, ou en tête (rien avant lui à prendre pour un nom
  // d'archive) : la chaîne entière est la provenance, sans découpe inventée.
  if (cut <= 0) return { archive: raw, inside: null };
  const inside = raw.slice(cut + SEPARATOR.length).trim();
  return {
    archive: raw.slice(0, cut),
    inside: inside ? inside.split("\\").join("/") : null,
  };
}
