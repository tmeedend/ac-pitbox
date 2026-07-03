// État d'import partagé (§4.6bis) : le glisser-déposer fonctionne partout dans
// l'app (pas seulement dans une bibliothèque), donc la progression, le rapport
// et l'arbitrage des conflits flous vivent ici plutôt que dans Library.svelte.
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
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

export const importState = $state<{
  importing: boolean;
  progress: ImportProgress | null;
  report: ArchiveResult[] | null;
  pendingConflicts: PendingConflict[];
  copyMode: boolean;
  /** Incrémenté à chaque fois que la bibliothèque a pu changer, pour resynchroniser les vues ouvertes. */
  version: number;
}>({
  importing: false,
  progress: null,
  report: null,
  pendingConflicts: [],
  copyMode: localStorage.getItem("pitbox.import.copy") !== "false",
  version: 0,
});

export function setCopyMode(v: boolean): void {
  importState.copyMode = v;
  localStorage.setItem("pitbox.import.copy", String(v));
}

async function runImport(task: Promise<ArchiveResult[]>): Promise<void> {
  importState.importing = true;
  importState.progress = null;
  try {
    const report = await task;
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
  await runImport(importArchives(Array.isArray(sel) ? sel : [sel]));
}

export async function pickAndImportFolder(): Promise<void> {
  const sel = await open({ directory: true, multiple: false });
  if (!sel || typeof sel !== "string") return;
  await runImport(importFolders([sel], importState.copyMode));
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
      if (archives.length) runImport(importArchives(archives));
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
