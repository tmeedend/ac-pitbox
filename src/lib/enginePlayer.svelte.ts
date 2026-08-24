// Lecture du son moteur dans la fiche voiture — un seul moteur à la fois.
//
// L'audio vit ici, en `$state` de module, et pas dans le composant : la fiche
// se démonte quand on change de voiture, et un son qui survit à son bouton est
// un son qu'on ne peut plus couper. L'exclusivité suit la même logique que les
// boutons radio de la liste — démarrer une entrée coupe la précédente.
//
// Rien à voir avec le moteur audio de la musique Big Picture (`music/engine.rs`,
// côté Rust) : celui-ci est purement Web Audio, dans la webview, et n'a ni
// playlist ni fondu enchaîné à gérer.
import { auditionEngineSound, clipToBuffer, type EngineClip } from "$lib/enginesound";

/** Fondu d'entrée et de sortie. Sans lui, démarrer ou couper claque. */
const FADE_SECONDS = 0.12;

/** Identifie une entrée de la liste : la voiture, et le mod ou l'origine. */
function keyOf(parentId: string, subId: string | null): string {
  return `${parentId}:${subId ?? ""}`;
}

let context: AudioContext | null = null;
let source: AudioBufferSourceNode | null = null;
let gain: GainNode | null = null;

/** Entrée en cours de lecture, `null` si rien ne tourne. */
let playing = $state<string | null>(null);
/** Entrée dont le fichier est en cours de lecture sur disque. */
let loading = $state<string | null>(null);

/** Les tampons déjà décodés, par entrée. Rouvrir la même voiture pour comparer
 * deux mods est le geste normal ici : le refaire décoder à chaque clic serait
 * une attente qu'on peut s'épargner. */
const decoded = new Map<string, AudioBuffer>();

export function engineState(parentId: string, subId: string | null): "off" | "loading" | "on" {
  const key = keyOf(parentId, subId);
  if (playing === key) return "on";
  if (loading === key) return "loading";
  return "off";
}

/** Coupe ce qui tourne, en fondu. */
export function stopEngine(): void {
  playing = null;
  if (!context || !gain || !source) return;
  const stopping = source;
  const now = context.currentTime;
  gain.gain.cancelScheduledValues(now);
  gain.gain.setValueAtTime(gain.gain.value, now);
  gain.gain.linearRampToValueAtTime(0, now + FADE_SECONDS);
  // Le nœud est jetable : on l'arrête après le fondu, jamais avant, sinon le
  // fondu ne s'entend pas.
  stopping.stop(now + FADE_SECONDS + 0.02);
  source = null;
  gain = null;
}

/**
 * Démarre ou coupe l'entrée demandée.
 *
 * `AudioContext` n'est construit qu'ici, au premier clic : les navigateurs
 * refusent de le laisser démarrer hors d'un geste de l'utilisateur, et en
 * construire un au montage de la fiche le laisserait suspendu.
 */
export async function toggleEngine(
  parentId: string,
  subId: string | null,
): Promise<EngineClip | null> {
  const key = keyOf(parentId, subId);
  // **Un clic pendant le chargement est ignoré.** Lire un bank de trente
  // mégaoctets et y chercher le ralenti prend un instant, pendant lequel rien
  // ne bougeait : l'utilisateur recliquait, et le second appel coupait le
  // premier pour relancer un décodage par-dessus. La clé dit maintenant que ça
  // vient (état `loading`), et les clics d'impatience ne font plus rien.
  if (loading !== null) return null;
  if (playing === key) {
    stopEngine();
    return null;
  }
  stopEngine();

  context ??= new AudioContext();
  // Un contexte créé avant un geste peut être suspendu : le réveiller ne coûte
  // rien quand il ne l'est pas.
  if (context.state === "suspended") await context.resume();

  let buffer = decoded.get(key);
  let clip: EngineClip | null = null;
  if (!buffer) {
    loading = key;
    try {
      clip = await auditionEngineSound(parentId, subId);
      buffer = await context.decodeAudioData(clipToBuffer(clip));
      decoded.set(key, buffer);
    } finally {
      if (loading === key) loading = null;
    }
  }

  // L'utilisateur a pu cliquer ailleurs pendant la lecture du fichier : ne
  // jamais démarrer un son que plus personne n'attend.
  if (loading !== null && loading !== key) return clip;

  const node = context.createBufferSource();
  node.buffer = buffer;
  node.loop = true;
  const volume = context.createGain();
  const now = context.currentTime;
  volume.gain.setValueAtTime(0, now);
  volume.gain.linearRampToValueAtTime(1, now + FADE_SECONDS);
  node.connect(volume);
  volume.connect(context.destination);
  node.start();

  source = node;
  gain = volume;
  playing = key;
  return clip;
}
