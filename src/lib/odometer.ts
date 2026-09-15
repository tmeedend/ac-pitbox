// La distance parcourue, telle qu'elle se lit sur une fiche (§6).
//
// Une ligne partagée par les deux types de fiche — la fiche technique d'une
// voiture (`TechSheet`) et la carte « Circuit » d'un circuit. Elle a sa propre
// règle, et c'est celle-là qui justifie le partage : **l'odomètre est toujours
// affiché**, contrairement à toutes les autres lignes qui disparaissent quand
// elles n'ont rien à dire. Un odomètre vide est lui-même une réponse, et
// « jamais essayée » se lit mieux qu'un tiret.
import { t } from "$lib/i18n/index.svelte";
import type { ModDetail } from "$lib/library";

export function odometerText(d: ModDetail): string {
  if (d.distance_km != null) return `${d.distance_km.toFixed(1)} km`;
  return d.tried ? t("detail.triedYes") : t("detail.triedNo");
}
