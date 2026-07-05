// Navigation manette dans toute l'application : la croix directionnelle (ou
// le stick gauche) déplace le focus clavier parmi les éléments interactifs
// visibles (cartes de la bibliothèque, boutons du menu latéral…), le bouton
// principal (A sur Xbox, Croix/X sur PlayStation — button[0] dans le mapping
// "standard" du navigateur, donc le même code marche pour la plupart des
// manettes/volants reconnus) valide l'élément ciblé, et le bouton secondaire
// (B/Rond — button[1]) ferme la fiche pleine page d'un mod si elle est ouverte.
//
// Approche générique par géométrie (plus proche voisin dans la direction
// visée), plutôt que du câblage par écran : marche pour n'importe quelle vue
// sans code spécifique, y compris pour passer de la grille au menu latéral en
// allant à gauche. Limite connue : ne « piège » pas le focus dans une modale
// ouverte par-dessus (BulkImport, sélection d'adversaire…) — hors périmètre
// demandé pour l'instant.

import { nav } from "$lib/nav.svelte";

type Dir = "up" | "down" | "left" | "right";

const FOCUSABLE = 'button:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled]), a[href]';

function focusableElements(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null,
  );
}

function moveFocus(dir: Dir) {
  const candidates = focusableElements();
  if (!candidates.length) return;
  const current = document.activeElement as HTMLElement | null;
  const from = current && candidates.includes(current) ? current.getBoundingClientRect() : null;
  if (!from) {
    candidates[0].focus();
    return;
  }
  const fromCenter = { x: from.left + from.width / 2, y: from.top + from.height / 2 };
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of candidates) {
    if (el === current) continue;
    const r = el.getBoundingClientRect();
    const c = { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    const dx = c.x - fromCenter.x;
    const dy = c.y - fromCenter.y;
    let primary: number;
    switch (dir) {
      case "up":
        if (dy >= -1) continue;
        primary = -dy;
        break;
      case "down":
        if (dy <= 1) continue;
        primary = dy;
        break;
      case "left":
        if (dx >= -1) continue;
        primary = -dx;
        break;
      case "right":
        if (dx <= 1) continue;
        primary = dx;
        break;
    }
    // Pénalise l'écart perpendiculaire : privilégie rester sur la même
    // ligne/colonne plutôt que sauter en diagonale.
    const secondary = dir === "up" || dir === "down" ? Math.abs(dx) : Math.abs(dy);
    const score = primary + secondary * 3;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  best?.focus();
}

interface ButtonEdges {
  up: boolean;
  down: boolean;
  left: boolean;
  right: boolean;
  confirm: boolean;
  back: boolean;
}

const AXIS_THRESHOLD = 0.6;

function readButtons(gp: Gamepad): ButtonEdges {
  const ax = gp.axes[0] ?? 0;
  const ay = gp.axes[1] ?? 0;
  return {
    up: (gp.buttons[12]?.pressed ?? false) || ay < -AXIS_THRESHOLD,
    down: (gp.buttons[13]?.pressed ?? false) || ay > AXIS_THRESHOLD,
    left: (gp.buttons[14]?.pressed ?? false) || ax < -AXIS_THRESHOLD,
    right: (gp.buttons[15]?.pressed ?? false) || ax > AXIS_THRESHOLD,
    confirm: gp.buttons[0]?.pressed ?? false,
    back: gp.buttons[1]?.pressed ?? false,
  };
}

const NONE: ButtonEdges = { up: false, down: false, left: false, right: false, confirm: false, back: false };

/** Démarre le scrutin manette global. Retourne une fonction d'arrêt. */
export function startGamepadNav(): () => void {
  let raf = 0;
  let last = NONE;

  function poll() {
    for (const gp of navigator.getGamepads?.() ?? []) {
      if (!gp) continue;
      const cur = readButtons(gp);

      if (nav.openFull) {
        // La fiche pleine page gère elle-même gauche/droite (mod précédent/
        // suivant, voir Library.svelte::navigateFull) — ici uniquement B=fermer.
        if (cur.back && !last.back) nav.openFull = null;
      } else {
        if (cur.up && !last.up) moveFocus("up");
        if (cur.down && !last.down) moveFocus("down");
        if (cur.left && !last.left) moveFocus("left");
        if (cur.right && !last.right) moveFocus("right");
        if (cur.confirm && !last.confirm) (document.activeElement as HTMLElement | null)?.click();
      }
      last = cur;
    }
    raf = requestAnimationFrame(poll);
  }

  raf = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(raf);
}
