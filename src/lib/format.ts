// Formatages d'affichage partagés (tableaux, vues transversales…).

const DASH = "—";

/** Taille lisible (Ko/Mo/Go/To, base 1024), « — » si pas encore calculée (SESSION§4). */
export function fmtSize(bytes: number | null | undefined): string {
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

/** Durée lisible : `13 s`, `5 min 29 s`, `1 h 34 min`. Les secondes tombent
 * passé l'heure — sur un replay d'endurance, elles n'apprennent rien. */
export function fmtDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return DASH;
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, "0")} min`;
  if (m > 0) return `${m} min ${String(s).padStart(2, "0")} s`;
  return `${s} s`;
}
