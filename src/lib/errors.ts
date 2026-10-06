// Résolution des erreurs remontées par le backend.
//
// Les erreurs destinées à l'utilisateur voyagent sous forme de CLÉS i18n
// (`errors.*`, voir `src-tauri/src/errors.rs`) : une phrase en dur côté Rust
// ne serait traduisible dans aucune langue. Les erreurs techniques (E/S,
// SQLite, 7-Zip) restent du texte brut — ce sont des diagnostics, pas des
// conseils, et les tronquer ferait perdre l'information utile au débogage.
// A key may carry the values of its placeholders (`$lib/errorKey`).
import { t } from "$lib/i18n/index.svelte";
import { parseErrorKey } from "$lib/errorKey";

/** Texte affichable d'une erreur `invoke`, traduite si c'est une clé connue. */
export function errorText(e: unknown): string {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  const parsed = parseErrorKey(raw);
  if (!parsed) return raw;
  // `t()` renvoie la clé elle-même si elle est absente des locales : on ne
  // risque donc jamais d'afficher une chaîne vide, au pire la clé brute.
  return t(parsed.key, parsed.values);
}
