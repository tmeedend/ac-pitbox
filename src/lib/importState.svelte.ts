// État d'import partagé (§4.2) : le glisser-déposer fonctionne partout dans
// l'app (pas seulement dans une bibliothèque), donc la progression, le rapport
// et l'arbitrage des conflits flous vivent ici plutôt que dans Library.svelte.
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { t } from "./i18n/index.svelte";
import { StorageKey } from "./storage";
import { getUiPref, setUiPref } from "./uiPrefs.svelte";
import { bumpLibraryVersion } from "./libraryVersion.svelte";
import { listPendingFolders } from "./pending";
import {
  cancelImport,
  importArchives,
  importFolders,
  resolveConflict,
  splitDroppedPaths,
  type ArchiveResult,
  type ImportProgress,
  type ImportDecision,
  type ModKind,
} from "$lib/library";

export interface PendingConflict {
  newId: string;
  newName: string;
  oldId: string;
  oldName: string;
}

/** Import ambigu à trancher (§4.4) : mise à jour ou extension ? */
export interface PendingAmbiguous {
  id: string;
  name: string;
  added: number;
  overwritten: number;
  total: number;
  /** Source à ré-importer pour appliquer la décision. */
  source: { paths: string[]; folder: boolean; copy: boolean };
}

/** Fragment dont l'hôte manque, à trancher (§4.3bis). Rien n'a été écrit : le
 * dossier est une couche déguisée en mod, et le circuit/la voiture qu'il vise
 * n'est pas là. `hostId` renseigné = on sait quoi attendre, donc on peut le
 * garder de côté ; absent = il ne reste que « laisser tomber » ou « importer
 * quand même », qui produira une entrée que le jeu ne saura pas charger. */
export interface PendingFragment {
  id: string;
  name: string;
  hostId: string | null;
  kind: ModKind;
  /** Livrée ou son plutôt que dossier de mod : la question est la même, le
   * texte non — une livrée n'a pas de "modèle 3D manquant" à expliquer. */
  isSub: boolean;
  /** Source à ré-importer pour appliquer la décision. */
  source: { paths: string[]; folder: boolean; copy: boolean };
}

/** One import report in the notification stack (§4.2bis).
 *
 * A stack, not a single slot: a second import used to overwrite the first
 * one's report — and its toast was drawn at the very same corner, so it hid it
 * anyway. Reports now pile up, and only the newest one is unfolded. */
export interface ReportEntry {
  /** Render key and arrival order. */
  id: number;
  report: ArchiveResult[];
  /** Folded = header line only. One unfolded report at a time. */
  collapsed: boolean;
}

/** Beyond that, the oldest ones drop out: a column of ten header bars eats the
 * screen the stack is meant to leave free, and a report still unread after
 * three imports will not be read. The last one stays available on the Import
 * screen regardless. */
const MAX_REPORTS = 3;

export const importState = $state<{
  importing: boolean;
  progress: ImportProgress | null;
  reports: ReportEntry[];
  /** Dernier rapport, conservé après fermeture du toast (§4.2bis) : un import
   * de quarante mods ne doit pas disparaître sur un clic réflexe sur ✕.
   * En mémoire seulement — il ne survit pas à un redémarrage, et n'a pas à le
   * faire : c'est le compte rendu d'une action, pas un réglage. */
  lastReport: ArchiveResult[] | null;
  pendingConflicts: PendingConflict[];
  pendingAmbiguous: PendingAmbiguous[];
  pendingFragments: PendingFragment[];
  copyMode: boolean;
  /** Arrêt demandé (§4.2bis) : le lot s'interrompt entre deux items, donc il
   * reste du travail en cours après le clic — le bouton doit le dire. */
  cancelling: boolean;
  /** Écran d'arbitrage des dossiers proposés (§4.6ter) ouvert. S'ouvre tout
   * seul en fin de lot quand il y a quelque chose à trancher, et **jamais
   * pendant** : un lot de cinquante mods ne s'interrompt pas. Fermer ne décide
   * rien — les dossiers restent en attente et la ligne du rapport y ramène. */
  pendingOpen: boolean;
  /** Combien de dossiers attendent encore, **partagé** entre le bandeau du
   * rapport et la modale. Deux compteurs locaux avaient divergé dès le premier
   * arbitrage : la modale se vidait, le bandeau continuait d'annoncer un
   * dossier à trancher, et le bouton n'ouvrait plus rien. */
  pendingCount: number;
}>({
  importing: false,
  progress: null,
  reports: [],
  lastReport: null,
  pendingConflicts: [],
  pendingAmbiguous: [],
  pendingFragments: [],
  // Défaut synchrone (état module-level, pas de composant/onMount ici),
  // corrigé de façon asynchrone juste en dessous dès que la valeur
  // sauvegardée répond (§6.2, même schéma que `nav.svelte.ts`).
  copyMode: true,
  cancelling: false,
  pendingOpen: false,
  pendingCount: 0,
});

/** État de progression initial, avant le premier événement backend. */
function queuedProgress(count: number): ImportProgress {
  return {
    item_index: 0,
    item_count: count,
    overall_ratio: 0,
    item_ratio: 0,
    eta_secs: null,
    archive: "",
    phase: "queued",
    sub_current: 0,
    sub_total: 0,
    label: "",
  };
}

/** Demande l'arrêt du lot en cours (§4.2bis). */
export function requestCancelImport(): void {
  importState.cancelling = true;
  cancelImport().catch((e) => console.error("cancel_import", e));
}

getUiPref(StorageKey.importCopy).then((v) => {
  if (v != null) importState.copyMode = v !== "false";
});

export function setCopyMode(v: boolean): void {
  importState.copyMode = v;
  setUiPref(StorageKey.importCopy, String(v));
}

/** Lance un import et récolte conflits flous + cas ambigus (§4.2/§4.4). La
 * `source` est mémorisée pour pouvoir reprendre un cas ambigu après décision. */
async function runImport(source: { paths: string[]; folder: boolean; copy: boolean }): Promise<void> {
  // Un seul lot à la fois : le backend n'a qu'un drapeau d'annulation et qu'un
  // état de progression, deux lots concurrents se marcheraient dessus. Les
  // boutons sont déjà désactivés pendant un import, mais pas le glisser-déposer.
  if (importState.importing) return;
  importState.importing = true;
  importState.cancelling = false;
  // Retour immédiat (§4.2) : la commande est asynchrone côté backend et met
  // un instant à démarrer réellement le traitement — sans cet état "queued",
  // le toast de progression (conditionné sur `progress` non nul) resterait
  // invisible pendant ce court laps, donnant l'impression que le drop n'a rien
  // fait. Remplacé dès le premier événement `import:progress` réel.
  importState.progress = queuedProgress(source.paths.length);
  try {
    const report = source.folder
      ? await importFolders(source.paths, source.copy)
      : await importArchives(source.paths);
    pushReport(report);
    importState.pendingConflicts = report.flatMap((a) =>
      a.mods
        .filter((m) => m.conflict)
        .map((m) => ({
          newId: m.id_interne,
          newName: m.display_name ?? m.id_interne,
          oldId: m.conflict!.existing_id,
          oldName: m.conflict!.existing_name ?? m.conflict!.existing_id,
        })),
    );
    // Une entrée du rapport par source, dans l'ordre du lot : `report[i]`
    // correspond donc à `source.paths[i]`. On ne mémorise que CETTE source-là
    // pour la reprise — renvoyer le lot entier ferait re-décompresser quarante
    // archives pour trancher un mod (§4.4).
    importState.pendingAmbiguous = report.flatMap((a, i) =>
      a.mods
        .filter((m) => m.outcome === "AMBIGUOUS")
        .map((m) => ({
          id: m.id_interne,
          name: m.display_name ?? m.id_interne,
          added: m.added_count ?? 0,
          overwritten: m.overwritten_count ?? 0,
          total: m.existing_total ?? 0,
          source: { ...source, paths: [source.paths[i] ?? source.paths[0]] },
        })),
    );
    // Fragments dont l'hôte manque (§4.3bis) : même mécanique de reprise que
    // ci-dessus, seule la question posée change.
    //
    // Les livrées et les sons sans leur voiture y sont joints : c'est la même
    // question — « ce contenu se pose DANS quelque chose que tu n'as pas » —
    // donc la même fenêtre. Leur clé porte un `/`, jamais présent dans un id de
    // mod, ce qui les distingue côté backend.
    importState.pendingFragments = report.flatMap((a, i) => {
      const src = { ...source, paths: [source.paths[i] ?? source.paths[0]] };
      const mods = a.mods
        .filter((m) => m.outcome === "HOST_MISSING" || m.outcome === "HOST_UNKNOWN")
        .map((m) => ({
          id: m.id_interne,
          name: m.display_name ?? m.id_interne,
          hostId: m.host_id ?? null,
          kind: m.kind,
          isSub: false,
          source: src,
        }));
      const subs = (a.subs ?? [])
        .filter((s) => s.awaiting_decision)
        .map((s) => ({
          id: `${s.parent_id}/${s.name}`,
          name: s.name,
          hostId: s.parent_id,
          kind: (s.sub_type === "TRACK_SKIN" ? "Track" : "Car") as ModKind,
          isSub: true,
          source: src,
        }));
      return [...mods, ...subs];
    });
    bumpLibraryVersion();
  } finally {
    importState.importing = false;
    importState.cancelling = false;
    importState.progress = null;
  }
}

export async function pickAndImportArchive(): Promise<void> {
  const sel = await open({
    multiple: true,
    filters: [{ name: "Archives", extensions: ["zip", "rar", "7z", "kn5"] }],
  });
  if (!sel) return;
  // Le tri revient au backend, comme pour le glisser-déposer : un `.kn5` de
  // pilote se choisit ici (c'est ainsi qu'ils se téléchargent, sans archive
  // autour) mais s'importe par le chemin des dossiers, qui sait le mettre en
  // boîte. Deux appels d'import concurrents étant exclus, les archives
  // passent d'abord — la même règle qu'au dépôt.
  const picked = Array.isArray(sel) ? sel : [sel];
  const { archives, folders } = await splitDroppedPaths(picked);
  if (archives.length) {
    await runImport({ paths: archives, folder: false, copy: false });
  } else if (folders.length) {
    await runImport({ paths: folders, folder: true, copy: importState.copyMode });
  }
}

export async function pickAndImportFolder(): Promise<void> {
  const sel = await open({ directory: true, multiple: false });
  if (!sel || typeof sel !== "string") return;
  await runImport({ paths: [sel], folder: true, copy: importState.copyMode });
}

/** Tranche un cas ambigu (§4.4) : ré-importe la même source en forçant la
 * décision pour cet id. Les mods déjà importés reviennent en « doublon ». */
export async function resolveAmbiguous(
  item: PendingAmbiguous,
  decision: "update" | "extension",
): Promise<void> {
  importState.pendingAmbiguous = importState.pendingAmbiguous.filter((p) => p.id !== item.id);
  await resumeWithDecision(item, decision);
}

/** Tranche un fragment dont l'hôte manque (§4.3bis). « skip » ne réimporte
 * rien : rien n'avait été écrit, il n'y a donc rien à défaire — la ligne du
 * rapport reste, elle dit ce qui a été écarté et pourquoi. */
export async function resolveFragment(
  item: PendingFragment,
  decision: "park" | "standalone" | "skip",
): Promise<void> {
  importState.pendingFragments = importState.pendingFragments.filter((p) => p.id !== item.id);
  if (decision === "skip") return;
  await resumeWithDecision(item, decision);
}

/** Rejoue une source en forçant une décision sur un seul de ses mods, et
 * remplace sa ligne dans les rapports affichés. Commun aux deux arbitrages :
 * la question diffère, la reprise est la même. */
async function resumeWithDecision(
  item: { id: string; source: { paths: string[]; folder: boolean; copy: boolean } },
  decision: ImportDecision["decision"],
): Promise<void> {
  importState.importing = true;
  importState.cancelling = false;
  importState.progress = queuedProgress(item.source.paths.length);
  try {
    const decisions = [{ id: item.id, decision }];
    const report = item.source.folder
      ? await importFolders(item.source.paths, item.source.copy, decisions)
      : await importArchives(item.source.paths, decisions);
    // Fusionne : remplace la ligne du mod tranché dans le rapport affiché.
    const resolved = report.flatMap((a) => a.mods).find((m) => m.id_interne === item.id);
    if (resolved) {
      // Toute la pile, pas seulement le dernier rapport : le même mod peut
      // figurer dans un lot précédent encore affiché. Le tableau est partagé
      // avec `lastReport`, donc l'écran Import en rend compte lui aussi.
      for (const entry of importState.reports) {
        for (const a of entry.report) {
          const i = a.mods.findIndex((m) => m.id_interne === item.id);
          if (i >= 0) a.mods[i] = resolved;
        }
      }
    }
    // Même remplacement pour un sous-élément tranché, repéré par sa clé
    // `<parent>/<nom>` : sans ça la ligne du rapport continuerait d'annoncer
    // « en attente » une livrée qu'on vient de ranger.
    const sub = report.flatMap((a) => a.subs ?? []).find((s) => `${s.parent_id}/${s.name}` === item.id);
    if (sub) {
      for (const entry of importState.reports) {
        for (const a of entry.report) {
          const i = (a.subs ?? []).findIndex((s) => `${s.parent_id}/${s.name}` === item.id);
          if (i >= 0) a.subs[i] = sub;
        }
      }
    }
    bumpLibraryVersion();
  } finally {
    importState.importing = false;
    importState.cancelling = false;
    importState.progress = null;
  }
}

export async function resolvePendingConflict(
  c: PendingConflict,
  action: "keep_both" | "replace",
): Promise<void> {
  await resolveConflict(c.newId, c.oldId, action);
  importState.pendingConflicts = importState.pendingConflicts.filter((p) => p !== c);
  bumpLibraryVersion();
}

let nextReportId = 1;

/** Pousse un rapport sur la pile. `lastReport` garde la même référence que
 * l'entrée : trancher un cas ambigu modifie le rapport en place, et les deux
 * vues (toast et écran Import) doivent en rendre compte. */
function pushReport(report: ArchiveResult[]): void {
  // Un seul déplié à la fois : le nouveau. Les précédents restent atteignables
  // en un clic sur leur bandeau — deux rapports de quarante lignes dépliés
  // côte à côte ne tiennent pas dans la hauteur d'écran.
  for (const e of importState.reports) e.collapsed = true;
  importState.reports.push({ id: nextReportId++, report, collapsed: false });
  const excess = importState.reports.length - MAX_REPORTS;
  if (excess > 0) importState.reports.splice(0, excess);
  importState.lastReport = report;
  // Fin de lot : c'est le moment convenu pour poser la question (§4.6). Le
  // rapport dit seulement qu'il y a **quelque chose** à trancher ; le compte,
  // lui, est relu en base — un dossier laissé en attente par un lot précédent
  // doit reparaître ici.
  if (report.some((a) => (a.pending ?? 0) > 0)) importState.pendingOpen = true;
  void refreshPendingCount();
}

/** Ouvre l'écran d'arbitrage des dossiers proposés (§4.6ter). */
export function openPendingDialog(): void {
  importState.pendingOpen = true;
}

/** Le referme sans rien décider — une réponse valable en soi. */
export function closePendingDialog(): void {
  importState.pendingOpen = false;
}

/** Relit en base combien de dossiers attendent, et referme la modale s'il n'y
 * a plus rien à y voir. Appelé après chaque arbitrage et à chaque nouveau
 * rapport : c'est la base qui fait foi, jamais un compte gardé en mémoire —
 * un dossier laissé en attente par un lot précédent compte lui aussi. */
export async function refreshPendingCount(): Promise<void> {
  try {
    importState.pendingCount = (await listPendingFolders()).length;
  } catch {
    // Pas de liste : rien à trancher. Ce n'est pas une erreur à montrer.
    importState.pendingCount = 0;
  }
  if (!importState.pendingCount) importState.pendingOpen = false;
}

/** Ferme un rapport. `lastReport` survit : l'écran Import le garde consultable. */
export function dismissReport(id: number): void {
  importState.reports = importState.reports.filter((e) => e.id !== id);
}

/** Déplie/replie un rapport. Déplier replie les autres (voir `pushReport`). */
export function toggleReport(id: number, collapsed?: boolean): void {
  const entry = importState.reports.find((e) => e.id === id);
  if (!entry) return;
  entry.collapsed = collapsed ?? !entry.collapsed;
  if (!entry.collapsed) {
    for (const other of importState.reports) if (other.id !== id) other.collapsed = true;
  }
}

/** Replie le rapport qui vient d'envoyer l'utilisateur sur une fiche. Replier
 * et non fermer : on ouvre souvent plusieurs mods d'un même lot l'un après
 * l'autre, et le rapport fermé ne revenait par aucun chemin. */
export function collapseReportOnNavigate(id: number): void {
  toggleReport(id, true);
}

/** Fin du flux d'import en masse (§4.2) : conflits déjà arbitrés, pas de modale. */
export function reportBulkDone(report: ArchiveResult[]): void {
  pushReport(report);
  bumpLibraryVersion();
}

/** Issues pour lesquelles **rien n'a été écrit** : l'archive identique qu'on ne
 * réimporte pas, le mod resté ambigu qui attend une décision, et le mod déjà
 * installé hors Pit Box qu'on ne touche pas (§8). Les compter comme importés
 * faisait mentir le titre du toast — « 1 élément importé » quand l'app venait
 * précisément de dire qu'elle n'avait rien fait (signalé à l'usage). */
const WROTE_NOTHING = new Set(["DUPLICATE", "AMBIGUOUS", "UNMANAGED", "HOST_MISSING", "HOST_UNKNOWN"]);

/** Résumé chiffré d'un rapport (§4.2). Un import peut ne produire aucun mod de
 * premier niveau (ex. un pack de skins rattaché à une voiture déjà connue) sans
 * pour autant n'avoir « rien » importé — le titre compte donc tout ce qui a été
 * réellement ajouté, pas seulement les mods. Une extension compte : elle range
 * bien une couche. */
export function importSummary(report: ArchiveResult[]): string {
  const written = (a: ArchiveResult) => a.mods.filter((m) => !WROTE_NOTHING.has(m.outcome));
  // Même règle pour les sous-éléments que pour les mods : un sous-élément resté
  // en attente de décision n'a **rien** rangé. Les compter faisait annoncer
  // « 6 éléments importés » juste après que l'utilisateur ait répondu six fois
  // « ne pas importer » — le titre démentait la fenêtre (signalé à l'usage,
  // même défaut que celui qui avait motivé `WROTE_NOTHING` pour les mods).
  const subsWritten = (a: ArchiveResult) => (a.subs ?? []).filter((s) => !s.awaiting_decision);
  const n = report.reduce(
    (acc, a) => acc + written(a).length + subsWritten(a).length + (a.apps?.length ?? 0) + (a.others?.length ?? 0),
    0,
  );
  // Dit dans le titre pourquoi le compte est plus bas qu'attendu ; le détail
  // (lequel, et pourquoi) est dans le corps du rapport, juste en dessous.
  const skipped = report.reduce(
    (acc, a) => acc + (a.mods.length - written(a).length) + ((a.subs?.length ?? 0) - subsWritten(a).length),
    0,
  );
  const errs = report.filter((a) => a.error).length;
  return (
    (natureBreakdown(report) ?? t("importOverlay.summaryBase", { n })) +
    (skipped ? t("importOverlay.summarySkipped", { skipped }) : "") +
    (errs ? t("importOverlay.summaryErrs", { errs }) : "")
  );
}

/**
 * Ce qui est entré, **par nature** : « 1 voiture, 3 circuits, 2 pilotes ».
 *
 * Un décompte unique disait « 1 élément importé » sur un lot d'une voiture et
 * de dix pilotes : le chiffre était juste au sens strict — un pilote est un
 * « autre mod », pas un mod — mais il donnait à croire que les dix autres
 * avaient été perdus. Les fondre dans un même total effacerait au contraire la
 * distinction qui dit **où** les retrouver ; les nommer règle les deux.
 *
 * Un mod « autre » est nommé par la zone du jeu qu'il touche, seul nom de
 * nature qu'il ait ; celui qui en touche plusieurs compte pour la première,
 * faute de pouvoir se compter deux fois dans un total.
 *
 * `null` quand rien n'a été écrit : le titre retombe alors sur le décompte
 * générique, qui sait dire zéro.
 */
function natureBreakdown(report: ArchiveResult[]): string | null {
  const counts = new Map<string, number>();
  const add = (key: string, n = 1) => counts.set(key, (counts.get(key) ?? 0) + n);
  for (const archive of report) {
    for (const mod of archive.mods) {
      if (WROTE_NOTHING.has(mod.outcome)) continue;
      add(mod.kind === "Track" ? "track" : "car");
    }
    for (const sub of archive.subs ?? []) {
      if (sub.awaiting_decision) continue;
      add(sub.sub_type === "SOUND" ? "sound" : "skin");
    }
    for (const _app of archive.apps ?? []) add("app");
    for (const other of archive.others ?? []) add(`other:${other.categories?.[0] ?? "other"}`);
  }
  if (counts.size === 0) return null;
  const parts = [...counts].map(([key, n]) =>
    key.startsWith("other:")
      ? t("importOverlay.natureOther", { n, what: t(`others.cat.${key.slice("other:".length)}`).toLowerCase() })
      : t(`importOverlay.nature.${key}`, { n }),
  );
  return t("importOverlay.summaryNatures", { list: parts.join(", ") });
}

/** À appeler une seule fois, depuis la racine de l'app (§4.2 : glisser-déposer partout). */
export function initGlobalDragDrop(): () => void {
  const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type !== "drop") return;
    // Le tri revient au backend : depuis le webview, un chemin sans extension
    // peut être un dossier de mod comme un fichier quelconque. L'ancien filtre
    // sur `.zip|.rar|.7z` faisait qu'un dossier déposé ne déclenchait
    // strictement rien — pas d'import, et aucun retour non plus.
    const dropped = event.payload.paths;
    splitDroppedPaths(dropped)
      .then(({ archives, folders }) => {
        if (archives.length) return runImport({ paths: archives, folder: false, copy: false });
        // Un lot mêlant archives et dossiers est rare ; les archives passent
        // d'abord, les dossiers restent à redéposer plutôt que de lancer deux
        // imports concurrents sur un backend qui n'en admet qu'un.
        if (folders.length) {
          return runImport({ paths: folders, folder: true, copy: importState.copyMode });
        }
      })
      .catch((e) => console.error("split_dropped_paths", e));
  });
  const unlistenProgress = listen<ImportProgress>("import:progress", (e) => {
    importState.progress = e.payload;
  });
  return () => {
    unlistenDrop.then((f) => f());
    unlistenProgress.then((f) => f());
  };
}
