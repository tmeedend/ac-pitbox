// Pont typé vers l'écoute du son moteur d'une voiture (docs/fsb5-format.md).
//
// Le WAV voyage **dans le résultat**, en base64, contrairement au `.glb` de
// l'aperçu 3D qui passe par un protocole dédié : une boucle de ralenti pèse
// quelques centaines de kilo-octets, ce qui ne justifie ni protocole ni cache
// sur disque.
import { invoke } from "@tauri-apps/api/core";

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

/** Décode le base64 renvoyé par le backend en tampon prêt pour `decodeAudioData`. */
export function clipToBuffer(clip: EngineClip): ArrayBuffer {
  const binary = atob(clip.wav);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}
