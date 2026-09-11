// État de la réparation générale en cours (§9.3) : progression et rapport.
//
// Vit ici, hors de l'écran Maintenance, pour la raison qui a valu à l'édition
// groupée le même traitement (§6.3bis) : la réparation dure des minutes sur une
// grosse install, et rien n'oblige à rester devant l'Atelier pendant ce
// temps-là. Sa progression s'affiche donc dans la pile de notifications, comme
// l'import et les lots — et le rapport lui survit.
import { listen } from "@tauri-apps/api/event";
import type { RepairAllReport, RepairProgress } from "$lib/maintenance";
import { bumpLibraryVersion } from "$lib/libraryVersion.svelte";

export const repairState = $state<{
  running: boolean;
  progress: RepairProgress | null;
  /** Dernier rapport, tant qu'on ne l'a pas fermé. */
  result: RepairAllReport | null;
}>({
  running: false,
  progress: null,
  result: null,
});

export function dismissRepairResult(): void {
  repairState.result = null;
}

/**
 * Enveloppe une réparation. La valeur initiale est posée ici et pas attendue du
 * backend : la phase de pesée précède le premier événement, et sans elle la
 * notification n'apparaîtrait qu'une fois la bibliothèque déjà parcourue —
 * c'est-à-dire au moment précis où l'écran a l'air figé.
 */
export async function runRepair(run: () => Promise<RepairAllReport>): Promise<RepairAllReport | null> {
  // Une seule à la fois : deux réparations concurrentes redéploieraient les
  // mêmes mods l'une sous l'autre.
  if (repairState.running) return null;
  repairState.running = true;
  repairState.result = null;
  repairState.progress = { phase: "sizing", index: 0, total: 0, ratio: 0, etaSecs: null, label: "" };
  try {
    const report = await run();
    repairState.result = report;
    // Le redéploiement change l'état de mods affichés ailleurs (bloc SESSION,
    // fiche ouverte) sans qu'on y ait touché : même resynchronisation que les
    // lots.
    bumpLibraryVersion();
    return report;
  } finally {
    repairState.running = false;
    repairState.progress = null;
  }
}

/** À appeler une seule fois, depuis la racine de l'app. */
export function initRepairProgress(): () => void {
  const unlisten = listen<RepairProgress>("repair:progress", (e) => {
    repairState.progress = e.payload;
  });
  return () => {
    unlisten.then((f) => f());
  };
}
