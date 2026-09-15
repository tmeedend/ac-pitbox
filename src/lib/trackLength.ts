// La longueur d'un tracé, telle que `ui_track.json` la déclare — et telle
// qu'on peut l'afficher.
//
// Le champ n'a pas d'unité convenue, et ce n'est pas une supposition :
// **mesuré sur les 70 `ui_track.json` de l'installation de référence**, il
// s'écrit de trois façons.
//
//   - un entier de mètres, le cas courant : `"7004"`, `"20832"`, `"2507"` ;
//   - un nombre déjà suivi de son unité : `"165km"` (six fois), `"190km"`,
//     `"4456 m"` ;
//   - un décimal de **kilomètres** : `"3.602"` — Laguna Seca, un cas unique
//     mais qui suffit à interdire d'ajouter « m » les yeux fermés, puisque
//     « 3.602 m » serait faux de trois ordres de grandeur.
//
// D'où les deux règles ci-dessous, et le fait que ce soit une fonction pure
// avec ses tests plutôt qu'un bout de gabarit : ses cas limites ne se
// vérifient qu'en l'exécutant.

/** En dessous de ce seuil, la valeur est lue comme des **kilomètres**.
 *
 * Aucun tracé d'Assetto Corsa ne mesure moins de cent mètres — les plus courts
 * de l'installation de référence sont des aires de drift et de drag à 200, 400
 * et 500 m, toutes bien au-dessus. Un nombre inférieur à 100 ne peut donc pas
 * être des mètres, et le seul rencontré (`3.602`) est bien des kilomètres. */
const KM_THRESHOLD = 100;

/** La longueur formatée avec son unité, ou `null` quand il n'y a rien à dire.
 *
 * Une valeur qui porte **déjà** une unité est rendue telle quelle : son auteur
 * a dit ce qu'il voulait dire, et la réécrire reviendrait à convertir ce que le
 * mod déclare — précisément ce que le WIKI§7.6 interdit. */
export function trackLength(raw: string | null | undefined): string | null {
  const s = raw?.trim();
  if (!s) return null;
  // Une lettre quelque part = l'unité est déjà écrite. Test volontairement
  // large (`km`, `m`, `KM`, `miles`…) : on ne cherche pas à reconnaître les
  // unités, seulement à savoir si l'auteur s'est exprimé.
  if (/[a-z]/i.test(s)) return s;
  const n = Number(s.replace(",", "."));
  if (!Number.isFinite(n) || n <= 0) return s;
  const metres = n < KM_THRESHOLD ? Math.round(n * 1000) : Math.round(n);
  return `${metres.toLocaleString()} m`;
}
