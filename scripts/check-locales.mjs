// Contrôle de cohérence des dictionnaires i18n, lancé par `npm run check`.
//
// Trois niveaux de sévérité, et ils ne sont pas arbitraires — ils suivent ce
// que chaque écart coûte réellement à l'écran :
//
//  - **Clé inconnue** (présente dans une locale, absente de la référence) =
//    ERREUR. C'est soit une faute de frappe dans le chemin, soit une clé
//    devenue morte : dans les deux cas elle ne s'affichera jamais, et personne
//    ne s'en apercevra sans ce contrôle.
//  - **Clé manquante dans `fr`** = ERREUR. Le français et l'anglais sont le
//    couple de référence maintenu par le projet (voir CLAUDE.md) : une clé
//    n'existe pas tant qu'elle n'est pas dans les deux.
//  - **Variable perdue ou inventée** (`{count}`, `{name}`…) = ERREUR. Une
//    traduction qui laisse tomber un `{count}` produit une phrase amputée
//    (« mods importés » au lieu de « 12 mods importés »), et une qui invente
//    un nom de variable affiche l'accolade brute à l'écran. Ni l'un ni l'autre
//    ne se voit à la relecture d'une langue qu'on ne parle pas.
//  - **Clé manquante dans une locale de traduction** = simple compte affiché.
//    `t()` retombe sur l'anglais (`i18n/index.svelte.ts`), donc une traduction
//    partielle s'affiche en anglais là où elle manque — jamais la clé brute.
//    C'est ce qui rend une contribution incomplète acceptable.
//
// Sortie sèche quand tout va bien : une ligne par locale traduite.

import { readdirSync, readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "lib", "i18n", "locales");
/** Dictionnaire de référence : il définit l'ensemble des clés qui existent. */
const REFERENCE = "en";
/** Doit être complète, comme la référence. */
const REQUIRED = ["fr"];

/** Noms des variables interpolées d'une chaîne : `{count}` → `count`. */
function placeholders(text) {
  return new Set([...String(text).matchAll(/\{(\w+)\}/g)].map((m) => m[1]));
}

/** Chaînes d'un dictionnaire, indexées par chemin en points. */
function stringsOf(value, prefix = "", out = new Map()) {
  for (const [k, v] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object") stringsOf(v, path, out);
    else out.set(path, v);
  }
  return out;
}

const load = (code) => JSON.parse(readFileSync(join(DIR, `${code}.json`), "utf8"));
const codes = readdirSync(DIR)
  .filter((f) => f.endsWith(".json"))
  .map((f) => f.slice(0, -5));

const referenceStrings = stringsOf(load(REFERENCE));
const reference = new Set(referenceStrings.keys());
let failed = false;

for (const code of codes) {
  if (code === REFERENCE) continue;
  const strings = stringsOf(load(code));
  const keys = new Set(strings.keys());

  for (const [k, en] of referenceStrings) {
    if (!strings.has(k)) continue;
    const want = placeholders(en);
    const got = placeholders(strings.get(k));
    const lost = [...want].filter((p) => !got.has(p));
    const made = [...got].filter((p) => !want.has(p));
    if (lost.length || made.length) {
      const detail = [lost.length ? `perdue(s) : ${lost.join(", ")}` : "", made.length ? `inventée(s) : ${made.join(", ")}` : ""]
        .filter(Boolean)
        .join(" · ");
      console.error(`[locales] ${code}.json : « ${k} » — variable ${detail}`);
      failed = true;
    }
  }
  const missing = [...reference].filter((k) => !keys.has(k));
  const unknown = [...keys].filter((k) => !reference.has(k));

  for (const k of unknown) {
    console.error(`[locales] ${code}.json : clé inconnue « ${k} » — absente de ${REFERENCE}.json`);
    failed = true;
  }
  if (REQUIRED.includes(code)) {
    for (const k of missing) {
      console.error(`[locales] ${code}.json : clé manquante « ${k} »`);
      failed = true;
    }
    if (!missing.length && !unknown.length) console.log(`[locales] ${code} : complète (${keys.size} clés)`);
  } else {
    const done = reference.size - missing.length;
    const pct = Math.round((done / reference.size) * 100);
    console.log(`[locales] ${code} : ${done}/${reference.size} clés traduites (${pct} %)`);
  }
}

if (failed) process.exit(1);
