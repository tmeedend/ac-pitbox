// État de navigation partagé + sélection de session (§8.6). La bibliothèque EST
// le sélecteur : ouvrir une voiture/un circuit le définit comme choix de session,
// affiché en permanence dans le bloc SESSION de la barre latérale (§6.1ter).

import { invoke } from "@tauri-apps/api/core";
import { StorageKey } from "./storage";

export interface LaunchPrefill {
  kind: "Car" | "Track";
  id: string;
  name: string;
}

/** Action demandée depuis la sélection groupée de la bibliothèque voitures
 * (§6.3ter) : envoyer les mods sélectionnés comme adversaires de la session
 * course courante. "set" vide la liste d'adversaires existante ; "add" la
 * complète. Consommée par Launch.svelte une fois monté et prêt (même schéma
 * que `autoLaunch`). */
export interface OpponentsAction {
  mode: "set" | "add";
  carIds: string[];
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

/** Ancien mécanisme (avant fix, voir plus bas) : lu une seule fois pour
 * migrer les choix déjà faits, jamais réécrit. `localStorage` n'est pas
 * garanti synchrone sur disque côté WebView2 — fermer l'app juste après un
 * clic pouvait perdre la sélection la plus récente (bug réel : le circuit,
 * choisi typiquement juste avant de fermer, ne survivait presque jamais à
 * un redémarrage, contrairement à la voiture choisie plus tôt). */
function loadLegacy(key: string): SessionPick | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as SessionPick) : null;
  } catch {
    return null;
  }
}

interface SessionPicks {
  car: SessionPick | null;
  track: SessionPick | null;
}

/** Persistance durable (§8.6) : fichier écrit côté Rust (`session_state.rs`,
 * `std::fs::write` synchrone) plutôt que `localStorage` — voir `loadLegacy`
 * pour le pourquoi du changement. */
function loadPicks(): Promise<SessionPicks> {
  return invoke<SessionPicks>("get_session_picks").catch(() => ({ car: null, track: null }));
}

function savePicks(picks: SessionPicks): void {
  invoke("save_session_picks", { picks }).catch((e) => console.error("save_session_picks", e));
}

export const nav = $state<{
  section: string;
  prefill: LaunchPrefill | null;
  /** Demande d'ouverture d'une fiche détail depuis une vue transversale (§12bis.3). */
  openMod: string | null;
  /** Terme de recherche à appliquer à la bibliothèque (ex. filtrer par pack, §4.4). */
  search: string | null;
  /** Duo de session courant (§8.6) — la bibliothèque le met à jour à l'ouverture. */
  sessionCar: SessionPick | null;
  sessionTrack: SessionPick | null;
  /** Id du mod affiché en fiche pleine page (Library), ou null si aucune n'est
   * ouverte — centralisé ici (plutôt que local à Library) pour que la
   * navigation manette globale (AppShell) sache si elle doit céder la main
   * gauche/droite au visualiseur (mod précédent/suivant) et gérer B = fermer. */
  openFull: string | null;
  /** Visionneuse plein écran d'image ouverte (galerie Screenshots/Backgrounds,
   * §6.1) — le navigateur manette global (`gamepadNav.ts`) et la navigation
   * manette mod précédent/suivant (`Library.svelte::navigateFull`) doivent
   * tous deux céder gauche/droite/B à la visionneuse tant que c'est vrai,
   * sinon une même pression ferait à la fois défiler les images ET changer de
   * mod, ou fermerait la fiche entière au lieu de juste la visionneuse. */
  lightboxOpen: boolean;
  /** Demande de lancement immédiat (bouton rouge « Démarrer la session » de
   * la barre latérale) : posée avant de naviguer vers l'écran de réglages,
   * consommée par Launch.svelte une fois monté et prêt (mêmes réglages que
   * s'ils avaient été ouverts normalement — dernier preset du type courant). */
  autoLaunch: boolean;
  /** Action « adversaires » en attente (§6.3ter), posée depuis la sélection
   * groupée de la bibliothèque voitures. */
  opponentsAction: OpponentsAction | null;
}>({
  section: "cars",
  prefill: null,
  openMod: null,
  search: null,
  // Hydraté juste en dessous, de façon asynchrone (lecture fichier côté
  // Rust) : reste `null` le temps d'un aller-retour IPC au tout premier
  // rendu, comme le zoom/la langue (`getConfig()` dans AppShell.svelte).
  sessionCar: null,
  sessionTrack: null,
  openFull: null,
  lightboxOpen: false,
  autoLaunch: false,
  opponentsAction: null,
});

loadPicks().then((picks) => {
  // Repli sur l'ancien `localStorage` seulement si le nouveau fichier n'a
  // rien pour cette entité (première ouverture après la mise à jour) — et
  // dans ce cas, persiste tout de suite au nouvel endroit pour ne plus
  // jamais redépendre de `localStorage`.
  const car = picks.car ?? loadLegacy(StorageKey.sessionCar);
  const track = picks.track ?? loadLegacy(StorageKey.sessionTrack);
  nav.sessionCar = car;
  nav.sessionTrack = track;
  if ((car && !picks.car) || (track && !picks.track)) {
    savePicks({ car, track });
  }
});

/** Définit le choix de session (persisté) — appelé à l'ouverture d'un mod (§8.6). */
export function pickSession(kind: "Car" | "Track", pick: SessionPick): void {
  if (kind === "Car") {
    nav.sessionCar = pick;
  } else {
    nav.sessionTrack = pick;
  }
  savePicks({ car: nav.sessionCar, track: nav.sessionTrack });
}

/** Pose une action « adversaires » à destination de l'écran de session (§6.3ter). */
export function queueOpponentsAction(mode: "set" | "add", carIds: string[]): void {
  nav.opponentsAction = { mode, carIds };
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
  // `openFull` n'est jamais remis à zéro par la fiche pleine page elle-même
  // en quittant (seul son bouton "retour" le fait) : sans ce reset, changer
  // de section par un autre chemin (ex. double-clic sur le slot de session)
  // laisse l'id traîner, et la bibliothèque fraîchement montée (potentiellement
  // d'un AUTRE type — voiture vs circuit) le rouvre tel quel, comme s'il lui
  // appartenait. Bug réel : ouvrir un circuit puis basculer vers les voitures
  // sans en avoir choisi une rouvrait la fiche du circuit en tant que voiture
  // (aperçu 3D lancé avec l'id du circuit → crash natif d'acShowroom.exe).
  nav.openFull = null;
  return true;
}
