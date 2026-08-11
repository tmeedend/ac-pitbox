// Petits réglages d'interface encore épars (§6.2/§8.6) : vue galerie/tableau
// et tri de la bibliothèque, filtres, regroupement/tri de la vue
// transversale, mode copier/déplacer à l'import, tags de fichier affichés,
// skin/layout préféré par mod. Persistance durable (`ui_prefs.json`, écrit
// côté Rust en synchrone), pas `localStorage` — `localStorage` n'est pas
// garanti synchrone sur disque côté WebView2, ce qui perdait ces réglages à
// la fermeture de l'app plutôt qu'au moment du changement (bug réel constaté :
// la vue galerie/tableau ne survivait pas à un redémarrage). Même mécanisme
// que `session_state.rs`/`saved_sessions.ts`/`columns.ts`.
//
// `.svelte.ts` (pas juste `.ts`) : le cache est un `$state` plutôt qu'une
// variable de module ordinaire — `preferred.ts` le lit de façon *synchrone*
// depuis des expressions de template (`{@const}` par carte de la bibliothèque,
// potentiellement des centaines) ; une variable de module plate ne
// déclencherait aucun re-rendu quand le chargement asynchrone se termine, la
// préférence resterait invisible jusqu'au prochain rendu déclenché par autre
// chose (bug réel évité, pas seulement une préférence de style).
import { invokeSafe } from "./invokeSafe";

const PREFIX = "pitbox.";
// Clés qui vivent déjà dans un fichier Rust dédié (session, presets de
// lancement, sessions enregistrées, colonnes de bibliothèque) : jamais les
// reprendre ici, sous peine de dupliquer une donnée déjà migrée par son
// propre module, ou d'écraser sa version faisant foi avec une copie plus
// ancienne encore traînant en `localStorage`.
const EXCLUDED_PREFIXES = [
  `${PREFIX}session.`,
  `${PREFIX}launchSel`,
  `${PREFIX}launchPresets`,
  `${PREFIX}savedSessions`,
  `${PREFIX}cols.`,
];

/** Migration en bloc (une fois pour toutes les clés, pas une par une) : toute
 * clé `pitbox.*` encore en `localStorage` (hors celles exclues ci-dessus) est
 * reprise telle quelle — valeur brute, jamais réinterprétée ici, chaque
 * appelant continue de la (dé)sérialiser exactement comme il le faisait avec
 * `localStorage.getItem`. */
function migrateLegacyLocalStorage(): Record<string, string> {
  const migrated: Record<string, string> = {};
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (!key || !key.startsWith(PREFIX)) continue;
    if (EXCLUDED_PREFIXES.some((p) => key.startsWith(p))) continue;
    const value = localStorage.getItem(key);
    if (value != null) migrated[key] = value;
  }
  return migrated;
}

function persist(all: Record<string, string>): Promise<void> {
  return invokeSafe<void>("save_ui_prefs", { prefs: all }, undefined);
}

// Chargé une seule fois pour toute la session (plusieurs composants lisent
// des clés différentes du même fichier) : un seul aller-retour Rust, les
// lectures suivantes retombent sur le cache en mémoire.
// `.raw` : jamais de mutation en profondeur, `cache` est toujours remplacé en
// bloc (`cache = updated`) — pas besoin (ni souhaitable) que Svelte proxifie
// récursivement chaque valeur du Record à chaque lecture/écriture.
let cache = $state.raw<Record<string, string> | null>(null);
let loadPromise: Promise<Record<string, string>> | null = null;

function ensureLoaded(): Promise<Record<string, string>> {
  if (cache) return Promise.resolve(cache);
  if (!loadPromise) {
    loadPromise = invokeSafe<Record<string, string>>("get_ui_prefs", undefined, {}).then(
      async (fromRust) => {
        if (Object.keys(fromRust).length > 0) {
          cache = fromRust;
          return fromRust;
        }
        // Rien côté Rust : premier démarrage après la mise à jour, ou tout
        // simplement premier lancement de l'app. Migre ce qu'il y a à migrer,
        // et persiste tout de suite pour ne plus jamais redépendre de
        // `localStorage` (même si la migration ne trouve rien : le fichier
        // existe désormais, on ne retentera pas la migration à chaque coup).
        const migrated = migrateLegacyLocalStorage();
        cache = migrated;
        await persist(migrated);
        return migrated;
      });
  }
  return loadPromise;
}

// Amorcé dès le premier import du module plutôt qu'à la première lecture :
// `peekUiPref` (lecture synchrone, `preferred.ts`) a besoin que le cache soit
// déjà chaud le plus tôt possible, pas seulement à la demande.
void ensureLoaded();

/** Lit un réglage — valeur brute telle que stockée, à charge de l'appelant de
 * la (dé)sérialiser comme il le faisait avec `localStorage.getItem`. */
export async function getUiPref(key: string): Promise<string | null> {
  const all = await ensureLoaded();
  return all[key] ?? null;
}

/** Lit plusieurs réglages d'un coup — pratique pour un écran qui a plusieurs
 * clés à restaurer à l'ouverture (filtres, tri…), un seul aller-retour. */
export async function getUiPrefs(keys: string[]): Promise<Record<string, string | null>> {
  const all = await ensureLoaded();
  return Object.fromEntries(keys.map((k) => [k, all[k] ?? null]));
}

/** Lecture synchrone, réactive : `null` tant que le cache n'a pas fini de
 * charger (fenêtre brève, cache amorcé dès l'import du module — même repli
 * transitoire que `nav.sessionCar`/`sessionTrack`, `null` jusqu'à leur
 * résolution asynchrone). Réservée aux appelants qui ne peuvent pas être
 * asynchrones (expressions de template) ; préférer `getUiPref` sinon. */
export function peekUiPref(key: string): string | null {
  return cache?.[key] ?? null;
}

// Coupe-circuit temporaire de diagnostic (§6.2, bug réel en cours
// d'investigation : `save_ui_prefs` observé en boucle — 285 000 appels et
// ça continuait — sans erreur ni boucle synchrone détectée par Svelte, donc
// probablement un cycle qui passe par une frontière asynchrone, réamorcé à
// chaque tour plutôt que détecté d'un coup). Le compteur se remet à zéro
// après un court silence (pas de rafale = pas un problème) ; s'il explose,
// on log la pile du déclencheur UNE fois puis on arrête d'écrire, plutôt que
// de continuer à marteler le disque et le canal IPC indéfiniment.
let callCount = 0;
let resetTimer: ReturnType<typeof setTimeout> | null = null;
let breakerTripped = false;
const CALL_BURST_LIMIT = 50;
const CALL_BURST_WINDOW_MS = 2000;

/** Écrit un réglage. Asynchrone en interne (recharge-modifie-réécrit le
 * fichier entier, comme les autres modules de persistance) mais l'appel
 * lui-même ne s'attend pas : mêmes usages fire-and-forget que
 * `persistLaunchState`/`persistColumnsPrefs` ailleurs dans le projet. */
export function setUiPref(key: string, value: string): void {
  if (breakerTripped) return;
  callCount++;
  if (resetTimer) clearTimeout(resetTimer);
  resetTimer = setTimeout(() => (callCount = 0), CALL_BURST_WINDOW_MS);
  if (callCount === CALL_BURST_LIMIT) {
    breakerTripped = true;
    console.error(
      `setUiPref("${key}") appelé ${CALL_BURST_LIMIT}+ fois en moins de ${CALL_BURST_WINDOW_MS}ms — coupe-circuit ` +
        `déclenché, plus aucune écriture ui_prefs jusqu'au prochain redémarrage. Pile du dernier appel :`,
    );
    console.trace();
    return;
  }
  void (async () => {
    const all = await ensureLoaded();
    const updated = { ...all, [key]: value };
    cache = updated;
    await persist(updated);
  })();
}
