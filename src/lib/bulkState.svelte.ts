// État d'un lot en cours (§6.3bis) : progression, arrêt demandé, rapport final.
//
// Vit ici plutôt que dans le panneau de sélection groupée parce que ces
// actions partent désormais aussi du **menu contextuel**, qui n'a pas de
// composant où loger un état — et parce qu'un rapport enfermé dans un panneau
// disparaissait avec lui. La progression et le rapport s'affichent donc dans
// la pile de notifications, comme l'import (§4.2bis).
import { listen } from "@tauri-apps/api/event";
import { cancelBulk, type BulkExportItem, type BulkProgress, type BulkReport } from "$lib/bulkEdit";

/** Les quatre lots qui touchent au disque. Les autres (favori, catégorie,
 * tags) sont quelques écritures SQLite : ni progression, ni rapport. */
export type BulkOp = "activate" | "deactivate" | "delete" | "export";

export const bulkState = $state<{
  running: boolean;
  /** Arrêt demandé : le lot s'interrompt entre deux mods, donc il reste du
   * travail en cours après le clic — le bouton doit le dire. */
  cancelling: boolean;
  progress: BulkProgress | null;
  op: BulkOp | null;
  /** Dernier rapport, tant qu'on ne l'a pas fermé. */
  result: { op: BulkOp; report: BulkReport } | null;
}>({
  running: false,
  cancelling: false,
  progress: null,
  op: null,
  result: null,
});

export function requestCancelBulk(): void {
  bulkState.cancelling = true;
  cancelBulk().catch((e) => console.error("cancel_bulk", e));
}

export function dismissBulkResult(): void {
  bulkState.result = null;
}

/** Enveloppe un lot : progression visible dès le premier instant, rapport
 * conservé à la fin. La valeur initiale est posée ici et pas attendue du
 * backend — la commande met un instant à démarrer, et sans elle la
 * notification n'apparaîtrait qu'une fois le premier mod déjà traité. */
export async function runBulkOp(
  op: BulkOp,
  total: number,
  run: () => Promise<BulkReport>,
): Promise<BulkReport | null> {
  // Un seul lot à la fois : le backend n'a qu'un drapeau d'annulation, deux
  // lots concurrents s'arrêteraient l'un l'autre.
  if (bulkState.running) return null;
  bulkState.running = true;
  bulkState.cancelling = false;
  bulkState.op = op;
  bulkState.progress = { index: 0, total, op, id: "" };
  try {
    const report = await run();
    bulkState.result = { op, report };
    return report;
  } finally {
    bulkState.running = false;
    bulkState.cancelling = false;
    bulkState.progress = null;
    bulkState.op = null;
  }
}

/** L'export ne rend pas un `BulkReport` mais un item par mod : converti ici
 * pour que la notification n'ait qu'une seule forme de rapport à afficher. */
export function exportToReport(items: BulkExportItem[], asked: number): BulkReport {
  return {
    ok: items.filter((i) => !i.error).map((i) => i.id),
    failed: items.filter((i) => i.error).map((i) => ({ id: i.id, error: i.error! })),
    // Moins d'items que de mods demandés = le lot s'est arrêté en chemin.
    cancelled: items.length < asked,
  };
}

/** À appeler une seule fois, depuis la racine de l'app. */
export function initBulkProgress(): () => void {
  const unlisten = listen<BulkProgress>("bulk:progress", (e) => {
    bulkState.progress = e.payload;
  });
  return () => {
    unlisten.then((f) => f());
  };
}
