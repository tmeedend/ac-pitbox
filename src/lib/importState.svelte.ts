// État d'import partagé (§4.2) : le glisser-déposer fonctionne partout dans
// l'app (pas seulement dans une bibliothèque), donc la progression, le rapport
// et l'arbitrage des conflits flous vivent ici plutôt que dans Library.svelte.
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { t } from "./i18n/index.svelte";
import { StorageKey } from "./storage";
import { getUiPref, setUiPref } from "./uiPrefs.svelte";
import {
  cancelImport,
  importArchives,
  importFolders,
  resolveConflict,
  splitDroppedPaths,
  type ArchiveResult,
  type ImportProgress,
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

export const importState = $state<{
  importing: boolean;
  progress: ImportProgress | null;
  report: ArchiveResult[] | null;
  /** Dernier rapport, conservé après fermeture du toast (§4.2bis) : un import
   * de quarante mods ne doit pas disparaître sur un clic réflexe sur ✕.
   * En mémoire seulement — il ne survit pas à un redémarrage, et n'a pas à le
   * faire : c'est le compte rendu d'une action, pas un réglage. */
  lastReport: ArchiveResult[] | null;
  pendingConflicts: PendingConflict[];
  pendingAmbiguous: PendingAmbiguous[];
  copyMode: boolean;
  /** Arrêt demandé (§4.2bis) : le lot s'interrompt entre deux items, donc il
   * reste du travail en cours après le clic — le bouton doit le dire. */
  cancelling: boolean;
  /** Incrémenté à chaque fois que la bibliothèque a pu changer, pour resynchroniser les vues ouvertes. */
  version: number;
}>({
  importing: false,
  progress: null,
  report: null,
  lastReport: null,
  pendingConflicts: [],
  pendingAmbiguous: [],
  // Défaut synchrone (état module-level, pas de composant/onMount ici),
  // corrigé de façon asynchrone juste en dessous dès que la valeur
  // sauvegardée répond (§6.2, même schéma que `nav.svelte.ts`).
  copyMode: true,
  cancelling: false,
  version: 0,
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
    // Même référence des deux côtés : trancher un cas ambigu modifie le rapport
    // en place, et les deux vues doivent en rendre compte.
    importState.report = report;
    importState.lastReport = report;
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
    importState.version++;
  } finally {
    importState.importing = false;
    importState.cancelling = false;
    importState.progress = null;
  }
}

export async function pickAndImportArchive(): Promise<void> {
  const sel = await open({
    multiple: true,
    filters: [{ name: "Archives", extensions: ["zip", "rar", "7z"] }],
  });
  if (!sel) return;
  await runImport({ paths: Array.isArray(sel) ? sel : [sel], folder: false, copy: false });
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
    if (resolved && importState.report) {
      for (const a of importState.report) {
        const i = a.mods.findIndex((m) => m.id_interne === item.id);
        if (i >= 0) a.mods[i] = resolved;
      }
    }
    importState.version++;
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
  importState.version++;
}

/** Ferme le toast. `lastReport` survit : l'écran Import le garde consultable. */
export function dismissReport(): void {
  importState.report = null;
}

/** Fin du flux d'import en masse (§4.2) : conflits déjà arbitrés, pas de modale. */
export function reportBulkDone(report: ArchiveResult[]): void {
  importState.report = report;
  importState.lastReport = report;
  importState.version++;
}

/** Résumé chiffré d'un rapport (§4.2). Un import peut ne produire aucun mod de
 * premier niveau (ex. un pack de skins rattaché à une voiture déjà connue) sans
 * pour autant n'avoir « rien » importé — le titre compte donc tout ce qui a été
 * réellement ajouté, pas seulement les mods. */
export function importSummary(report: ArchiveResult[]): string {
  const n = report.reduce(
    (acc, a) => acc + a.mods.length + (a.subs?.length ?? 0) + (a.apps?.length ?? 0) + (a.others?.length ?? 0),
    0,
  );
  const errs = report.filter((a) => a.error).length;
  return t("importOverlay.summaryBase", { n }) + (errs ? t("importOverlay.summaryErrs", { errs }) : "");
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
