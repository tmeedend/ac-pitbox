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

// Inclut les champs de formulaire (curseurs, nombres, listes déroulantes) —
// sans ça, les réglages de session (carburant, dégâts, ghost car…) sont
// invisibles à la navigation manette : ni atteignables, ni modifiables.
const FOCUSABLE =
  'button:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled]), a[href], ' +
  'input:not([type="hidden"]):not([disabled]), select:not([disabled])';

function focusableElements(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null,
  );
}

// Curseurs/nombres/listes : `.click()` (le geste "confirm" ordinaire) ne fait
// rien d'utile dessus (un select ouvrirait une popup native que la manette ne
// peut pas piloter). Gauche/droite pas à pas remplace la souris pour ces
// éléments une fois qu'ils ont le focus — haut/bas continue de déplacer le
// focus normalement.
function isAdjustable(el: Element | null): el is HTMLInputElement | HTMLSelectElement {
  if (!el) return false;
  if (el instanceof HTMLSelectElement) return true;
  return el instanceof HTMLInputElement && (el.type === "range" || el.type === "number");
}

function stepAdjustable(el: HTMLInputElement | HTMLSelectElement, dir: 1 | -1) {
  if (el instanceof HTMLSelectElement) {
    const next = el.selectedIndex + dir;
    if (next < 0 || next >= el.options.length) return;
    el.selectedIndex = next;
  } else {
    const step = Number(el.step) || 1;
    const min = el.min !== "" ? Number(el.min) : -Infinity;
    const max = el.max !== "" ? Number(el.max) : Infinity;
    const current = Number(el.value) || 0;
    el.value = String(Math.max(min, Math.min(max, current + dir * step)));
  }
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

// Repère visuel garanti pour la sélection manette : `:focus-visible` ne
// suffisait pas (le focus posé par script depuis un scrutin rAF ne déclenche
// pas toujours l'heuristique "focus-visible" de Chromium, contrairement à un
// vrai événement clavier) — on pose donc explicitement une classe, gérée ici,
// plutôt que de compter sur la pseudo-classe native.
const GP_CLASS = "gp-focus";
let gpFocusEl: HTMLElement | null = null;

function setGamepadFocus(el: HTMLElement) {
  gpFocusEl?.classList.remove(GP_CLASS);
  gpFocusEl = el;
  el.classList.add(GP_CLASS);
  el.focus();
}

// Toute prise de focus qui ne vient pas de nous (clic souris, Tab clavier…)
// efface le repère — sinon il resterait collé sur le dernier élément visé
// à la manette même après une interaction souris.
function initFocusTracking() {
  document.addEventListener("focusin", (e) => {
    if (e.target !== gpFocusEl) {
      gpFocusEl?.classList.remove(GP_CLASS);
      gpFocusEl = null;
    }
  });
}

function moveFocus(dir: Dir) {
  const candidates = focusableElements();
  if (!candidates.length) return;
  const current = document.activeElement as HTMLElement | null;
  const from = current && candidates.includes(current) ? current.getBoundingClientRect() : null;
  if (!from) {
    setGamepadFocus(candidates[0]);
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
  if (best) setGamepadFocus(best);
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
  initFocusTracking();
  let raf = 0;
  let last = NONE;

  function poll() {
    for (const gp of navigator.getGamepads?.() ?? []) {
      if (!gp) continue;
      const cur = readButtons(gp);

      if (nav.lightboxOpen) {
        // Visionneuse plein écran ouverte par-dessus la fiche (§6.1,
        // Lightbox.svelte) : elle gère elle-même tout son input manette
        // (gauche/droite/B), y compris la fermeture — ne rien faire ici, sous
        // peine qu'un même B ferme à la fois la visionneuse et la fiche.
      } else if (nav.openFull) {
        // La fiche pleine page gère elle-même gauche/droite (mod précédent/
        // suivant, voir Library.svelte::navigateFull) — ici uniquement B=fermer.
        if (cur.back && !last.back) nav.openFull = null;
      } else {
        const active = document.activeElement;
        if (isAdjustable(active)) {
          // Gauche/droite règle la valeur du champ ciblé plutôt que de
          // changer le focus ; haut/bas continue de naviguer normalement.
          if (cur.left && !last.left) stepAdjustable(active, -1);
          if (cur.right && !last.right) stepAdjustable(active, 1);
          if (cur.up && !last.up) moveFocus("up");
          if (cur.down && !last.down) moveFocus("down");
          if (cur.confirm && !last.confirm && !(active instanceof HTMLSelectElement)) active.click();
        } else {
          if (cur.up && !last.up) moveFocus("up");
          if (cur.down && !last.down) moveFocus("down");
          if (cur.left && !last.left) moveFocus("left");
          if (cur.right && !last.right) moveFocus("right");
          if (cur.confirm && !last.confirm) (document.activeElement as HTMLElement | null)?.click();
        }
      }
      last = cur;
    }
    raf = requestAnimationFrame(poll);
  }

  raf = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(raf);
}
