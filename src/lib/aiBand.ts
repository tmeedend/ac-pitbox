// Le modèle « centre ± écart » de la difficulté et de l'agressivité (§2.9).
//
// Son propre module, et pour une raison mécanique : `launch.ts` importe
// `gridThumbs.svelte.ts`, donc des runes, donc il n'est pas chargeable par
// Vitest — qui n'a pas le plugin Svelte, décision assumée du projet. Une
// fonction qu'on veut tester ne peut pas vivre dans un fichier qui en dépend.
//
// Même raison que `carSpecs.ts` et `opponentPool.ts` : pas de Svelte, pas de
// DOM, rien que des fonctions.

/**
 * Les bornes réelles d'un réglage « centre ± écart », **bornées** (§2.9).
 *
 * Le bornage porte sur l'affichage **et** sur ce qui part en jeu : un centre de
 * 3 avec un écart de 5 donne `3% ± 5 (0–8)`, jamais `(-2–8)`. Le côté Rust
 * refait le même calcul à la frontière du preset — les deux doivent dire la
 * même chose, et c'est la seule raison pour laquelle il est écrit deux fois.
 *
 * Ici et non dans le composant : Vitest n'a pas le plugin Svelte (décision
 * assumée, voir la section Tests), donc une fonction qu'on veut tester ne peut
 * pas vivre dans un `.svelte`.
 */
export function band(center: number, spread: number, min: number, max: number): [number, number] {
  return [Math.max(min, center - spread), Math.min(max, center + spread)];
}

/**
 * Passe d'un minimum et d'un maximum au couple centre + écart (§2.9).
 *
 * Sert à deux endroits qui parlent encore en bornes, et pour deux raisons
 * différentes : les presets et sessions enregistrés **avant** ce modèle, et les
 * grilles, qui gardent les bornes parce que c'est le vocabulaire de Content
 * Manager d'où elles viennent.
 *
 * Un écart demi-entier est arrondi **vers le haut** : mieux vaut une fourchette
 * d'un point trop large que d'un point trop étroite, qui écraserait une
 * dispersion que l'utilisateur avait réglée.
 */
export function centerSpreadOf(min: number, max: number): { center: number; spread: number } {
  const lo = Math.min(min, max);
  const hi = Math.max(min, max);
  return { center: Math.round((lo + hi) / 2), spread: Math.ceil((hi - lo) / 2) };
}
