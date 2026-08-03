// Résolution des erreurs remontées par le backend.
//
// Les erreurs destinées à l'utilisateur voyagent sous forme de CLÉS i18n
// (`errors.*`, voir `src-tauri/src/errors.rs`) : une phrase en dur côté Rust
// ne serait traduisible dans aucune langue. Les erreurs techniques (E/S,
// SQLite, 7-Zip) restent du texte brut — ce sont des diagnostics, pas des
// conseils, et les tronquer ferait perdre l'information utile au débogage.
import { t } from "./i18n/index.svelte";

const KEY_PREFIX = "errors.";

/** Texte affichable d'une erreur `invoke`, traduite si c'est une clé connue. */
export function errorText(e: unknown): string {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  const trimmed = raw.trim();
  if (!trimmed.startsWith(KEY_PREFIX)) return raw;
  // `t()` renvoie la clé elle-même si elle est absente des locales : on ne
  // risque donc jamais d'afficher une chaîne vide, au pire la clé brute.
  return t(trimmed);
}
