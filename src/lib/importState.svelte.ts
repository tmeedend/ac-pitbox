// État d'import partagé (§4.6bis) : le glisser-déposer fonctionne partout dans
// l'app (pas seulement dans une bibliothèque), donc la progression, le rapport
// et l'arbitrage des conflits flous vivent ici plutôt que dans Library.svelte.
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { StorageKey } from "./storage";
import {
  importArchives,
  importFolders,
  resolveConflict,
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
  pendingConflicts: PendingConflict[];
  pendingAmbiguous: PendingAmbiguous[];
  copyMode: boolean;
  /** Incrémenté à chaque fois que la bibliothèque a pu changer, pour resynchroniser les vues ouvertes. */
  version: number;
}>({
  importing: false,
  progress: null,
  report: null,
  pendingConflicts: [],
  pendingAmbiguous: [],
  copyMode: localStorage.getItem(StorageKey.importCopy) !== "false",
  version: 0,
});

export function setCopyMode(v: boolean): void {
  importState.copyMode = v;
  localStorage.setItem(StorageKey.importCopy, String(v));
}

/** Lance un import et récolte conflits flous + cas ambigus (§4.2/§4.4). La
 * `source` est mémorisée pour pouvoir reprendre un cas ambigu après décision. */
async function runImport(source: { paths: string[]; folder: boolean; copy: boolean }): Promise<void> {
  importState.importing = true;
  importState.progress = null;
  try {
    const report = source.folder
      ? await importFolders(source.paths, source.copy)
      : await importArchives(source.paths);
    importState.report = report;
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
    importState.pendingAmbiguous = report.flatMap((a) =>
      a.mods
        .filter((m) => m.outcome === "AMBIGUOUS")
        .map((m) => ({
          id: m.id_interne,
          name: m.display_name ?? m.id_interne,
          added: m.added_count ?? 0,
          overwritten: m.overwritten_count ?? 0,
          total: m.existing_total ?? 0,
          source,
        })),
    );
    importState.version++;
  } finally {
    importState.importing = false;
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
  importState.progress = null;
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

export function dismissReport(): void {
  importState.report = null;
}

/** Fin du flux d'import en masse (§4.6) : conflits déjà arbitrés, pas de modale. */
export function reportBulkDone(report: ArchiveResult[]): void {
  importState.report = report;
  importState.version++;
}

/** À appeler une seule fois, depuis la racine de l'app (§4.6bis : glisser-déposer partout). */
export function initGlobalDragDrop(): () => void {
  const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "drop") {
      const archives = event.payload.paths.filter((p) => /\.(zip|rar|7z)$/i.test(p));
      if (archives.length) runImport({ paths: archives, folder: false, copy: false });
    }
  });
  const unlistenProgress = listen<ImportProgress>("import:progress", (e) => {
    importState.progress = e.payload;
  });
  return () => {
    unlistenDrop.then((f) => f());
    unlistenProgress.then((f) => f());
  };
}
