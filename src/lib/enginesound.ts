// Pont typé vers l'écoute du son moteur d'une voiture (docs/fsb5-format.md).
//
// Le WAV voyage **dans le résultat**, en base64, contrairement au `.glb` de
// l'aperçu 3D qui passe par un protocole dédié : une boucle de ralenti pèse
// quelques centaines de kilo-octets, ce qui ne justifie ni protocole ni cache
// sur disque.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { ResourceFile } from "$lib/library";

export interface EngineClip {
  /** Un fichier WAV complet, encodé en base64. */
  wav: string;
  frequency: number;
  seconds: number;
  /** Diagnostics, jamais affichés tels quels — utiles dans un rapport de bug. */
  codec: string;
  sampleIndex: number;
  sampleName: string | null;
  /** `"name"` si l'échantillon a été trouvé par son nom, `"pitch"` s'il a fallu
   * le reconnaître à l'oreille de la machine (les mods n'ont pas de noms). */
  pickedBy: "name" | "pitch";
}

/**
 * Lit le son d'une entrée de la liste « Son du moteur » et renvoie une boucle
 * de ralenti. `subId` à `null` désigne le son d'origine.
 *
 * **Ne déploie rien** : c'est `activateSound` qui remplace les fichiers du jeu.
 * Rejette avec une clé i18n (`errors.sound*`) que `errorText` sait résoudre.
 */
export function auditionEngineSound(parentId: string, subId: string | null): Promise<EngineClip> {
  return invoke<EngineClip>("audition_engine_sound", { parentId, subId });
}

// --- Écoute native, par le FMOD du jeu (docs/SPEC-engine-sound-fmod.md) -----

/** Ce que le vrai moteur audio du jeu renvoie quand il a réussi à jouer. */
export interface NativeAudition {
  /** L'événement effectivement joué, tel que le `GUIDs.txt` l'orthographie —
   * diagnostic, jamais affiché tel quel. */
  eventPath: string;
  /** Nom du paramètre de régime reconnu, `null` si l'événement n'en expose
   * aucun : il se joue quand même, il ne se règle simplement pas. */
  revParam: string | null;
  revMin: number | null;
  revMax: number | null;
  throttleParam: string | null;
  /** Plage du curseur, qui vient de la **voiture** (sa courbe de puissance) et
   * non de l'événement — un F1 monte à 19 500, un Berlingo à 5 000. */
  revFloor: number;
  revCeiling: number;
  revStart: number;
}

/**
 * Joue l'événement moteur par les DLL FMOD d'Assetto Corsa.
 *
 * **Rejette quand le chemin natif n'est pas disponible** — pas d'AC configuré,
 * DLL introuvables, aucun événement moteur. Ce rejet est un signal de repli
 * vers `auditionEngineSound`, pas un message à afficher : `enginePlayer` s'en
 * charge et n'en dit rien à l'écran.
 */
export function auditionEngineNative(
  parentId: string,
  subId: string | null,
  interior = false,
): Promise<NativeAudition> {
  return invoke<NativeAudition>("audition_engine_native", { parentId, subId, interior });
}

/** Règle le régime de l'écoute native en cours. Sans effet si rien ne joue. */
export function setAuditionRev(rev: number): Promise<void> {
  return invoke<void>("set_audition_rev", { rev });
}

/**
 * Déplace l'oreille autour de la voiture : angle d'orbite et hauteur en degrés,
 * distance en mètres.
 *
 * L'événement moteur d'AC est spatialisé et expose `Event Cone Angle` en
 * paramètre **automatique** : c'est FMOD qui change le timbre entre l'avant et
 * l'arrière, on ne fait que dire où on se trouve.
 */
export function setAuditionListener(
  azimuth: number,
  elevation: number,
  distance: number,
): Promise<void> {
  return invoke<void>("set_audition_listener", { azimuth, elevation, distance });
}

/** Lance ou coupe les coups d'accélérateur. */
export function setAuditionShowcase(on: boolean): Promise<void> {
  return invoke<void>("set_audition_showcase", { on });
}

/** Coupe l'écoute native. Sans effet si rien ne joue. */
export function stopAuditionNative(): Promise<void> {
  return invoke<void>("stop_audition_native");
}

/** Décode le base64 renvoyé par le backend en tampon prêt pour `decodeAudioData`. */
export function clipToBuffer(clip: EngineClip): ArrayBuffer {
  const binary = atob(clip.wav);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

// --- Fiche d'un mod de son -------------------------------------------------

/** Ce qu'un bank contient réellement — la partie de la fiche qu'aucun autre
 * outil n'affiche, parce qu'il faut décoder le conteneur pour la connaître. */
export interface BankFacts {
  fileName: string;
  codec: string;
  sampleCount: number;
  frequency: number;
  seconds: number;
  /** Faux quand le mod a supprimé la table des noms — le cas courant, et la
   * raison pour laquelle le ralenti se trouve à la mesure. */
  named: boolean;
  sizeBytes: number;
}

export interface SoundDetail {
  id: string;
  name: string;
  parentId: string;
  parentName: string | null;
  author: string | null;
  sourceArchive: string | null;
  importedAt: string;
  isActive: boolean;
  removable: boolean;
  sizeBytes: number;
  /** `null` quand le bank n'a pas pu être ouvert : la fiche le dit au lieu de
   * faire croire à un mod vide. */
  bank: BankFacts | null;
}

export function soundDetail(subId: string): Promise<SoundDetail> {
  return invoke<SoundDetail>("sound_detail", { subId });
}

/** Enregistre l'auteur, saisi à la main : aucun fichier de mod ne le porte. */
export function setSoundAuthor(subId: string, author: string | null): Promise<void> {
  return invoke<void>("set_sound_author", { subId, author });
}

// Ressources d'un mod de son (§4.5.2) — mêmes cinq opérations que pour un mod,
// une app ou un pack.
export function listSoundResources(subId: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_sound_resources", { subId });
}

export function openSoundResource(subId: string, relPath: string): Promise<void> {
  return invoke<void>("open_sound_resource", { subId, relPath });
}

export function soundResourcePath(subId: string, relPath: string): Promise<string> {
  return invoke<string>("get_sound_resource_path", { subId, relPath });
}

export async function soundResourceSrc(subId: string, relPath: string): Promise<string> {
  return convertFileSrc(await soundResourcePath(subId, relPath));
}

export function readSoundResource(subId: string, relPath: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_sound_resource", { subId, relPath });
}
