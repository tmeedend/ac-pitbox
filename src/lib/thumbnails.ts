// Miniatures des galeries médias (§6.1) : les captures AC sont en pleine
// résolution jeu, jamais retaillées avant — voir `thumbnails.rs` côté
// backend. Persistées sur disque (app_cache_dir), réutilisées telles quelles
// après un redémarrage de l'app, pas seulement pendant la session courante.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

/** Chemin (asset://) d'une miniature JPEG mise en cache, générée au besoin. */
export function getThumbnail(path: string, maxDim = 320): Promise<string> {
  return invoke<string>("get_thumbnail", { path, maxDim }).then((cached) => convertFileSrc(cached));
}

// Concurrence bornée : les captures AC peuvent peser plusieurs dizaines de Mo
// en pleine résolution jeu, parfois bien plus (captures supersampled
// dépassant 100 Mpx, vues en usage réel — jusqu'à 50 s de décodage chacune en
// build de dev, non optimisée). Décoder tout un dossier d'un coup sature
// CPU/RAM au point que la galerie ne finit jamais d'afficher quoi que ce
// soit ; quelques-unes à la fois suffit à garder l'app réactive.
const CONCURRENCY = 3;

/**
 * Charge les miniatures de plusieurs fichiers, `CONCURRENCY` à la fois.
 * `isStale()` est revérifiée avant chaque callback pour ignorer les réponses
 * d'un lot abandonné (changement de fiche pendant le chargement — même
 * principe que ResourcesBlock.svelte).
 */
export function loadThumbnails(
  paths: string[],
  isStale: () => boolean,
  onLoaded: (path: string, src: string) => void,
): void {
  let next = 0;
  async function worker() {
    while (next < paths.length) {
      if (isStale()) return;
      const path = paths[next++];
      try {
        const src = await getThumbnail(path);
        if (!isStale()) onLoaded(path, src);
      } catch {
        // miniature ratée : le fichier reste sans image plutôt que de casser la galerie
      }
    }
  }
  for (let i = 0; i < Math.min(CONCURRENCY, paths.length); i++) worker();
}
