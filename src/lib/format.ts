// Formatages d'affichage partagés (tableaux, vues transversales…).

const DASH = "—";

/** Taille lisible (Ko/Mo/Go/To, base 1024), « — » si pas encore calculée (§9.4). */
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
