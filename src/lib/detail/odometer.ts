// La distance parcourue, telle qu'elle se lit sur une fiche (§6).
//
// Une ligne partagée par les deux types de fiche — la ligne d'usage de
// l'en-tête d'une voiture (`usageLine`, FICHE§7.1) et la carte « Circuit »
// d'un circuit. Elle a sa propre
// règle, et c'est celle-là qui justifie le partage : **l'odomètre est toujours
// affiché**, contrairement à toutes les autres lignes qui disparaissent quand
// elles n'ont rien à dire. Un odomètre vide est lui-même une réponse, et
// « jamais essayée » se lit mieux qu'un tiret.
import { t } from "$lib/i18n/index.svelte";
import type { ModDetail } from "$lib/library/library";

export function odometerText(d: ModDetail): string {
  if (d.distance_km != null) return `${d.distance_km.toFixed(1)} km`;
  return d.tried ? t("detail.triedYes") : t("detail.triedNo");
}

/** The usage line under a car's name (FICHE§7.1): the odometer, when there is
 * one, and whether the car was tried. It left the tech sheet: it is the
 * user's use of the car, not a property of it. */
export function usageLine(d: ModDetail): string {
  const parts: string[] = [];
  if (d.distance_km != null && d.distance_km > 0) {
    parts.push(`${t("detail.odometer")} ${d.distance_km.toFixed(1)} km`);
  }
  parts.push(d.tried ? t("detail.triedYes") : t("detail.triedNo"));
  return parts.join(" · ");
}
