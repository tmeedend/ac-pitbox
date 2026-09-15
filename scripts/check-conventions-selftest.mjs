// Chaque règle de `check-conventions.mjs` sait-elle **échouer** ?
//
// **Pourquoi ce banc.** Une porte verte dont les règles sont cassées est pire
// qu'aucune porte : elle donne l'assurance sans le contrôle. Écrit en même
// temps que les règles, il en a immédiatement trouvé une vraie fausse — la
// détection de composant orphelin ne regardait que les fichiers **suivis** par
// git, donc un composant tout juste créé y échappait, c'est-à-dire précisément
// celui sur lequel on veut être prévenu. (Il a aussi démasqué deux défauts de
// ce fichier-ci : `lib.rs` est en CRLF, et injecter un `getBoundingClientRect`
// dans le module qui définit `zoomFactor` ne prouve rien.)
//
// Hors de `npm run verify` **exprès** : il modifie de vrais fichiers suivis.
// À lancer à la main quand on ajoute ou change une règle :
//
//     node scripts/check-conventions-selftest.mjs
//
// Il restaure tout dans un `finally`, y compris si une règle plante.

import { readFileSync, writeFileSync, unlinkSync, existsSync } from "node:fs";
import { execSync } from "node:child_process";

/** Sortie du contrôleur, qu'il passe ou non. */
function run() {
  try {
    execSync("node scripts/check-conventions.mjs", { stdio: "pipe" });
    return "";
  } catch (e) {
    return (e.stdout?.toString() ?? "") + (e.stderr?.toString() ?? "");
  }
}

/** Chaque cas injecte **une** violation et attend que sa règle la nomme. */
const cases = [
  {
    rule: "no-scroll-into-view",
    file: "src/lib/shell/zoom.svelte.ts",
    inject: (s) => `${s}\nfunction _probe(e: HTMLElement) { e.scrollIntoView({ block: "center" }); }\n`,
  },
  {
    rule: "no-localstorage-write",
    file: "src/lib/shell/zoom.svelte.ts",
    inject: (s) => `${s}\nfunction _probe() { localStorage.setItem("x", "y"); }\n`,
  },
  {
    // Surtout pas dans `zoom.svelte.ts` : le module qui **définit**
    // `zoomFactor` contient le mot partout, donc la règle s'y croirait
    // satisfaite et le test passerait pour une règle muette.
    rule: "zoom-unscaled",
    file: "src/lib/format.ts",
    inject: (s) =>
      `${s}\nfunction _probe(a: HTMLElement, b: HTMLElement) {\n  const r = a.getBoundingClientRect();\n  b.style.left = \`\${r.left}px\`;\n}\n`,
  },
  {
    rule: "orphan-component",
    file: "src/lib/components/ui/ZzSelfTestOrphan.svelte",
    create: '<script lang="ts">\n  let x = 1;\n</script>\n\n<span>{x}</span>\n',
  },
  {
    // `lib.rs` est en CRLF : viser les deux fins de ligne, sinon on n'injecte
    // rien et on conclut que la règle ne sait pas échouer.
    rule: "tauri-command-unregistered",
    file: "src-tauri/src/lib.rs",
    inject: (s) => s.replace(/ *commands::config::get_config,\r?\n/, ""),
  },
  {
    rule: "i18n-unused-key",
    file: "src/lib/i18n/locales/en.json",
    inject: (s) => `${JSON.stringify({ ...JSON.parse(s), zzSelfTestDeadKey: "never used" }, null, 2)}\n`,
  },
  {
    // La clé est aussi citée nulle part, donc elle déclencherait `i18n-unused-key` :
    // ce qu'on vérifie ici est que `no-spec-ref-in-locale` sort **aussi**.
    rule: "no-spec-ref-in-locale",
    file: "src/lib/i18n/locales/fr.json",
    inject: (s) => `${JSON.stringify({ ...JSON.parse(s), zzSelfTestRef: "voir la règle (§4.4)." }, null, 2)}\n`,
  },
  {
    // Valeur volontairement fantaisiste : ce qu'on teste est la **forme** du
    // jeton, celle que les robots de moissonnage cherchent sur GitHub.
    rule: "no-secret",
    file: "src/lib/shell/zoom.svelte.ts",
    inject: (s) => `${s}\nconst _probe = "ghp_000000000000000000000000000000000000";\n`,
  },
  {
    rule: "no-secret-file",
    file: "zz-selftest-probe.pem",
    create: "pas une vraie clé, juste un nom de fichier\n",
  },
  {
    rule: "docs-index-incomplete",
    file: "docs/zz-selftest-orphan.md",
    create: "# Document que l'index ne connaît pas\n",
  },
];

let proven = 0;
const total = cases.length + 1;

for (const c of cases) {
  const before = existsSync(c.file) ? readFileSync(c.file, "utf8") : null;
  try {
    writeFileSync(c.file, c.create ?? c.inject(before));
    const out = run();
    if (out.includes(c.rule)) {
      proven++;
      console.log(`OK    ${c.rule}`);
    } else {
      console.log(`RATÉ  ${c.rule} — la violation injectée n'a rien déclenché`);
    }
  } finally {
    if (c.create) unlinkSync(c.file);
    else writeFileSync(c.file, before);
  }
}

// L'échappatoire doit éteindre la règle, sinon elle n'est pas utilisable et
// quelqu'un finira par retirer la règle plutôt que de la contourner proprement.
{
  const file = "src/lib/shell/zoom.svelte.ts";
  const before = readFileSync(file, "utf8");
  try {
    writeFileSync(
      file,
      `${before}\n// conventions: allow no-scroll-into-view\nfunction _probe(e: HTMLElement) { e.scrollIntoView(); }\n`,
    );
    if (!run().includes("no-scroll-into-view")) {
      proven++;
      console.log("OK    échappatoire `conventions: allow`");
    } else {
      console.log("RATÉ  échappatoire `conventions: allow` — la règle crie quand même");
    }
  } finally {
    writeFileSync(file, before);
  }
}

console.log(`\n${proven}/${total} règles prouvées`);
process.exit(proven === total ? 0 : 1);
