// Clés `localStorage`, rassemblées en un seul endroit.
//
// Pourquoi un module dédié : une clé mal orthographiée ne casse rien. L'écriture
// part dans le vide et la lecture retombe sur le défaut — le réglage se « perd »
// silencieusement, sans erreur ni au typage ni à l'exécution. Les centraliser
// est le seul moyen d'en faire une faute de compilation.
//
// Les clés paramétrées (par type d'entité, par mod) sont des **fonctions** :
// c'est là que se jouait le vrai risque. `Library` et `columns.ts` sont rendus
// une fois par type, et un suffixe oublié fait silencieusement partager le
// réglage entre voitures et circuits.
import type { ModKind } from "./library";

const PREFIX = "pitbox";

/** Suffixe de clé par type d'entité — la bibliothèque est rendue une fois par type. */
export function kindKey(kind: ModKind): string {
  return kind === "Track" ? "tracks" : "cars";
}

export const StorageKey = {
  // --- Réglages globaux ---
  /** Tags issus du fichier mod affichés ou masqués (§5). */
  showFileTags: `${PREFIX}.showFileTags`,
  /** Import : copier (défaut) plutôt que déplacer la source (§4.5). */
  importCopy: `${PREFIX}.import.copy`,

  // --- Duo de session, écran de lancement (§8.6/§8.4bis) — LEGACY ---
  // Tout ceci vit désormais dans des fichiers écrits côté Rust
  // (`session_state.rs`, `saved_sessions.rs`), pas ici : `localStorage` n'est
  // pas garanti synchrone sur disque côté WebView2, ce qui perdait le réglage
  // le plus récent à la fermeture de l'app plutôt qu'au moment du changement.
  // Ces clés ne sont plus lues qu'une fois, en migration (`nav.svelte.ts`,
  // `Launch.svelte`, `savedSessions.ts`) — jamais réécrites.
  sessionCar: `${PREFIX}.session.car`,
  sessionTrack: `${PREFIX}.session.track`,
  /** Dernière sélection de l'écran de session. */
  launchSelection: `${PREFIX}.launchSel`,
  /** Réglages de session mémorisés par type de session. */
  launchPresets: `${PREFIX}.launchPresets`,
  /** Sessions nommées enregistrées par l'utilisateur. */
  savedSessions: `${PREFIX}.savedSessions`,

  // --- Vues transversales / add-ons (§12bis.3) ---
  transversalGroupBy: `${PREFIX}.transversal.groupBy`,
  transversalSortBy: `${PREFIX}.transversal.sortBy`,

  // --- Bibliothèque : un réglage par type d'entité ---
  /** Vue galerie ou tableau. */
  libraryView: (kind: ModKind) => `${PREFIX}.view.${kindKey(kind)}`,
  /** Colonnes visibles en vue tableau. */
  libraryColumns: (kind: ModKind) => `${PREFIX}.cols.${kindKey(kind)}`,
  /** Colonne de tri. */
  librarySortKey: (kind: ModKind) => `${PREFIX}.sort.${kindKey(kind)}.key`,
  /** Sens de tri (`"1"` / `"-1"`). */
  librarySortDir: (kind: ModKind) => `${PREFIX}.sort.${kindKey(kind)}.dir`,
  /** Filtres avancés. */
  libraryFilters: (kind: ModKind) => `${PREFIX}.filters.${kindKey(kind)}`,

  // --- Préférences par entité (§8.6) ---
  /** Dernier skin choisi pour une voiture. */
  preferredSkin: (carId: string) => `${PREFIX}.skin.${carId}`,
  /** Dernier layout choisi pour un circuit. */
  preferredLayout: (trackId: string) => `${PREFIX}.layout.${trackId}`,
} as const;
