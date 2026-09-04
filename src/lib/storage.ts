// Clés `localStorage` historiques — TOUTES LEGACY (§6.2).
//
// Aucune n'est plus réécrite : chaque réglage listé ici vit désormais soit
// dans un fichier JSON dédié écrit côté Rust (duo de session, presets de
// lancement, sessions enregistrées, colonnes de bibliothèque — visibilité,
// ordre et largeur), soit dans `ui_prefs.json` via `uiPrefs.svelte.ts` (tout
// le reste, migré en bloc au premier démarrage après la mise à jour par
// `uiPrefs.svelte.ts::migrateLegacyLocalStorage`). Raison du changement,
// partout la même : `localStorage` n'est pas garanti synchrone sur disque
// côté WebView2, ce qui perdait le réglage le plus récent à la fermeture de
// l'app plutôt qu'au moment du changement (bug réel constaté plusieurs fois :
// le circuit de session, puis la vue galerie/tableau, ne survivaient pas à un
// redémarrage). Voir CLAUDE.md, section « Persistance des petits réglages »,
// pour la règle à suivre sur tout nouveau réglage.
//
// Ce module reste la seule collection de clés — il continue de protéger
// contre une faute de frappe silencieuse dans les lectures de migration
// (`nav.svelte.ts`, `Launch.svelte`, `savedSessions.ts`, `columns.ts`,
// `uiPrefs.svelte.ts`) — mais plus aucune de ces clés n'est écrite.
import type { ModKind } from "./library";

const PREFIX = "pitbox";

/** Suffixe de clé par type d'entité — la bibliothèque est rendue une fois par type. */
export function kindKey(kind: ModKind): string {
  return kind === "Track" ? "tracks" : "cars";
}

export const StorageKey = {
  // --- Migrées vers un fichier Rust dédié (fichier propre à chacune) ---
  sessionCar: `${PREFIX}.session.car`,
  sessionTrack: `${PREFIX}.session.track`,
  launchSelection: `${PREFIX}.launchSel`,
  launchPresets: `${PREFIX}.launchPresets`,
  savedSessions: `${PREFIX}.savedSessions`,
  /** Visibilité seule (jamais l'ordre ni la largeur, fonctionnalités
   * postérieures à cette clé) — le reste repart des défauts à la migration. */
  libraryColumns: (kind: ModKind) => `${PREFIX}.cols.${kindKey(kind)}`,

  // --- Migrées vers `ui_prefs.json` (bulk, `uiPrefs.svelte.ts`) ---
  showFileTags: `${PREFIX}.showFileTags`,
  importCopy: `${PREFIX}.import.copy`,
  transversalGroupBy: `${PREFIX}.transversal.groupBy`,
  transversalSortBy: `${PREFIX}.transversal.sortBy`,
  libraryView: (kind: ModKind) => `${PREFIX}.view.${kindKey(kind)}`,
  librarySortKey: (kind: ModKind) => `${PREFIX}.sort.${kindKey(kind)}.key`,
  librarySortDir: (kind: ModKind) => `${PREFIX}.sort.${kindKey(kind)}.dir`,
  libraryFilters: (kind: ModKind) => `${PREFIX}.filters.${kindKey(kind)}`,
  /** Filtres épinglés (§6.3), et leur ordre. Suffixée par type comme le reste
   * de l'écran, et pas seulement par symétrie : marque, année, classe et
   * pilote n'existent que pour les voitures — une liste partagée aurait posé
   * des fantômes sans objet sur l'écran des circuits. Cette clé-ci n'a jamais
   * connu `localStorage` : elle naît dans `ui_prefs.json`. */
  libraryPinned: (kind: ModKind) => `${PREFIX}.pinned.${kindKey(kind)}`,
  preferredSkin: (carId: string) => `${PREFIX}.skin.${carId}`,
  preferredLayout: (trackId: string) => `${PREFIX}.layout.${trackId}`,
  /** Tenue de pilote choisie pour cette voiture (SPEC-ecran-pilote §1.4).
   * Une clé par voiture, comme le skin préféré : le filtre « pilote modifié »
   * de la bibliothèque la lit par carte, donc de façon synchrone. */
  driverOutfit: (carId: string) => `${PREFIX}.driver.car.${carId}`,
} as const;
