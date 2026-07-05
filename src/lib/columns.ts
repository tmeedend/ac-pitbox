// Définitions de colonnes de tableau, propres à chaque type.
// La sélection de colonnes visibles est mémorisée indépendamment par type.
import type { ModCard, ModKind } from "./library";
import { t } from "./i18n/index.svelte";

export interface ColumnDef {
  key: string;
  /** Clé i18n du libellé d'en-tête (résolue au rendu, pour rester réactive à la langue). */
  labelKey: string;
  /** Triable par clic d'en-tête. */
  sortable: boolean;
  /** Affichée par défaut (avant tout choix utilisateur). */
  defaultVisible: boolean;
  /** Toujours affichée, absente du sélecteur (colonne essentielle). */
  fixed?: boolean;
  /** Valeur d'affichage ; « — » si la donnée n'existe pas encore. */
  value: (c: ModCard) => string;
  /** Clé de tri (défaut = value en minuscule). */
  sortValue?: (c: ModCard) => string | number;
  /** Rendu en police mono (valeurs techniques/dates). */
  mono?: boolean;
}

const DASH = "—";

/** Date courte AAAA-MM-JJ, « — » si absente. */
function fmtDate(iso: string | null): string {
  return iso ? iso.slice(0, 10) : DASH;
}

/** Taille lisible (Ko/Mo/Go/To, base 1024), « — » si pas encore calculée (§9.4). */
function fmtSize(bytes: number | null): string {
  if (bytes == null) return DASH;
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = -1;
  do {
    v /= 1024;
    i++;
  } while (v >= 1024 && i < units.length - 1);
  return `${v.toFixed(v < 10 ? 2 : v < 100 ? 1 : 0)} ${units[i]}`;
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
      value: (c) => (c.active ? t("common.active").toLowerCase() : DASH),
      sortValue: (c) => (c.active ? 1 : 0),
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
    { key: "added", labelKey: "columns.added", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.created_at), sortValue: (c) => c.created_at },
    { key: "updated", labelKey: "columns.updated", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.updated_at), sortValue: (c) => c.updated_at ?? c.created_at },
    { key: "published", labelKey: "columns.published", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.published_at), sortValue: (c) => c.published_at ?? "" },
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
  { key: "name", labelKey: "columns.name", sortable: true, defaultVisible: true, fixed: true, value: (c) => c.display_name ?? c.id_interne },
  { key: "brand", labelKey: "columns.brand", sortable: true, defaultVisible: true, value: (c) => c.brand ?? DASH },
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
    key: "layouts",
    labelKey: "columns.layouts",
    sortable: true,
    defaultVisible: true,
    // Mono-layout : identifiant réel = "" (voir inspect::track_layouts côté
    // Rust, même convention que uijson::read_track_detail) — affichage seul.
    value: (c) => (c.layouts.length ? c.layouts.map((l) => l || t("detail.defaultLayout")).join(", ") : DASH),
    sortValue: (c) => c.layouts.length,
  },
  { key: "length", labelKey: "columns.length", sortable: true, defaultVisible: false, mono: true, value: () => DASH, sortValue: () => -1 },
  { key: "turns", labelKey: "columns.turns", sortable: true, defaultVisible: false, mono: true, value: () => DASH, sortValue: () => -1 },
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

/** Suffixe de clé localStorage par type. */
export function kindKey(kind: ModKind): string {
  return kind === "Track" ? "tracks" : "cars";
}

/** Charge les clés de colonnes visibles pour un type (repli sur les défauts). */
export function loadVisible(kind: ModKind): Set<string> {
  const defs = columnsFor(kind);
  const raw = localStorage.getItem(`pitbox.cols.${kindKey(kind)}`);
  if (raw) {
    try {
      const keys: string[] = JSON.parse(raw);
      const valid = new Set(defs.map((d) => d.key));
      return new Set(keys.filter((k) => valid.has(k)));
    } catch {
      /* repli sur les défauts */
    }
  }
  return new Set(defs.filter((d) => d.fixed || d.defaultVisible).map((d) => d.key));
}

export function saveVisible(kind: ModKind, keys: Set<string>): void {
  localStorage.setItem(`pitbox.cols.${kindKey(kind)}`, JSON.stringify([...keys]));
}
