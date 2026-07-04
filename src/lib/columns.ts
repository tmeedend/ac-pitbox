// Définitions de colonnes de tableau, propres à chaque type (§6.2).
// La sélection de colonnes visibles est mémorisée indépendamment par type.
import type { ModCard, ModKind } from "./library";

export interface ColumnDef {
  key: string;
  label: string;
  /** Triable par clic d'en-tête. */
  sortable: boolean;
  /** Affichée par défaut (avant tout choix utilisateur). */
  defaultVisible: boolean;
  /** Toujours affichée, absente du sélecteur (colonne essentielle). */
  fixed?: boolean;
  /** Valeur d'affichage ; « — » si la donnée n'existe pas encore (§6.2). */
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

function allTags(c: ModCard): string[] {
  return [...c.tags_from_mod, ...c.tags_from_rule, ...c.tags_manual];
}

// Colonnes communes à tous les types (dates, auteur, version, tags, état).
// distance → §6.5. Date de publication : estimée dès l'import depuis les
// dates de fichiers (§6.2) ; une source plus fiable (extension L7) la remplacera.
function commonTail(): ColumnDef[] {
  return [
    { key: "author", label: "Auteur", sortable: true, defaultVisible: false, value: (c) => c.author ?? DASH },
    { key: "country", label: "Pays", sortable: true, defaultVisible: false, value: (c) => c.country ?? DASH },
    { key: "version", label: "Version", sortable: true, defaultVisible: false, mono: true, value: (c) => c.active_version_label ?? DASH },
    {
      key: "tags",
      label: "Tags",
      sortable: false,
      defaultVisible: false,
      value: (c) => allTags(c).slice(0, 4).join(", ") || DASH,
    },
    {
      key: "active",
      label: "État",
      sortable: true,
      defaultVisible: true,
      value: (c) => (c.active ? "actif" : DASH),
      sortValue: (c) => (c.active ? 1 : 0),
    },
    {
      key: "distance",
      label: "Distance",
      sortable: true,
      defaultVisible: false,
      mono: true,
      // §6.5 : km CM si connus ; sinon « essayé » (marqueur app) ou « — ».
      value: (c) => (c.distance_km != null ? `${c.distance_km.toFixed(1)} km` : c.tried ? "essayé" : DASH),
      // Tri : km croissants font remonter les peu/pas explorés ; jamais essayé en tête.
      sortValue: (c) => (c.distance_km ?? (c.tried ? 0 : -1)),
    },
    { key: "added", label: "Date d'ajout", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.created_at), sortValue: (c) => c.created_at },
    { key: "updated", label: "Date de MAJ", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.updated_at), sortValue: (c) => c.updated_at ?? c.created_at },
    { key: "published", label: "Date de publication", sortable: true, defaultVisible: false, mono: true, value: (c) => fmtDate(c.published_at), sortValue: (c) => c.published_at ?? "" },
  ];
}

const CAR_COLUMNS: ColumnDef[] = [
  { key: "name", label: "Nom", sortable: true, defaultVisible: true, fixed: true, value: (c) => c.display_name ?? c.id_interne },
  { key: "brand", label: "Marque", sortable: true, defaultVisible: true, value: (c) => c.brand ?? DASH },
  { key: "category", label: "Catégorie", sortable: true, defaultVisible: true, value: (c) => c.category ?? DASH },
  { key: "car_class", label: "Classe", sortable: true, defaultVisible: false, value: (c) => c.car_class ?? DASH },
  { key: "year", label: "Année", sortable: true, defaultVisible: true, mono: true, value: (c) => c.year?.toString() ?? DASH, sortValue: (c) => c.year ?? 0 },
  { key: "weight", label: "Poids", sortable: true, defaultVisible: false, mono: true, value: (c) => c.weight ?? DASH },
  { key: "drivetrain", label: "Transmission", sortable: true, defaultVisible: false, value: (c) => c.drivetrain ?? DASH },
  { key: "gearbox", label: "Boîte", sortable: true, defaultVisible: false, value: (c) => c.gearbox ?? DASH },
  { key: "engine_config", label: "Moteur", sortable: true, defaultVisible: false, value: (c) => c.engine_config ?? DASH },
  { key: "engine_pos", label: "Position moteur", sortable: true, defaultVisible: false, value: (c) => c.engine_pos ?? DASH },
  { key: "aspiration", label: "Admission", sortable: true, defaultVisible: false, value: (c) => c.aspiration ?? DASH },
  ...commonTail(),
];

const TRACK_COLUMNS: ColumnDef[] = [
  { key: "name", label: "Nom", sortable: true, defaultVisible: true, fixed: true, value: (c) => c.display_name ?? c.id_interne },
  {
    key: "layouts",
    label: "Layouts",
    sortable: true,
    defaultVisible: true,
    value: (c) => (c.layouts.length ? c.layouts.join(", ") : DASH),
    sortValue: (c) => c.layouts.length,
  },
  { key: "length", label: "Longueur", sortable: true, defaultVisible: false, mono: true, value: () => DASH, sortValue: () => -1 },
  { key: "turns", label: "Virages", sortable: true, defaultVisible: false, mono: true, value: () => DASH, sortValue: () => -1 },
  {
    key: "csp",
    label: "Extensions CSP",
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
