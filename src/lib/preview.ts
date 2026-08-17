// Pont typé vers l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §7).
//
// Le `.glb` ne passe jamais par ici : la commande renvoie une URL servie par
// le protocole `carpreview`, que le chargeur three.js va chercher lui-même
// (§7.2). Le seul objet qui transite est ce petit descripteur.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface CarPreview {
  /** URL à donner à `GLTFLoader`. */
  url: string;
  triangleCount: number;
  materialCount: number;
  textureCount: number;
  /** `true` quand rien n'a été converti — deuxième affichage d'une voiture. */
  fromCache: boolean;
}

/** Étapes émises pendant une conversion, dans cet ordre. */
export type PreviewStage = "geometry" | "textures" | "writing";

/**
 * Prépare l'aperçu d'une voiture. Rejette avec une clé i18n : `errors.preview*`
 * pour les cas attendus (modèle absent, modèle protégé, demande remplacée par
 * une sélection plus récente), un message technique sinon.
 *
 * `errors.previewSuperseded` n'est pas une panne : c'est la réponse normale
 * quand l'utilisateur a changé de voiture avant la fin de la conversion, et
 * l'appelant doit simplement l'ignorer.
 */
export function prepareCarPreview(carId: string, skinId?: string | null): Promise<CarPreview> {
  return invoke<CarPreview>("prepare_car_preview", { carId, skinId: skinId ?? null });
}

/** Vide le cache d'aperçus, renvoie le nombre d'octets libérés. */
export function clearPreviewCache(): Promise<number> {
  return invoke<number>("clear_preview_cache");
}

/**
 * S'abonne à la progression de la conversion en cours, pour alimenter le
 * squelette de chargement (§7.3). Renvoie la fonction de désabonnement.
 */
export function onPreviewProgress(handler: (stage: PreviewStage) => void): Promise<() => void> {
  return listen<PreviewStage>("preview://progress", (event) => handler(event.payload));
}
