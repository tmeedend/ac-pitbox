// Lest et bride du joueur (§2.7) — les deux handicaps qu'on s'impose à
// soi-même pour équilibrer une course.
//
// **Pourquoi un store, comme `gridMods`.** Ces deux réglages s'éditent dans la
// carte voiture du panneau gauche, qui est toujours à l'écran ; ils font partie
// de la configuration de session (`RaceSetup`), qui vit dans l'écran de
// réglages, monté seulement quand on l'ouvre. Sans intermédiaire, la carte ne
// pourrait ni les lire ni les écrire hors de cet écran.
//
// **Pourquoi le panneau gauche et pas SESSION OPTIONS.** Ils valent pour les
// quatre types de session : ils ne dépendent pas du type, donc ils n'ont rien à
// faire dans le bloc dont tout le contenu en dépend. Ça ne les sort pas du
// setup pour autant — une session enregistrée les restitue.
//
// Le sens de circulation est unique : ce store est la valeur vivante, l'écran
// de réglages la recopie dans `setup`, et un chargement (preset, session
// enregistrée) revient l'écrire ici. Jamais l'inverse, sinon les deux
// s'entre-réveillent.

import { invoke } from "@tauri-apps/api/core";

/** Bornes relevées sur Content Manager : au-delà, c'est lui qui recalerait. */
export const BALLAST_MAX = 200;
export const RESTRICTOR_MAX = 100;

export const playerHandicap = $state<{ ballast: number; restrictor: number }>({ ballast: 0, restrictor: 0 });

const clamp = (v: number, max: number): number =>
  Number.isFinite(v) ? Math.max(0, Math.min(max, Math.round(v))) : 0;

export function setPlayerHandicap(ballast: number, restrictor: number): void {
  playerHandicap.ballast = clamp(ballast, BALLAST_MAX);
  playerHandicap.restrictor = clamp(restrictor, RESTRICTOR_MAX);
}

/**
 * Amorce depuis `launch_state.json` au démarrage.
 *
 * Rangé dans la **sélection** et non dans les presets par type, parce que ces
 * deux valeurs ne dépendent pas du type de session (§2.7) : il n'y en a qu'un
 * jeu, celui qu'on avait sous les yeux. Jamais une erreur — un fichier
 * illisible laisse simplement les deux à zéro, ce qu'ils valaient avant que ce
 * réglage n'existe.
 */
export async function loadPlayerHandicap(): Promise<void> {
  try {
    const state = await invoke<{ selection: { player_ballast?: number; player_restrictor?: number } | null }>(
      "get_launch_state",
    );
    setPlayerHandicap(state?.selection?.player_ballast ?? 0, state?.selection?.player_restrictor ?? 0);
  } catch {
    /* rien à dire */
  }
}
