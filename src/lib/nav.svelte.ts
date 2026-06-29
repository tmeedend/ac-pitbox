// État de navigation partagé (§8.6) : permet à une fiche de demander l'ouverture
// de l'écran de lancement pré-rempli avec une voiture ou un circuit.

export interface LaunchPrefill {
  kind: "Car" | "Track";
  id: string;
  name: string;
}

export const nav = $state<{ section: string; prefill: LaunchPrefill | null }>({
  section: "cars",
  prefill: null,
});
