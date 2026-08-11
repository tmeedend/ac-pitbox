// Filet de sécurité générique autour d'`invoke` (§6.2) : une commande Rust
// qui ne répond jamais (bug encore non identifié, verrou, etc.) ne doit plus
// jamais bloquer l'app indéfiniment — bug réel constaté sur `get_ui_prefs`/
// `save_ui_prefs` (écran bibliothèque restant bloqué sur le chargement à
// chaque démarrage, cause exacte non identifiée malgré investigation :
// base SQLite saine, aucun verrou fichier, E/S disque instantanée en test
// isolé). Tant que la cause réelle n'est pas trouvée, mieux vaut un repli
// silencieux sur une valeur par défaut qu'un blocage total de l'app.
import { invoke } from "@tauri-apps/api/core";

const DEFAULT_TIMEOUT_MS = 5000;

/** `invoke` avec repli automatique si la commande ne répond pas dans le délai.
 * Si la commande finit par répondre après coup, la réponse tardive est
 * silencieusement ignorée — l'appelant a déjà basculé sur `fallback`. */
export function invokeSafe<T>(
  cmd: string,
  args: Record<string, unknown> | undefined,
  fallback: T,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      console.error(`invoke(${cmd}) : pas de réponse après ${timeoutMs}ms, repli sur défaut`);
      resolve(fallback);
    }, timeoutMs);
    invoke<T>(cmd, args).then(
      (v) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(v);
      },
      (e) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        console.error(`invoke(${cmd})`, e);
        resolve(fallback);
      },
    );
  });
}
