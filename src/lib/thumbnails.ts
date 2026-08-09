// Miniatures des galeries médias (§6.1) : les captures AC sont en pleine
// résolution jeu, jamais retaillées avant — voir `thumbnails.rs` côté
// backend. Persistées sur disque (app_cache_dir), réutilisées telles quelles
// après un redémarrage de l'app, pas seulement pendant la session courante.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

/** Chemin (asset://) d'une miniature JPEG mise en cache, générée au besoin. */
export function getThumbnail(path: string, maxDim = 320): Promise<string> {
  return invoke<string>("get_thumbnail", { path, maxDim }).then((cached) => convertFileSrc(cached));
}
