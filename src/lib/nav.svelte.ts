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
  /** Voiture : preview du skin choisi ; circuit : photo illustratrice (fond). */
  preview: string | null;
  /** Circuit : layout choisi. */
  layout: string | null;
  /** Voiture : id du skin choisi (mémorisé par voiture). */
  skin: string | null;
  /** Circuit : tracé à superposer à la photo dans le bloc Session. */
  outline: string | null;
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
  /** Id du mod affiché en fiche pleine page (Library), ou null si aucune n'est
   * ouverte — centralisé ici (plutôt que local à Library) pour que la
   * navigation manette globale (AppShell) sache si elle doit céder la main
   * gauche/droite au visualiseur (mod précédent/suivant) et gérer B = fermer. */
  openFull: string | null;
  /** Demande de lancement immédiat (bouton rouge « Démarrer la session » de
   * la barre latérale) : posée avant de naviguer vers l'écran de réglages,
   * consommée par Launch.svelte une fois monté et prêt (mêmes réglages que
   * s'ils avaient été ouverts normalement — dernier preset du type courant). */
  autoLaunch: boolean;
}>({
  section: "cars",
  prefill: null,
  openMod: null,
  search: null,
  sessionCar: load("pitbox.session.car"),
  sessionTrack: load("pitbox.session.track"),
  openFull: null,
  autoLaunch: false,
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

// --- Garde de navigation (§10bis) ---
// Un écran avec des modifications non enregistrées (ex. Réglages, zoom/langue
// appliqués en aperçu live avant la sauvegarde) peut poser une garde : avant
// tout changement de section, on lui laisse la main pour proposer
// d'enregistrer ou d'annuler, plutôt que de quitter en silence en laissant
// un état incohérent (aperçu appliqué mais jamais sauvegardé).
let sectionGuard: (() => Promise<boolean>) | null = null;

/** Posée par l'écran courant à son montage, retirée à son démontage. */
export function setSectionGuard(guard: (() => Promise<boolean>) | null): void {
  sectionGuard = guard;
}

/** Seul point d'entrée pour changer de section — respecte la garde posée.
 * Renvoie `true` si la navigation a bien eu lieu (utile pour enchaîner une
 * action, ex. ouvrir un mod précis, seulement si on a effectivement changé
 * d'écran). */
export async function requestSection(id: string): Promise<boolean> {
  if (sectionGuard) {
    const ok = await sectionGuard();
    if (!ok) return false;
  }
  nav.section = id;
  return true;
}
