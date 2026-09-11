// Définitions de colonnes de tableau, propres à chaque type.
// Visibilité et ordre sont mémorisés indépendamment par type (§6.2).
import { invokeSafe } from "./invokeSafe";
import type { ModCard, ModKind } from "./library";
import { t } from "./i18n/index.svelte";
import { fmtSize } from "./format";
import { StorageKey, kindKey } from "./storage";
import { peekUiPref } from "./uiPrefs.svelte";
import { withoutBrand } from "./displayName";

// `kindKey` a déménagé dans storage.ts (il ne servait qu'à bâtir des clés) ;
// ré-exporté ici pour les appelants existants.
export { kindKey };

/** Le tableau retire-t-il la marque du nom ? Vrai par défaut : la colonne
 * « Marque » est visible par défaut elle aussi, et les deux ensemble
 * écriraient la marque deux fois par ligne. */
export function tableHidesBrand(): boolean {
  return peekUiPref(StorageKey.tableHideBrand) !== "0";
}

export interface ColumnDef {
  key: string;
  /** Clé i18n du libellé d'en-tête (résolue au rendu, pour rester réactive à la langue). */
  labelKey: string;
  /** Triable par clic d'en-tête. */
  sortable: boolean;
  /** Affichée par défaut (avant tout choix utilisateur). */
  defaultVisible: boolean;
  /** Toujours affichée et absente du sélecteur (colonne essentielle).
   *
   * **Ne veut plus dire « immobile ».** Les deux allaient ensemble, sans que
   * rien ne l'exige : une colonne qu'on ne peut pas masquer peut très bien se
   * déplacer, et le nom devait pouvoir passer après la marque. */
  fixed?: boolean;
  /** Valeur d'affichage ; « — » si la donnée n'existe pas encore. */
  value: (c: ModCard) => string;
  /** Clé de tri (défaut = value en minuscule). */
  sortValue?: (c: ModCard) => string | number;
  /** Rendu en police mono (valeurs techniques/dates). */
  mono?: boolean;
  /** Clé i18n d'une info-bulle affichée sur l'en-tête (icône ⓘ) — pour une
   * colonne dont le sens n'est pas évident au premier regard (ex. les trois
   * colonnes de dates, dont deux propres à l'installation locale et une au
   * mod lui-même, une distinction facile à manquer). */
  tooltipKey?: string;
}

const DASH = "—";

/** Date courte selon la locale système (jj/MM/aaaa en fr), « — » si absente
 * ou invalide. Les dates sont stockées en ISO/RFC3339 : ceci n'est qu'un
 * formatage d'affichage (le tri utilise toujours la chaîne ISO brute). */
function fmtDate(iso: string | null): string {
  if (!iso) return DASH;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? DASH : d.toLocaleDateString();
}

function allTags(c: ModCard): string[] {
  return [...c.tags_from_mod, ...c.tags_from_rule, ...c.tags_manual];
}

// Colonnes communes à tous les types (dates, auteur, version, tags, état).
// Date de publication : estimée dès l'import depuis les dates de fichiers ;
// une source plus fiable (extension navigateur) la remplacera un jour.
function commonTail(): ColumnDef[] {
  return [
    { key: "author", labelKey: "columns.author", sortable: true, defaultVisible: false, value: (c) => c.author ?? DASH },
    { key: "country", labelKey: "columns.country", sortable: true, defaultVisible: false, value: (c) => c.country ?? DASH },
    { key: "version", labelKey: "columns.version", sortable: true, defaultVisible: false, mono: true, value: (c) => c.active_version_label ?? DASH },
    {
      key: "tags",
      labelKey: "columns.tags",
      sortable: false,
      defaultVisible: false,
      value: (c) => allTags(c).slice(0, 4).join(", ") || DASH,
    },
    {
      key: "active",
      labelKey: "columns.active",
      sortable: true,
      defaultVisible: true,
      // Rendu par `StateBadge` dans le tableau (pastille + libellé) ; ce
      // `value` ne sert que de repli textuel, gardé cohérent avec lui.
      // `sortValue` est inchangé : le tri et le filtre d'état ne dépendent
      // pas de l'affichage — le contenu de base reste `active` pour eux.
      value: (c) =>
        c.is_unmanaged
          ? t("common.unmanagedState")
          : c.is_stock
            ? t("common.stockState")
            : c.active
              ? t("common.active")
              : t("common.inactive"),
      sortValue: (c) => (c.active ? 1 : 0),
    },
    {
      key: "note",
      labelKey: "columns.note",
      sortable: true,
      defaultVisible: false,
      // Le texte, pas un simple oui/non : une colonne qui dirait seulement
      // « oui » obligerait à ouvrir chaque fiche pour savoir laquelle on
      // cherche. Le tri met les mods annotés en tête, ce qui est la question
      // qu'on pose à cette colonne.
      value: (c) => c.notes_user?.replace(/\s+/g, " ").trim() || DASH,
      sortValue: (c) => (c.notes_user ? 0 : 1),
    },
    {
      key: "distance",
      labelKey: "columns.distance",
      sortable: true,
      defaultVisible: false,
      mono: true,
      // Km CM si connus ; sinon « essayé » (marqueur app) ou « — ».
      value: (c) => (c.distance_km != null ? `${c.distance_km.toFixed(1)} km` : c.tried ? t("library.tried") : DASH),
      // Tri : km croissants font remonter les peu/pas explorés ; jamais essayé en tête.
      sortValue: (c) => (c.distance_km ?? (c.tried ? 0 : -1)),
    },
    { key: "added", labelKey: "columns.added", tooltipKey: "columns.addedTooltip", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.created_at), sortValue: (c) => c.created_at ?? "" },
    { key: "updated", labelKey: "columns.updated", tooltipKey: "columns.updatedTooltip", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.updated_at), sortValue: (c) => c.updated_at ?? c.created_at ?? "" },
    { key: "published", labelKey: "columns.published", tooltipKey: "columns.publishedTooltip", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.published_at), sortValue: (c) => c.published_at ?? "" },
    {
      key: "size",
      labelKey: "columns.size",
      sortable: true,
      defaultVisible: false,
      mono: true,
      // Somme de toutes les versions en bibliothèque (§9.4) ; « — » tant que non
      // calculée (mod importé avant cette fonctionnalité, cf. Maintenance).
      value: (c) => fmtSize(c.size_bytes),
      sortValue: (c) => c.size_bytes ?? -1,
    },
  ];
}

const CAR_COLUMNS: ColumnDef[] = [
  // **La marque en tête, le nom ensuite.** C'est l'ordre dans lequel on cherche
  // une voiture, le même que sur la carte de la grille — et cet ordre-là n'est
  // qu'un défaut : la colonne se déplace comme les autres.
  { key: "brand", labelKey: "columns.brand", sortable: true, defaultVisible: true, value: (c) => c.brand ?? DASH },
  {
    key: "name",
    // « Modèle » et non « Nom », puisque la colonne d'à côté porte la marque et
    // que le nom en est désormais privé. Clé propre aux voitures : un circuit a
    // un nom, pas un modèle, et les deux types partageaient le même libellé.
    labelKey: "columns.model",
    sortable: true,
    defaultVisible: true,
    fixed: true,
    // La marque retirée du nom quand la colonne d'à côté la porte déjà —
    // « Nissan | Nissan Skyline GT-R R34 » écrit deux fois la même chose. Le
    // réglage est celui du TABLEAU, distinct de celui des grilles : deux
    // présentations, deux décisions.
    //
    // Lu ici et pas au rendu pour que le TRI suive ce qui est affiché : une
    // colonne triée sur un texte qu'on ne voit pas se lit comme non triée.
    // `peekUiPref` est fait pour ça — lecture synchrone, une fois par ligne.
    value: (c) => (tableHidesBrand() ? withoutBrand(c.display_name ?? c.id_interne, c.brand) : (c.display_name ?? c.id_interne)),
  },
  { key: "category", labelKey: "columns.category", sortable: true, defaultVisible: true, value: (c) => c.category ?? DASH },
  { key: "car_class", labelKey: "columns.carClass", sortable: true, defaultVisible: false, value: (c) => c.car_class ?? DASH },
  { key: "year", labelKey: "columns.year", sortable: true, defaultVisible: true, mono: true, value: (c) => c.year?.toString() ?? DASH, sortValue: (c) => c.year ?? 0 },
  { key: "weight", labelKey: "columns.weight", sortable: true, defaultVisible: false, mono: true, value: (c) => c.weight ?? DASH },
  { key: "drivetrain", labelKey: "columns.drivetrain", sortable: true, defaultVisible: false, value: (c) => c.drivetrain ?? DASH },
  { key: "gearbox", labelKey: "columns.gearbox", sortable: true, defaultVisible: false, value: (c) => c.gearbox ?? DASH },
  { key: "engine_config", labelKey: "columns.engineConfig", sortable: true, defaultVisible: false, value: (c) => c.engine_config ?? DASH },
  { key: "engine_pos", labelKey: "columns.enginePos", sortable: true, defaultVisible: false, value: (c) => c.engine_pos ?? DASH },
  { key: "aspiration", labelKey: "columns.aspiration", sortable: true, defaultVisible: false, value: (c) => c.aspiration ?? DASH },
  ...commonTail(),
];

const TRACK_COLUMNS: ColumnDef[] = [
  { key: "name", labelKey: "columns.name", sortable: true, defaultVisible: true, fixed: true, value: (c) => c.display_name ?? c.id_interne },
  {
    key: "category",
    labelKey: "columns.category",
    sortable: true,
    defaultVisible: true,
    // Multi-valué (§5bis.2), ordonné par priorité ; la 1ʳᵉ = catégorie principale (tri).
    value: (c) => (c.categories.length ? c.categories.join(" · ") : DASH),
    sortValue: (c) => c.categories[0] ?? "",
  },
  {
    key: "layouts",
    labelKey: "columns.layouts",
    sortable: true,
    defaultVisible: true,
    // Mono-layout : identifiant réel = "" (voir inspect::track_layouts côté
    // Rust, même convention que uijson::read_track_detail) — affichage seul.
    value: (c) => (c.layouts.length ? c.layouts.map((l) => l || t("detail.defaultLayout")).join(", ") : DASH),
    sortValue: (c) => c.layouts.length,
  },
  {
    key: "csp",
    labelKey: "columns.csp",
    sortable: false,
    defaultVisible: true,
    value: (c) => (c.csp_features.length ? c.csp_features.join(" · ") : DASH),
  },
  ...commonTail(),
];

export function columnsFor(kind: ModKind): ColumnDef[] {
  return kind === "Track" ? TRACK_COLUMNS : CAR_COLUMNS;
}

export interface ColumnsPrefs {
  /** Clés visibles (les colonnes `fixed` sont toujours affichées en plus, sans y figurer). */
  visible: string[];
  /** Ordre d'affichage — toutes les colonnes du type, visibles ou non, pour
   * qu'une colonne masquée retrouve sa position relative une fois réaffichée. */
  order: string[];
  /** Largeur en pixels des colonnes redimensionnées à la main (glissé sur la
   * poignée d'en-tête). Absente d'une clé = largeur naturelle (au contenu). */
  widths: Record<string, number>;
}

function defaultOrder(kind: ModKind): string[] {
  return columnsFor(kind).map((d) => d.key);
}

function defaultVisibleKeys(kind: ModKind): string[] {
  return columnsFor(kind)
    .filter((d) => d.fixed || d.defaultVisible)
    .map((d) => d.key);
}

/** Répare un ordre persisté face à une évolution du jeu de colonnes (une
 * colonne retirée du code disparaît silencieusement, une colonne ajoutée
 * apparaît en fin de liste) — même esprit que `#[serde(default)]` côté Rust :
 * un format légèrement désynchronisé ne doit jamais planter ni se figer. */
function reconcileOrder(saved: string[] | undefined, kind: ModKind): string[] {
  const validKeys = columnsFor(kind).map((d) => d.key);
  const validSet = new Set(validKeys);
  const kept = (saved ?? []).filter((k) => validSet.has(k));
  const missing = validKeys.filter((k) => !kept.includes(k));
  return [...kept, ...missing];
}

/** Ancien mécanisme (avant fix) : lu une seule fois pour migrer la visibilité
 * déjà choisie, jamais réécrit. `localStorage` n'est pas garanti synchrone
 * sur disque côté WebView2 — voir `session_state.rs` pour le pourquoi du
 * changement. Ne portait que la visibilité, jamais l'ordre (fonctionnalité
 * nouvelle) : une migration ne restaure donc que `visible`, `order` repart de
 * l'ordre par défaut.
 */
function loadLegacyVisible(kind: ModKind): string[] | null {
  const raw = localStorage.getItem(StorageKey.libraryColumns(kind));
  if (!raw) return null;
  try {
    const keys: string[] = JSON.parse(raw);
    return Array.isArray(keys) ? keys : null;
  } catch {
    return null;
  }
}

interface AllColumnsPrefs {
  cars?: ColumnsPrefs;
  tracks?: ColumnsPrefs;
}

/** Persistance durable (§6.2) : fichier écrit côté Rust
 * (`library_columns.json`, `std::fs::write` synchrone), pas `localStorage` —
 * voir `loadLegacyVisible` pour le pourquoi du changement. Un seul fichier
 * pour les deux types (clé `cars`/`tracks`) : chaque sauvegarde réécrit tout
 * le fichier, donc on recharge-modifie-réécrit l'objet entier à chaque appel,
 * jamais une écriture partielle qui effacerait l'autre type. */
function loadAllPrefs(): Promise<AllColumnsPrefs> {
  return invokeSafe<AllColumnsPrefs>("get_library_columns", undefined, {});
}

function persistAllPrefs(all: AllColumnsPrefs): Promise<void> {
  return invokeSafe<void>("save_library_columns", { prefs: all }, undefined);
}

/** Charge visibilité + ordre pour un type, avec repli sur l'ancienne clé
 * `localStorage` (visibilité seule) puis sur les défauts. */
export async function loadColumnsPrefs(kind: ModKind): Promise<ColumnsPrefs> {
  const all = await loadAllPrefs();
  const saved = kindKey(kind) === "tracks" ? all.tracks : all.cars;
  if (saved) {
    const validSet = new Set(columnsFor(kind).map((d) => d.key));
    return {
      visible: (saved.visible ?? []).filter((k) => validSet.has(k)),
      order: reconcileOrder(saved.order, kind),
      widths: Object.fromEntries(Object.entries(saved.widths ?? {}).filter(([k]) => validSet.has(k))),
    };
  }
  const legacy = loadLegacyVisible(kind);
  const validSet = new Set(columnsFor(kind).map((d) => d.key));
  return {
    visible: legacy ? legacy.filter((k) => validSet.has(k)) : defaultVisibleKeys(kind),
    order: defaultOrder(kind),
    widths: {},
  };
}

export async function saveColumnsPrefs(kind: ModKind, prefs: ColumnsPrefs): Promise<void> {
  const all = await loadAllPrefs();
  if (kindKey(kind) === "tracks") all.tracks = prefs;
  else all.cars = prefs;
  await persistAllPrefs(all);
}
