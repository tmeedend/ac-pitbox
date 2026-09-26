// Mémorisation de la sélection + presets (SESSION§3): `launch_state.json`, what
// the session screen finds again when it is mounted.
//
// `opponents` en fait partie (SESSION§3.3, bug réel) : sans elle, revenir sur cet
// écran après être allé choisir un circuit/une voiture démonte puis remonte
// Launch.svelte — `setup.opponents` (état local) repart de zéro, et
// `applyPreset` régénère alors un plateau aléatoire à la place de celui,
// potentiellement construit à la main (mode « libre »), qu'avait l'utilisateur.
//
// Persisté côté Rust (`launch_state.json`, écriture synchrone), pas en
// `localStorage` : même bug que le duo voiture/circuit (SESSION§3, voir
// `nav.svelte.ts`/`session_state.rs`) — `localStorage` n'est pas garanti
// synchrone sur disque côté WebView2, ce qui perdait les réglages de
// session à la fermeture de l'app plutôt qu'au prochain changement d'onglet.
import { invoke } from "@tauri-apps/api/core";
import type { RaceSetup, SessionType } from "$lib/launch/launch";
import type { Opponent } from "$lib/launch/opponent";
import type { TypePreset } from "$lib/launch/typePresets";
import { StorageKey } from "$lib/storage";

/** What the screen had selected, whatever the session type. */
export interface Selection {
  car_id: string;
  car_skin: string | null;
  track_id: string;
  track_layout: string | null;
  session_type: SessionType;
  opponents: Opponent[];
  player_ballast: number;
  player_restrictor: number;
}

export type TypePresets = Record<string, TypePreset>;

interface LaunchStateFile {
  selection: Selection | null;
  presets: TypePresets | null;
}

/** The selection part of the current settings. */
export function selectionOf(setup: RaceSetup): Selection {
  return {
    car_id: setup.car_id,
    car_skin: setup.car_skin,
    track_id: setup.track_id,
    track_layout: setup.track_layout,
    session_type: setup.session_type,
    opponents: setup.opponents,
    // Dans la sélection et non dans les presets par type : ces deux-là ne
    // dépendent pas du type de session (SETUP§2.7).
    player_ballast: setup.player_ballast,
    player_restrictor: setup.player_restrictor,
  };
}

/** What the screen starts from. `fromFile` is false when the file had nothing
 * and the old `localStorage` was read instead — the caller then writes the
 * file once, so that it reflects the state without waiting for a change. */
export async function loadLaunchState(): Promise<{
  selection: Partial<Selection>;
  presets: TypePresets;
  fromFile: boolean;
}> {
  const state = await invoke<LaunchStateFile>("get_launch_state").catch(
    (): LaunchStateFile => ({ selection: null, presets: null }),
  );
  // Repli sur l'ancien `localStorage` seulement si le fichier Rust n'a rien
  // (première ouverture après la mise à jour) — voir `nav.svelte.ts` pour le
  // même schéma sur le duo voiture/circuit.
  const hasPersisted = state.selection !== null || state.presets !== null;
  if (hasPersisted) {
    return { selection: state.selection ?? {}, presets: state.presets ?? {}, fromFile: true };
  }
  return {
    selection: JSON.parse(localStorage.getItem(StorageKey.launchSelection) ?? "{}"),
    presets: JSON.parse(localStorage.getItem(StorageKey.launchPresets) ?? "{}"),
    fromFile: false,
  };
}

// Envoie systématiquement l'état complet (sélection + presets) : la commande
// réécrit tout le fichier à chaque appel, comme `save_session_picks` — un
// envoi partiel effacerait l'autre moitié.
export function saveLaunchState(selection: Selection, presets: TypePresets): void {
  invoke("save_launch_state", { state: { selection, presets } }).catch((e) => console.error("save_launch_state", e));
}
