// Poids de la documentation — **un rapport, jamais une porte.**
//
// **Pourquoi.** `SPEC.md` a atteint 2 448 lignes dont 924 pour son seul §9,
// 38 % du fichier dans une section. Personne ne l'a vu venir : un document
// grossit d'une ligne à la fois, et aucune ligne n'est de trop. Quand on s'en
// aperçoit, la section est devenue impossible à relire et sa numérotation a
// dégénéré en `bis`/`ter`/`quater` — parce que renuméroter cassait des renvois
// en silence, donc personne ne renumérotait.
//
// **Aucun seuil, aucun échec.** Un seuil serait arbitraire, et une porte qui
// refuse un commit parce qu'un document a grandi de dix lignes finit
// désactivée — c'est la leçon de `check-conventions.mjs`, où une règle qui
// crie sur du code correct n'entre pas. Ici on affiche un nombre à chaque
// `npm run check`, à côté du décompte de locales, et c'est l'œil humain qui
// décide quand découper. Le sort du §9 se serait vu venir des mois à l'avance.
//
// Sortie toujours en succès : ce script est un `report:`, pas un `check:`.

import { readdirSync, readFileSync } from "node:fs";

/** Sections d'un document, avec le nombre de lignes de chacune. */
function sections(file) {
  const lines = readFileSync(`docs/${file}`, "utf8").split(/\r?\n/);
  const out = [];
  let current = null;
  for (const line of lines) {
    // Un titre numéroté, à n'importe quel niveau : les documents ne s'accordent
    // pas là-dessus (SPEC-grille met ses sections en `#`, SPEC.md en `##`).
    const m = /^(#{1,3})\s+([0-9]+(?:\.[0-9]+)*(?:[a-z-]+(?:\.[0-9]+)*)*)[.\s]/.exec(line);
    // Seulement les sections de **premier** niveau numéroté : une sous-section
    // lourde n'est pas un problème en soi, une section-monde si.
    if (m && !m[2].includes(".")) {
      current = { num: m[2], n: 0 };
      out.push(current);
    }
    if (current) current.n++;
  }
  return out;
}

const docs = readdirSync("docs").filter((f) => f.endsWith(".md"));
let totalLines = 0;
const heaviest = [];

for (const f of docs) {
  const lines = readFileSync(`docs/${f}`, "utf8").split(/\r?\n/).length;
  totalLines += lines;
  for (const s of sections(f)) heaviest.push({ file: f, ...s, of: lines });
}

heaviest.sort((a, b) => b.n - a.n);
const top = heaviest
  .slice(0, 3)
  .map((s) => `${s.file} §${s.num} (${s.n} l., ${Math.round((s.n / s.of) * 100)} %)`);

console.log(`[docs] ${docs.length} documents, ${totalLines.toLocaleString("fr-FR")} lignes`);
console.log(`[docs] sections les plus lourdes : ${top.join(" · ")}`);
