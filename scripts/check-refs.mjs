// Les renvois `§X` du code pointent-ils sur une section qui existe ?
//
// **Pourquoi ce script.** Le code porte ~2 800 renvois vers les specs, et rien
// ne les vérifiait : un numéro reste valide à l'œil longtemps après que la
// section a été renumérotée ou fondue dans une autre. Mesuré avant d'écrire
// ceci : 401 renvois ne pointaient sur aucun titre d'aucun document, et deux
// tiers étaient **ambigus** — `§5.3` désigne SPEC-grille dans `gridthumbs.rs`
// et SPEC.md dans `importer.rs`, sans que rien ne le dise.
//
// **La convention.** Un `§X` nu désigne `docs/SPEC.md`, qui reste la référence
// par défaut. Les autres documents ont une étiquette, collée devant : on écrit
// `GRILLE§5.3`, `WIKI§4.2`, `PILOTE§6.3`. C'est la seule façon qu'un renvoi
// dise de lui-même où il va.
//
// **Le socle (`refs-baseline.json`).** Les renvois déjà cassés au moment où ce
// contrôle est né y sont listés, un par un. Ils ne font pas échouer la CI —
// sinon rien ne passerait tant qu'ils ne sont pas tous repris — mais **tout
// nouveau renvoi cassé, lui, échoue**. La pourriture s'arrête aujourd'hui et se
// résorbe au fil de l'eau. Un renvoi du socle qu'on répare est simplement
// signalé : `node scripts/check-refs.mjs --update` retire l'entrée.
//
// Une entrée du socle est un couple (fichier, renvoi), pas une ligne : un même
// renvoi cassé répété dans un fichier n'y figure qu'une fois. C'est délibéré —
// sinon le socle changerait au moindre déplacement de ligne, et son diff
// noierait le seul événement qui compte, l'apparition d'un renvoi neuf.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

/** Étiquette → document. Un `§` nu vaut `SPEC.md`. */
const DOCS = {
  "": "SPEC.md",
  SESSION: "SPEC-session.md",
  GRILLE: "SPEC-grille.md",
  PILOTE: "SPEC-ecran-pilote.md",
  WIKI: "SPEC-wikipedia-fiche-detail.md",
  PREVIEW: "SPEC-preview-3d-kn5.md",
  FMOD: "SPEC-engine-sound-fmod.md",
  IMPORT: "SPEC-import.md",
  REFONTE: "SPEC-refonte-navigation-et-fiches.md",
  TEXTURE: "SPEC-texture-update.md",
  // Les instructions par lot de la refonte de l'écran de session. Livrées,
  // gardées pour leurs arguments — et parce que le code y renvoie.
  L5: "LOT5-refonte-ecran-session.md",
  // Le module musique renvoie à sa spec d'origine, écrite pour une autre stack
  // (C#/NAudio) : elle garde sa numérotation propre, et §16 de SPEC.md décrit
  // ce que l'app en a réellement fait.
  MUSIQUE: "spec-module-musique_2.md",
};

const BASELINE = "scripts/refs-baseline.json";

/** Numéros de section réellement définis par un document. */
function sectionsOf(file) {
  const set = new Set();
  for (const line of readFileSync("docs/" + file, "utf8").split(/\r?\n/)) {
    // Les documents ne s'accordent pas sur le niveau de titre — SPEC-grille met
    // ses sections en `#`, SPEC.md en `##` — donc on accepte n'importe lequel et
    // on ne regarde que le numéro en tête de texte.
    const m = /^#{1,6}\s+([0-9]+(?:\.[0-9]+)*(?:[a-z-]+(?:\.[0-9]+)*)*)[.\s]/.exec(line);
    if (m) set.add(m[1].replace(/\.$/, ""));
  }
  return set;
}

const sections = Object.fromEntries(
  Object.entries(DOCS).map(([tag, file]) => [tag, sectionsOf(file)]),
);

const missingDocs = Object.entries(DOCS).filter(([, f]) => !readdirSync("docs").includes(f));
if (missingDocs.length) {
  console.error(`[refs] document introuvable : ${missingDocs.map(([, f]) => f).join(", ")}`);
  process.exit(1);
}

// Pas de `\b` devant l'étiquette : un `§` nu est le plus souvent précédé d'une
// parenthèse ou d'une espace, où `\b` ne matche pas — c'est ce qui faisait
// rendre « 0 renvoi » au premier jet.
const REF = /([A-Z]{3,8})?§([0-9]+(?:\.[0-9]+)*(?:[a-z-]+(?:\.[0-9]+)*)*)/g;

const files = execSync("git ls-files src src-tauri/src src-tauri/crates", { maxBuffer: 1e9 })
  .toString().trim().split("\n")
  .filter((f) => /\.(rs|ts|svelte|js)$/.test(f));

const broken = [];
let total = 0;
for (const f of files) {
  const text = readFileSync(f, "utf8");
  for (const m of text.matchAll(REF)) {
    total++;
    const tag = m[1] ?? "";
    const ref = m[2];
    if (m[1] && !(m[1] in DOCS)) {
      broken.push({ file: f, ref: m[0], why: `étiquette inconnue « ${m[1]} »` });
      continue;
    }
    if (!sections[tag].has(ref)) {
      broken.push({ file: f, ref: m[0], why: `${DOCS[tag]} ne définit pas §${ref}` });
    }
  }
}

const key = (b) => `${b.file}|${b.ref}`;
let baseline = [];
try {
  baseline = JSON.parse(readFileSync(BASELINE, "utf8")).entries;
} catch {
  baseline = [];
}

if (process.argv.includes("--update")) {
  const entries = [...new Set(broken.map(key))].sort();
  writeFileSync(
    BASELINE,
    JSON.stringify(
      {
        _comment:
          "Renvois §X cassés hérités, tolérés le temps d'être repris. Tout NOUVEAU renvoi cassé fait échouer scripts/check-refs.mjs. Régénérer avec : node scripts/check-refs.mjs --update",
        entries,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`[refs] socle réécrit : ${entries.length} renvois hérités`);
  process.exit(0);
}

const known = new Set(baseline);
const fresh = broken.filter((b) => !known.has(key(b)));
const fixed = [...known].filter((k) => !broken.some((b) => key(b) === k));

console.log(`[refs] ${total} renvois §X, ${broken.length} sans cible (${baseline.length} hérités)`);

if (fixed.length) {
  console.log(`[refs] ${fixed.length} renvoi(s) du socle sont réparés — « node scripts/check-refs.mjs --update » les retire.`);
}

if (fresh.length) {
  console.error(`\n[refs] ${fresh.length} renvoi(s) cassés, hors socle :`);
  for (const b of fresh) console.error(`  ${b.file} : ${b.ref} — ${b.why}`);
  console.error(
    `\nUn renvoi désigne son document : « §4.5 » = SPEC.md, sinon une étiquette (${Object.keys(DOCS).filter(Boolean).join(", ")}).`,
  );
  process.exit(1);
}
