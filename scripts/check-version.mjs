// Vérifie que les quatre fichiers qui portent le numéro de version disent la
// même chose. Lancé par `npm run check`, donc par la CI.
//
// Le contrôle existe parce que la panne est silencieuse : rien ne casse à la
// compilation quand `tauri.conf.json` reste en 0.2.0 alors que le tag dit
// v0.3.0. Ça se voit seulement après coup, dans l'écran À propos d'une release
// déjà publiée. La montée de version passe normalement par `npm version <x>`,
// qui les synchronise tous — ce contrôle attrape la modification faite à la
// main, qui en oublie toujours un.

import { versions } from "./sync-version.mjs";

const found = versions();
const distinct = [...new Set(Object.values(found))];

if (distinct.length === 1 && distinct[0]) {
  console.log(`[version] ${distinct[0]} — les quatre fichiers concordent.`);
  process.exit(0);
}

console.error("[version] numéros divergents :");
for (const [file, v] of Object.entries(found)) {
  console.error(`  ${v ?? "introuvable"}  ${file}`);
}
console.error("[version] corrige avec `npm version <x.y.z>` plutôt qu'à la main.");
process.exit(1);
