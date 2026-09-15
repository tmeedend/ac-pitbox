// Les conventions de `CLAUDE.md` qu'une machine peut vérifier, lancées par
// `npm run check`.
//
// **Pourquoi.** La section « Conventions qui ne se devinent pas » décrit huit
// classes de bugs silencieux, chacune payée au moins une fois en vrai, et
// aucune n'était vérifiée par quoi que ce soit. Le résultat était prévisible :
// `scrollIntoView` est interdit en gras dans `CLAUDE.md` **et** commenté
// « Jamais `scrollIntoView` » dans deux fichiers — et il était appelé dans deux
// autres. Une règle qu'aucun outil ne vérifie est une règle qu'on suivra
// jusqu'au jour où on est fatigué.
//
// **Ce qui entre ici, et ce qui n'y entre pas.** Seulement les règles dont la
// violation se reconnaît **sans ambiguïté** dans le texte du fichier. Une règle
// qui crie sur du code correct n'est pas une porte, c'est du bruit, et une
// porte bruyante finit désactivée. Mesuré avant d'écrire ce fichier, et donc
// **écarté** :
//
//  - « un `let _ =` s'accompagne d'un `log::warn!` » → 145 occurrences, dont
//    des quantités de légitimes (`let _ = (link, target);` pour taire un
//    paramètre inutilisé, nettoyages de test). Vraie règle, mauvais détecteur.
//  - « aucune chaîne visible en dur » → 1 880 candidats avec un détecteur
//    naïf, parce qu'un nom propre (« Assetto Corsa », « FMOD Studio ») ne se
//    traduit pas et qu'un fragment de balise multi-ligne ressemble à du texte.
//    Il faudrait un vrai parseur Svelte ; à reprendre séparément.
//
// **L'échappatoire.** Une exception légitime se déclare sur la ligne ou juste
// au-dessus : `// conventions: allow <règle>`. Elle est faite pour être rare et
// pour se voir en revue — si elle devient fréquente, c'est la règle qu'il faut
// revoir, pas les exceptions qu'il faut multiplier.

import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";

// `--others --exclude-standard` en plus du suivi : un composant tout juste
// créé et pas encore ajouté à l'index est exactement celui sur lequel on veut
// être prévenu — attendre le commit, c'est prévenir trop tard.
const ls = (paths) =>
  execSync(`git ls-files --cached --others --exclude-standard ${paths}`, { maxBuffer: 1e9 })
    .toString()
    .trim()
    .split("\n")
    .filter(Boolean);

const front = ls("src").filter((f) => /\.(ts|svelte|js)$/.test(f));
const rust = ls("src-tauri/src src-tauri/crates").filter((f) => f.endsWith(".rs"));
const read = (f) => readFileSync(f, "utf8");

const violations = [];
/** @param rule identifiant court, celui qu'on écrit dans l'échappatoire. */
function report(rule, file, line, message) {
  violations.push({ rule, file, line, message });
}

/** Une ligne (ou celle du dessus) porte-t-elle l'échappatoire de cette règle ? */
function allowed(lines, i, rule) {
  const marker = `conventions: allow ${rule}`;
  return (lines[i] ?? "").includes(marker) || (lines[i - 1] ?? "").includes(marker);
}

/** Règle par balayage de lignes, le cas le plus courant. */
function lineRule(rule, files, re, message) {
  for (const f of files) {
    const lines = read(f).split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (re.test(lines[i]) && !allowed(lines, i, rule)) report(rule, f, i + 1, message);
    }
  }
}

// --- 1. scrollIntoView ------------------------------------------------------
// Il fait défiler *tous* les ancêtres scrollables jusqu'à la fenêtre, document
// compris. Or `html`/`body` sont en `overflow: hidden` exprès : un décalage
// posé là est définitif, la molette ne peut plus le rattraper, et seul un
// redémarrage efface. Le remplaçant est `scrollIntoContainer` de
// `$lib/shellScroll`, qui ne fait défiler que le conteneur.
lineRule(
  "no-scroll-into-view",
  front,
  /\.scrollIntoView\s*\(/,
  "défilement jusqu'à la fenêtre — utiliser `scrollIntoContainer` de `$lib/shellScroll`",
);

// --- 2. écriture dans localStorage ------------------------------------------
// Règle d'or n°6 : `localStorage` n'est pas garanti synchrone sur disque côté
// WebView2, et le réglage survit tant que l'app reste ouverte mais jamais à un
// vrai redémarrage. La **lecture** reste permise : plusieurs modules la font
// pour migrer un réglage déjà posé, une fois, et ne réécrivent jamais.
lineRule(
  "no-localstorage-write",
  front,
  /\blocalStorage\s*\.\s*(setItem|removeItem|clear)\s*\(/,
  "règle d'or n°6 — persister via un fichier Rust ou `ui_prefs.json`, jamais `localStorage`",
);

// --- 3. pixels de fenêtre écrits sans diviser par le zoom -------------------
// Le zoom d'interface est un `zoom` CSS posé sur `<html>` : `getBoundingClientRect`
// et `clientX/Y` rendent des pixels **déjà multipliés**, alors qu'un `left`/`top`
// écrit sur un descendant est en pixels CSS que le zoom multipliera encore.
// Trois fois le même bug (menu contextuel, listes déroulantes, colonnes).
for (const f of front) {
  const lines = read(f).split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (!/\.style\.(left|top|right|bottom|width|height)\s*=/.test(lines[i])) continue;
    const window_ = lines.slice(Math.max(0, i - 12), i + 3).join("\n");
    if (!/getBoundingClientRect|clientX|clientY|innerWidth|innerHeight/.test(window_)) continue;
    if (/zoomFactor/.test(window_) || allowed(lines, i, "zoom-unscaled")) continue;
    report(
      "zoom-unscaled",
      f,
      i + 1,
      "mesure de fenêtre écrite dans un style — diviser par `zoomFactor()` (`$lib/zoom.svelte`)",
    );
  }
}

// --- 4. composant jamais importé --------------------------------------------
// Le CSS d'un composant Svelte est scopé et rien ne signale un fichier que plus
// personne ne rend : deux y ont survécu à la refonte de la navigation, 876
// lignes en tout, et l'un d'eux tenait encore des clés i18n vivantes.
for (const f of front.filter((f) => f.endsWith(".svelte"))) {
  const name = f.split("/").pop().replace(".svelte", "");
  // Une route SvelteKit (`+page.svelte`…) n'est jamais importée : c'est le
  // routeur qui la monte.
  if (name.startsWith("+")) continue;
  const imported = front.some((g) => g !== f && new RegExp(`/${name}\\.svelte["']`).test(read(g)));
  if (!imported) report("orphan-component", f, 1, "composant importé nulle part");
}

// --- 5. commande Tauri non enregistrée --------------------------------------
// Oublier `invoke_handler` ne casse **rien** à la compilation : l'erreur
// n'apparaît qu'à l'exécution, quand l'écran qui appelle la commande reste
// vide. C'est exactement le genre d'écart qu'une machine doit tenir.
{
  const defined = new Map();
  for (const f of ls("src-tauri/src/commands")) {
    const lines = read(f).split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (!/#\[tauri::command/.test(lines[i])) continue;
      // Entre l'attribut et la signature peuvent s'intercaler d'autres
      // attributs (`#[cfg(windows)]`) et des commentaires.
      for (let j = i + 1; j < Math.min(i + 8, lines.length); j++) {
        const m = /^\s*pub\s+(?:async\s+)?fn\s+([a-z_0-9]+)/.exec(lines[j]);
        if (m) {
          defined.set(m[1], [f, j + 1]);
          break;
        }
      }
    }
  }
  const lib = read("src-tauri/src/lib.rs").split("\n");
  const from = lib.findIndex((l) => l.includes("generate_handler!["));
  // Jusqu'à la ligne qui referme la macro — et non le premier `]` venu : la
  // liste contient des `#[cfg(windows)]`, qui en portent un chacun.
  const to = lib.findIndex((l, i) => i > from && /^\s*\]\)/.test(l));
  const registered = new Set(
    lib
      .slice(from, to)
      .join("\n")
      .match(/commands::[a-z_0-9]+::([a-z_0-9]+)/g)
      ?.map((s) => s.split("::").pop()) ?? [],
  );
  for (const [name, [f, line]] of defined) {
    if (!registered.has(name)) report("tauri-command-unregistered", f, line, `\`${name}\` absente d'\`invoke_handler\``);
  }
}

// --- 6. clé i18n jamais atteignable -----------------------------------------
// `check-locales.mjs` vérifie la cohérence **entre** locales, pas l'usage : une
// clé dont l'écran a disparu y reste indéfiniment (119 retrouvées d'un coup).
//
// **Le détecteur est volontairement conservateur** : tout littéral qui
// ressemble à un préfixe de clé protège ce qui est dessous, quel que soit ce
// qui le suit. Rater un préfixe garde une clé morte, c'est sans gravité ;
// l'inverse supprime une clé vivante. C'est ce qui a sauvé
// `driver.fromLivery.*`, construite par `(cond ? "a." : "b.") + lane`.
{
  const en = JSON.parse(read("src/lib/i18n/locales/en.json"));
  const keys = [];
  (function walk(o, p) {
    for (const k in o) {
      const q = p ? `${p}.${k}` : k;
      if (o[k] && typeof o[k] === "object") walk(o[k], q);
      else keys.push(q);
    }
  })(en, "");

  const sources = [...front, ...rust].filter((f) => !f.includes("i18n/locales")).map(read).join("\n");
  const prefixes = new Set();
  for (const m of sources.matchAll(/[`"']([a-zA-Z][a-zA-Z0-9_.]*)\$\{/g)) prefixes.add(m[1]);
  for (const m of sources.matchAll(/[`"']([a-zA-Z][a-zA-Z0-9_.]*[._])[`"']/g)) prefixes.add(m[1]);

  const dead = keys.filter((k) => !sources.includes(k) && ![...prefixes].some((p) => k.startsWith(p)));
  for (const k of dead) {
    report("i18n-unused-key", "src/lib/i18n/locales/en.json", 1, `\`${k}\` n'est plus atteignable`);
  }
}

// --- 7. renvoi de spec dans une chaîne visible -------------------------------
// Trouvé en vrai : « contenu de base Kunos : déjà présent, non activable
// (§12bis.1). » s'affichait tel quel. Un numéro de section ne veut rien dire
// pour qui utilise l'app, et il périme en silence — celui-là désignait une
// section disparue depuis longtemps. Le renvoi appartient au commentaire du
// code, jamais au libellé ; c'est aussi ce que dit la règle « un libellé
// d'écran n'explique jamais son propre fonctionnement ».
for (const locale of ["fr", "en", "it", "de", "es", "pt"]) {
  const file = `src/lib/i18n/locales/${locale}.json`;
  const walk = (o, path) => {
    for (const k in o) {
      const q = path ? `${path}.${k}` : k;
      if (o[k] && typeof o[k] === "object") walk(o[k], q);
      else if (typeof o[k] === "string" && /§[0-9]/.test(o[k])) {
        report("no-spec-ref-in-locale", file, 1, `\`${q}\` affiche un renvoi de spec`);
      }
    }
  };
  walk(JSON.parse(read(file)), "");
}

// --- Rapport ----------------------------------------------------------------

if (!violations.length) {
  console.log("[conventions] rien à signaler");
  process.exit(0);
}

const byRule = new Map();
for (const v of violations) byRule.set(v.rule, [...(byRule.get(v.rule) ?? []), v]);

console.error(`[conventions] ${violations.length} écart(s) :\n`);
for (const [rule, list] of byRule) {
  console.error(`  ${rule} (${list.length})`);
  for (const v of list.slice(0, 12)) console.error(`    ${v.file}:${v.line} — ${v.message}`);
  if (list.length > 12) console.error(`    … et ${list.length - 12} de plus`);
  console.error("");
}
console.error("Exception légitime : `// conventions: allow <règle>` sur la ligne ou juste au-dessus.");
process.exit(1);
