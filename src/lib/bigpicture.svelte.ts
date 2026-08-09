// Mode Big Picture : fenêtre plein écran + zoom dédié + démarrage/arrêt de
// l'ambiance musicale (docs/spec-module-musique_2.md). L'app ne propose pas
// (encore) d'interface "10-foot" dédiée : c'est l'UI existante, agrandie.
import { getCurrentWindow, currentMonitor, type PhysicalPosition, type PhysicalSize } from "@tauri-apps/api/window";
import { setZoom } from "./zoom.svelte";
import { getConfig } from "./config";
import { musicEnterBigPicture, musicExitBigPicture } from "./music";

export const bigPictureState = $state<{ active: boolean }>({ active: false });

// Restaurés à la sortie — taille/position/zoom en place avant l'entrée en
// Big Picture, pas forcément les valeurs par défaut si l'utilisateur avait
// déjà déplacé/redimensionné la fenêtre ou changé le zoom ailleurs.
let previousZoom: number | null = null;
let previousSize: PhysicalSize | null = null;
let previousPosition: PhysicalPosition | null = null;

export async function enterBigPicture(): Promise<void> {
  if (bigPictureState.active) return;
  const cfg = await getConfig();
  const win = getCurrentWindow();
  previousZoom = cfg.prefs.ui_zoom;
  previousSize = await win.outerSize();
  previousPosition = await win.outerPosition();

  await win.setFullscreen(true);
  // Filet de sécurité Windows : sur une fenêtre sans décorations,
  // `setFullscreen` couvre parfois seulement la zone de travail (écran moins
  // la barre des tâches) plutôt que l'écran entier — bug constaté (zone en
  // bas de l'écran, là où était la barre des tâches, qui restait hors de la
  // fenêtre). On force explicitement les bornes du moniteur pour ne jamais
  // laisser de bande non couverte.
  const monitor = await currentMonitor();
  if (monitor) {
    await win.setPosition(monitor.position);
    await win.setSize(monitor.size);
  }

  setZoom(cfg.prefs.bigpicture_zoom ?? cfg.prefs.ui_zoom);
  bigPictureState.active = true;
  // Musique après le passage en plein écran/zoom : si `enabled` est faux
  // côté réglages, le moteur ne joue rien (§2, coupe-circuit) — pas de
  // condition à dupliquer côté frontend.
  await musicEnterBigPicture();
}

export async function exitBigPicture(): Promise<void> {
  if (!bigPictureState.active) return;
  const win = getCurrentWindow();
  await win.setFullscreen(false);
  // Restaure explicitement (voir le filet de sécurité ci-dessus : on a pu
  // redimensionner nous-mêmes la fenêtre au-delà de ce que `setFullscreen`
  // sait annuler tout seul).
  if (previousSize) await win.setSize(previousSize);
  if (previousPosition) await win.setPosition(previousPosition);
  setZoom(previousZoom);
  bigPictureState.active = false;
  await musicExitBigPicture();
}
