// The columns of a table view, whatever it lists (SPEC §7.4): which ones show,
// in what order, at what width. Shared by the library (cars, tracks) and the
// Online page, which render through `components/ui/DataTable.svelte`.
// Pure, for Vitest; where they are kept is `tablePrefs.svelte.ts`.

/** What the header needs to know about a column. */
export interface TableColumn {
  key: string;
  /** i18n key of the header, resolved at render so it follows the language. */
  labelKey: string;
  sortable: boolean;
  /** Shown before the user chooses anything. */
  defaultVisible: boolean;
  /** Always shown, and absent from the column chooser. Does **not** mean
   * immobile: a column one cannot hide may still move (`locked` does that). */
  fixed?: boolean;
  /** Neither moved nor moved before: it stays where the default order puts
   * it — the Online page's track, which is what a row is about. */
  locked?: boolean;
  /** Monospaced cells (technical values, dates, figures). */
  mono?: boolean;
  /** i18n key of a ⓘ tooltip on the header, for a column whose meaning is not
   * obvious at a glance. */
  tooltipKey?: string;
  /** Alignment of that tooltip: a column near the right edge grows it left,
   * or it runs over the panel beside the table (reported). */
  tooltipAlign?: "center" | "right";
  /** Natural width in px for a virtualised table, whose rows come and go and
   * cannot size their columns to their content. */
  width?: number;
}

export interface ColumnsPrefs {
  /** Visible keys (fixed columns show in addition, without being listed). */
  visible: string[];
  /** Display order — every column, visible or not, so a hidden one gets its
   * relative place back once shown again. */
  order: string[];
  /** Width in px of the columns resized by hand. Absent = natural width. */
  widths: Record<string, number>;
}

/** The screens that keep their columns. */
export type TableScreen = "cars" | "tracks" | "online";

export function defaultPrefs(defs: TableColumn[]): ColumnsPrefs {
  return {
    visible: defs.filter((d) => d.fixed || d.defaultVisible).map((d) => d.key),
    order: defs.map((d) => d.key),
    widths: {},
  };
}

/** Repairs saved prefs against the current columns: a column removed from the
 * code disappears silently, a column added appears at the end — the spirit of
 * `#[serde(default)]`: a slightly out-of-step file must never break nor
 * freeze. A locked column keeps its default place, whatever was saved. */
export function reconcilePrefs(saved: Partial<ColumnsPrefs> | undefined, defs: TableColumn[]): ColumnsPrefs {
  const valid = new Set(defs.map((d) => d.key));
  const kept = (saved?.order ?? []).filter((k) => valid.has(k));
  let order = [...kept, ...defs.map((d) => d.key).filter((k) => !kept.includes(k))];
  defs.forEach((d, i) => {
    if (!d.locked) return;
    order = order.filter((k) => k !== d.key);
    order.splice(Math.min(i, order.length), 0, d.key);
  });
  return {
    visible: saved?.visible ? saved.visible.filter((k) => valid.has(k)) : defaultPrefs(defs).visible,
    order,
    widths: Object.fromEntries(Object.entries(saved?.widths ?? {}).filter(([k]) => valid.has(k))),
  };
}

/** The columns on screen, in their order. */
export function visibleColumns<C extends TableColumn>(defs: C[], prefs: ColumnsPrefs): C[] {
  return prefs.order
    .map((key) => defs.find((d) => d.key === key))
    .filter((d): d is C => !!d && (!!d.fixed || prefs.visible.includes(d.key)));
}

/** Moves `source` just before or after `target` in the whole order (hidden
 * columns included). A locked column neither moves nor lets anything take its
 * place; the same order comes back when the move is refused. */
export function moveColumn(
  order: string[],
  defs: TableColumn[],
  source: string,
  target: string,
  before: boolean,
): string[] {
  const locked = (k: string) => defs.some((d) => d.key === k && d.locked);
  if (source === target || locked(source) || (locked(target) && before)) return order;
  const rest = order.filter((k) => k !== source);
  const at = rest.indexOf(target);
  if (at < 0) return order;
  const insertAt = before ? at : at + 1;
  return [...rest.slice(0, insertAt), source, ...rest.slice(insertAt)];
}

export function toggleVisible(prefs: ColumnsPrefs, key: string): ColumnsPrefs {
  const visible = prefs.visible.includes(key) ? prefs.visible.filter((k) => k !== key) : [...prefs.visible, key];
  return { ...prefs, visible };
}
