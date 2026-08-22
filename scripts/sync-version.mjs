// Propage le numéro de version de `package.json` vers les autres fichiers qui
// le portent. Appelé par le hook `version` de npm, donc au milieu de
// `npm version <x>` : les fichiers écrits ici sont ajoutés à l'index par le
// même hook, et npm les emporte dans le commit et le tag qu'il crée ensuite.
//
// `package.json` est la source, parce que c'est le seul fichier dont `npm
// version` sait bumper le numéro tout seul — le reste suit.
//
// Trois destinations, et aucune n'est décorative :
//
//  - `tauri.conf.json` : c'est **elle** que l'installateur et l'écran À propos
//    affichent. Un oubli ici produit une release `v0.3.0` dont le `.exe`
//    annonce 0.2.0 — exactement le genre d'incohérence qu'un utilisateur
//    remonte comme un bug.
//  - `Cargo.toml` : la version du crate.
//  - `Cargo.lock` : il est suivi par git, donc le laisser en arrière salit le
//    diff du build suivant. Corrigé ici par simple remplacement plutôt qu'en
//    appelant cargo : ce script doit tourner même sans chaîne Rust installée.

import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");
const write = (p, s) => writeFileSync(join(ROOT, p), s);

export const VERSION = JSON.parse(read("package.json")).version;

/** Remplace la première occurrence, en refusant de le faire en silence si le
 * motif n'est plus là — un fichier réorganisé doit casser le script, pas le
 * laisser croire qu'il a fait son travail. */
function replaceOnce(path, pattern, replacement) {
  const before = read(path);
  const after = before.replace(pattern, replacement);
  if (after === before) {
    throw new Error(`[version] ${path} : motif introuvable (${pattern}) — script à mettre à jour`);
  }
  write(path, after);
}

/** Numéros lus dans chaque fichier, pour la synchro comme pour le contrôle. */
export function versions() {
  return {
    "package.json": JSON.parse(read("package.json")).version,
    "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
    "src-tauri/Cargo.toml": read("src-tauri/Cargo.toml").match(/^name = "pitbox"\r?\n^version = "([^"]+)"/m)?.[1],
    "src-tauri/Cargo.lock": read("src-tauri/Cargo.lock").match(/^name = "pitbox"\r?\n^version = "([^"]+)"/m)?.[1],
  };
}

if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("sync-version.mjs")) {
  replaceOnce("src-tauri/tauri.conf.json", /"version": "[^"]+"/, `"version": "${VERSION}"`);
  // Ancré sur `name = "pitbox"` : `Cargo.toml` contient d'autres `version =`
  // (les dépendances), et `Cargo.lock` en contient un par paquet.
  const anchored = /^(name = "pitbox"\r?\n)version = "[^"]+"/m;
  replaceOnce("src-tauri/Cargo.toml", anchored, `$1version = "${VERSION}"`);
  replaceOnce("src-tauri/Cargo.lock", anchored, `$1version = "${VERSION}"`);
  console.log(`[version] ${VERSION} propagée dans tauri.conf.json, Cargo.toml et Cargo.lock.`);
}
