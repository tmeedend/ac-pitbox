// État de navigation partagé + sélection de session (§8.6). La bibliothèque EST
// le sélecteur : ouvrir une voiture/un circuit le définit comme choix de session,
// affiché en permanence dans le bloc SESSION de la barre latérale (§6.1ter).

export interface LaunchPrefill {
  kind: "Car" | "Track";
  id: string;
  name: string;
}

/** Élément sélectionné pour la session (voiture ou circuit). */
export interface SessionPick {
  id: string;
  name: string;
  meta: string;
  preview: string | null;
  layout: string | null;
}

function load(key: string): SessionPick | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as SessionPick) : null;
  } catch {
    return null;
  }
}

export const nav = $state<{
  section: string;
  prefill: LaunchPrefill | null;
  /** Demande d'ouverture d'une fiche détail depuis une vue transversale (§12bis.3). */
  openMod: string | null;
  /** Terme de recherche à appliquer à la bibliothèque (ex. filtrer par pack, §4.7). */
  search: string | null;
  /** Duo de session courant (§8.6) — la bibliothèque le met à jour à l'ouverture. */
  sessionCar: SessionPick | null;
  sessionTrack: SessionPick | null;
}>({
  section: "cars",
  prefill: null,
  openMod: null,
  search: null,
  sessionCar: load("pitbox.session.car"),
  sessionTrack: load("pitbox.session.track"),
});

/** Définit le choix de session (persisté) — appelé à l'ouverture d'un mod (§8.6). */
export function pickSession(kind: "Car" | "Track", pick: SessionPick): void {
  if (kind === "Car") {
    nav.sessionCar = pick;
    localStorage.setItem("pitbox.session.car", JSON.stringify(pick));
  } else {
    nav.sessionTrack = pick;
    localStorage.setItem("pitbox.session.track", JSON.stringify(pick));
  }
}
