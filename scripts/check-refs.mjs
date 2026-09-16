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
// **Le socle (`refs-baseline.json`) est vide, et c'est un aboutissement.** Il a
// été créé avec 348 couples (fichier, renvoi) cassés — des séquelles de
// renumérotations successives — précisément pour que le contrôle puisse entrer
// en CI sans tout bloquer : les renvois hérités passaient, un renvoi cassé
// **neuf** échouait. Ils ont tous été repris depuis. Le contrôle est donc
// strict aujourd'hui : **le moindre renvoi sans cible fait échouer
// `npm run check`.**
//
// On garde le fichier vide plutôt que le mécanisme : le jour où une refonte
// casse trente renvois d'un coup, `--update` permet de les geler et de les
// reprendre par lots, au lieu de tout faire d'un bloc ou de retirer le
// contrôle. **Mais on ne grossit pas le socle pour faire taire une erreur** —
// il est là pour se vider, et il est vide.
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
  L4: "LOT4-etat-de-piste.md",
  L2: "LOT2-selection-adversaires.md",
  L1: "LOT1-forme-ecran-session.md",
  SETUP: "LOT-session-setup.md",
  CIBLE: "CIBLE-reglages-session.md",
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
//
// L'étiquette peut porter des chiffres et n'en faire que deux (`L5§2.3`) : la
// première version exigeait trois majuscules, si bien que `L5§2.3` se lisait
// comme un `§2.3` nu et se faisait vérifier contre le mauvais document — en
// silence, puisque le socle le connaissait déjà sous cette forme. Une étiquette
// inconnue vaut mieux qu'une étiquette invisible : elle, au moins, échoue.
const REF = /([A-Z][A-Z0-9]{0,7})?§([0-9]+(?:\.[0-9]+)*(?:[a-z-]+(?:\.[0-9]+)*)*)/g;

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

// --- Les renvois de `docs/` : un rapport, pas une porte ---------------------
//
// **Le contrôle ci-dessus ne regarde que le code.** `git ls-files src
// src-tauri/…` n'a jamais inclus `docs/`, si bien que les documents qui
// *définissent* les sections n'ont jamais eu leurs propres renvois vérifiés.
// Cinq renvois mal dirigés y ont été trouvés à la main en deux jours, et la
// mesure qui a suivi en donne 213 — dont 15 dans `SPEC.md`, la source de
// vérité, et 8 dans `README.md`, le point d'entrée.
//
// **La règle n'est pas celle du code.** Dans un document, un `§` nu vaut
// **d'abord le document lui-même** : une spec qui écrit « voir §7.2 » parle de
// son §7.2, et c'est le cas le plus courant de loin — 401 des 679 renvois de
// `docs/`. Appliquer la règle du code (« § nu = SPEC.md ») les condamnerait
// tous. L'ordre est donc : le document, puis l'étiquette si elle est là, et
// c'est seulement quand ni l'un ni l'autre ne répond qu'il y a un défaut.
//
// **Ce que le second compteur vaut, et ce qu'il ne vaut pas.** Les 137 `§` nus
// qui sortaient de leur document ont été relus un par un (2026-09-16) : 75
// visaient une autre spec et portent désormais leur étiquette, les autres
// visaient bien `SPEC.md` et sont donc **corrects** — un `§` nu vaut `SPEC.md`,
// c'est la convention. Le compteur n'a donc pas vocation à tomber à zéro ; il
// sert de **fil-piège** : s'il bondit, quelqu'un a écrit des renvois sans se
// demander vers quel document ils pointaient. Aucune règle mécanique ne
// distingue les deux cas — il faut lire la phrase qui porte le renvoi.
//
// **Pourquoi un rapport et pas une porte.** 213 défauts préexistants ne
// passent pas en une fois, et les geler dans le socle serait précisément ce
// que `CLAUDE.md` refuse — « on ne grossit pas le socle pour faire taire une
// erreur, il est là pour se vider ». Même patron que `report-docs.mjs` : le
// nombre passe sous les yeux à chaque vérification, on le draine par lots, et
// le jour où il atteint zéro cette section devient une porte comme l'autre.
const docsNoTarget = [];
{
  const mdFiles = readdirSync("docs").filter((f) => f.endsWith(".md"));
  const own = Object.fromEntries(mdFiles.map((f) => [f, sectionsOf(f)]));
  let docTotal = 0;
  let selfRefs = 0;
  const noTarget = docsNoTarget;
  const toSpec = [];
  for (const f of mdFiles) {
    for (const m of readFileSync("docs/" + f, "utf8").matchAll(REF)) {
      docTotal++;
      const tag = m[1] ?? "";
      const ref = m[2];
      if (m[1]) {
        if (!(m[1] in DOCS)) noTarget.push(`${f} : ${m[0]} — étiquette inconnue`);
        else if (!sections[tag].has(ref)) noTarget.push(`${f} : ${m[0]} — ${DOCS[tag]} ne définit pas §${ref}`);
        continue;
      }
      // Auto-renvoi : le document définit lui-même ce numéro. Rien à signaler.
      if (own[f].has(ref)) {
        selfRefs++;
        continue;
      }
      // Il sort du document. S'il tombe dans SPEC.md c'est peut-être voulu,
      // mais rien ne le dit — et c'est exactement par là que les cinq renvois
      // mal dirigés sont passés. L'étiquette lève l'ambiguïté ; elle manque.
      if (sections[""].has(ref)) toSpec.push(`${f} : §${ref}`);
      else noTarget.push(`${f} : §${ref} — ni ce document ni SPEC.md ne le définit`);
    }
  }
  const byFile = (list) => {
    const n = {};
    for (const e of list) n[e.split(" : ")[0]] = (n[e.split(" : ")[0]] ?? 0) + 1;
    return Object.entries(n).sort((a, b) => b[1] - a[1]);
  };
  console.log(
    `[refs] docs/ : ${docTotal} renvois — ${selfRefs} auto-renvois, ` +
      `${noTarget.length} sans cible, ${toSpec.length} nus vers SPEC.md (convention)`,
  );
  const worst = [...byFile(noTarget), ...byFile(toSpec)].reduce((acc, [f, n]) => {
    acc[f] = (acc[f] ?? 0) + n;
    return acc;
  }, {});
  const top = Object.entries(worst).sort((a, b) => b[1] - a[1]).slice(0, 3);
  if (top.length) console.log(`[refs] docs/ les plus touchés : ${top.map(([f, n]) => `${f} (${n})`).join(" · ")}`);
  if (process.argv.includes("--docs")) {
    for (const e of noTarget) console.log(`  sans cible   ${e}`);
    for (const e of toSpec) console.log(`  vers SPEC   ${e}`);
  }
}

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

// Les 76 renvois sans cible de `docs/` ont été repris, donc cette moitié
// devient une porte — comme annoncé quand elle n'était qu'un rapport. L'autre
// moitié reste un rapport : un `§` nu qui sort de son document tombe sur une
// vraie section de `SPEC.md`, donc rien ne le distingue d'un renvoi correct
// sans lire la phrase qui le porte.
if (docsNoTarget.length) {
  console.error(`\n[refs] ${docsNoTarget.length} renvoi(s) de docs/ sans cible :`);
  for (const e of docsNoTarget) console.error(`  ${e}`);
  console.error(
    `\nDans un document, un « § » nu vaut d'abord CE document ; sinon il lui faut une étiquette.`,
  );
  process.exit(1);
}
