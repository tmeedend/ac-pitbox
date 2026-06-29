// État de navigation partagé (§8.6) : permet à une fiche de demander l'ouverture
// de l'écran de lancement pré-rempli avec une voiture ou un circuit.

export interface LaunchPrefill {
  kind: "Car" | "Track";
  id: string;
  name: string;
}

export const nav = $state<{
  section: string;
  prefill: LaunchPrefill | null;
  /** Demande d'ouverture d'une fiche détail depuis une vue transversale (§12bis.3). */
  openMod: string | null;
}>({
  section: "cars",
  prefill: null,
  openMod: null,
});
