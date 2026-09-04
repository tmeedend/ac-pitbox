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
import {
  auditionEngineNative,
  auditionEngineSound,
  clipToBuffer,
  setAuditionListener,
  setAuditionPedal,
  setAuditionRev,
  setAuditionShowcase,
  stopAuditionNative,
  type EngineClip,
  type NativeAudition,
} from "$lib/enginesound";

/** Fondu d'entrée et de sortie. Sans lui, démarrer ou couper claque. */
const FADE_SECONDS = 0.12;

/** Identifie une entrée de la liste : la voiture, et le mod ou l'origine. */
function keyOf(parentId: string, subId: string | null): string {
  return `${parentId}:${subId ?? ""}`;
}

/** L'écoute native tourne côté Rust, pas dans la webview : ceci n'en est que
 * le reflet, pour savoir quoi couper et s'il faut afficher le curseur. */
let native = $state<NativeAudition | null>(null);
/** Régime demandé, en tr/min. Vit ici et non dans le composant, pour la même
 * raison que le reste : la fiche se démonte, le son non. */
let rev = $state(0);
let showcase = $state(false);

/** Dernier angle envoyé au moteur, pour ne pas répéter le même. */
let lastAngle = { azimuth: 999, elevation: 999, distance: 0 };
/** Horodatage du dernier envoi — l'aperçu 3D appelle à chaque image. */
let lastAngleAt = 0;

/** Écart minimal, en degrés, qui vaut la peine d'un aller vers Rust. */
const ANGLE_EPSILON = 1.5;
/** Et jamais plus souvent que ça, en millisecondes. Le timbre suit largement
 * à cette cadence, et l'aperçu tourne lui à 60 images par seconde. */
const ANGLE_INTERVAL = 60;

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

/** Un moteur tourne-t-il, quel qu'il soit ?
 *
 * Sans l'entrée qui joue : l'aperçu 3D n'a qu'une question, « y a-t-il
 * quelqu'un pour tourner la clé », et il n'affiche jamais qu'une voiture à la
 * fois — celle dont on écoute le son. */
export function engineRunning(): boolean {
  return playing !== null;
}

export function engineState(parentId: string, subId: string | null): "off" | "loading" | "on" {
  const key = keyOf(parentId, subId);
  if (playing === key) return "on";
  if (loading === key) return "loading";
  return "off";
}

/** Réglages de l'écoute en cours, `null` quand c'est le repli Web Audio qui
 * joue — le décodeur maison rend un échantillon figé, il n'y a rien à régler. */
export function engineControls(): NativeAudition | null {
  return native;
}

/** Régime actuellement demandé, en tr/min. */
export function engineRev(): number {
  return rev;
}

/** Déplace le régime de l'écoute native en cours.
 *
 * L'appel part sans être attendu : le curseur doit suivre la main, et une
 * aller-retour vers Rust par pixel parcouru la ferait traîner. Un échec est
 * sans conséquence — le thread ignore un réglage quand rien ne joue.
 *
 * **Le sens du déplacement porte le gaz**, et c'est Rust qui le déduit : vers
 * la droite on accélère, vers la gauche on lève le pied, curseur posé on tient
 * le régime. Rien à envoyer d'autre d'ici — il n'y a volontairement plus de
 * réglage d'accélérateur séparé, il serait écrasé au tick suivant. */
export function setEngineRev(value: number): void {
  rev = value;
  // Prendre le curseur en main arrête la démonstration : côté Rust aussi, pour
  // que les deux ne se disputent pas le même paramètre.
  showcase = false;
  void setAuditionRev(value).catch(() => {});
}

/** Holding the mouse button down on the rev slider is holding the pedal down;
 * letting go is lifting off.
 *
 * Sent apart from the engine speed because it answers what a movement cannot:
 * what a STILL slider means. Held still, button down, is an engine held ON the
 * throttle - reported: keeping the slider in place made the sound fall back to
 * the off-throttle layers, stillness alone reading as "slider put down". What a
 * MOVING slider means is unchanged: dragging down is a lift-off, button held or
 * not, because that is the deceleration and the hand controls how far it goes.
 *
 * Pressing is a takeover like moving, showcase included - the routine drives
 * the same parameters and would argue with the hand on the slider. */
export function setEnginePedal(down: boolean): void {
  if (down) showcase = false;
  void setAuditionPedal(down).catch(() => {});
}

/** Les coups d'accélérateur tournent-ils ? */
export function engineShowcase(): boolean {
  return showcase;
}

/** Lance ou coupe les coups d'accélérateur. */
export function setEngineShowcase(on: boolean): void {
  showcase = on;
  void setAuditionShowcase(on).catch(() => {});
}

/**
 * Position de l'oreille, appelée par l'aperçu 3D **à chaque image**.
 *
 * D'où le filtrage ici plutôt que chez l'appelant : ce module sait s'il y a
 * quelque chose à spatialiser, l'aperçu ne le sait pas et n'a pas à le savoir.
 * Sans écoute native en cours, l'appel ne coûte rien.
 */
export function reportListenerAngle(azimuth: number, elevation: number, distance: number): void {
  if (!native) return;
  const now = performance.now();
  const moved =
    Math.abs(azimuth - lastAngle.azimuth) > ANGLE_EPSILON ||
    Math.abs(elevation - lastAngle.elevation) > ANGLE_EPSILON ||
    Math.abs(distance - lastAngle.distance) > 0.25;
  if (!moved || now - lastAngleAt < ANGLE_INTERVAL) return;
  lastAngle = { azimuth, elevation, distance };
  lastAngleAt = now;
  void setAuditionListener(azimuth, elevation, distance).catch(() => {});
}

/** Coupe ce qui tourne, en fondu. */
export function stopEngine(): void {
  playing = null;
  showcase = false;
  if (native) {
    native = null;
    void stopAuditionNative().catch(() => {});
  }
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

  // **Le vrai moteur du jeu d'abord.** Il joue l'événement `engine_ext` que le
  // jeu jouerait, réglable en régime, au lieu d'un échantillon deviné — mesuré
  // sur les 299 voitures de l'installation de référence, il aboutit à chaque
  // fois (docs/SPEC-engine-sound-fmod.md §5, lot 4).
  //
  // Un échec ici n'est **pas** une erreur à montrer : pas d'AC configuré, DLL
  // absentes, bank refusé. C'est le basculement vers le décodeur maison, qui
  // reste le seul chemin fonctionnant sans installation du jeu (§4.1). Il n'en
  // reste qu'une ligne de journal.
  loading = key;
  try {
    const audition = await auditionEngineNative(parentId, subId);
    native = audition;
    rev = audition.revStart;
    showcase = false;
    // Un nouvel événement, une nouvelle instance : le prochain angle doit
    // repartir, même si la caméra n'a pas bougé entre-temps.
    lastAngle = { azimuth: 999, elevation: 999, distance: 0 };
    playing = key;
    return null;
  } catch (e) {
    console.info(`[son moteur] chemin natif indisponible, repli sur le décodeur : ${e}`);
  } finally {
    if (loading === key) loading = null;
  }

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
