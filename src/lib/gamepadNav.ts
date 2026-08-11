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
import { peekUiPref } from "$lib/uiPrefs.svelte";

type Dir = "up" | "down" | "left" | "right";

// Réglage utilisateur (Réglages > Général) : "" (ou absent) = auto (filtre
// mapping standard ci-dessous), "off" = désactivé, sinon l'`id` exact d'une
// manette (`Gamepad.id`) à utiliser explicitement — filet de sécurité pour un
// périphérique qui échapperait au filtre auto (ex. un volant qui se déclare
// "standard" via une couche de compatibilité XInput).
export const GAMEPAD_NAV_MODE_KEY = "pitbox.gamepadNav.mode";

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

// Curseurs (`type="range"`) : `.click()` ne fait rien d'utile dessus, gauche/
// droite pas à pas remplace la souris dès le focus, sans geste d'entrée — un
// curseur n'a pas de popup native à éviter, gauche/droite y est déjà
// l'équivalent naturel d'un clic-glisse. Haut/bas continue de déplacer le
// focus normalement.
function isAdjustable(el: Element | null): el is HTMLInputElement {
  if (!el) return false;
  return el instanceof HTMLInputElement && el.type === "range";
}

// Champs qui ont besoin d'un geste d'« entrée » avant que gauche/droite
// change leur valeur : listes déroulantes (`<select>`, popup native que la
// manette ne peut pas piloter) et champs numériques (`type="number"`, ex.
// année min/max de la bibliothèque). Sans ce geste, gauche/droite au simple
// survol changeait la valeur au lieu de déplacer le focus — deux bugs réels
// signalés : les filtres de bibliothèque changeaient en passant dessus, et le
// champ année ne pouvait plus être quitté par gauche/droite (piégé, valeur
// qui grimpe sans fin). Voir `entered` dans `startGamepadNav`.
function needsEntry(el: Element | null): el is HTMLSelectElement | HTMLInputElement {
  if (!el) return false;
  if (el instanceof HTMLSelectElement) return true;
  return el instanceof HTMLInputElement && el.type === "number";
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

// Régions de mise en page (AppShell.svelte : `.side` = menu latéral, `.content`
// = zone d'écran active). Le plus proche voisin géométrique seul peut
// préférer un bouton du menu latéral — horizontalement proche du bord gauche
// du contenu — à une carte de la grille plus bas dans le contenu : bug réel
// signalé, "bas" depuis les filtres de bibliothèque retombait sur le menu
// latéral au lieu d'entrer dans la grille. `moveFocus` cherche donc d'abord
// un candidat dans la même région que l'élément courant, et ne se rabat sur
// toutes les régions que si la région courante n'a rien dans cette direction
// — c'est ce repli qui préserve le passage intentionnel grille → menu en
// allant à gauche (documenté plus haut) : la grille n'a plus rien à gauche
// d'elle-même, donc le repli hors-région prend le relais normalement.
const REGION_SELECTOR = ".side, .content";

function regionOf(el: Element): Element | null {
  return el.closest(REGION_SELECTOR);
}

function bestCandidate(
  candidates: HTMLElement[],
  current: HTMLElement,
  fromCenter: { x: number; y: number },
  dir: Dir,
  region: Element | null,
): HTMLElement | null {
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of candidates) {
    if (el === current) continue;
    if (region && regionOf(el) !== region) continue;
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
  return best;
}

function moveFocus(dir: Dir) {
  const candidates = focusableElements();
  if (!candidates.length) return;
  const current = document.activeElement as HTMLElement | null;
  const from = current && candidates.includes(current) ? current.getBoundingClientRect() : null;
  if (!from || !current) {
    setGamepadFocus(candidates[0]);
    return;
  }
  const fromCenter = { x: from.left + from.width / 2, y: from.top + from.height / 2 };
  const region = regionOf(current);
  const best =
    bestCandidate(candidates, current, fromCenter, dir, region) ??
    bestCandidate(candidates, current, fromCenter, dir, null);
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

// Table d'overrides par périphérique (Réglages > Général affiche un tableau
// de diagnostic — mapping/axes/boutons en direct — servant justement à
// relever les valeurs ci-dessous pour un nouveau volant). Chaque volant a un
// layout non standard qui lui est propre, sans norme fiable : rien ne garantit
// qu'un autre modèle (même une autre rim Fanatec) partage ces codes, donc pas
// de généralisation tentée — un override par modèle constaté, ajouté au fil
// de l'eau.
interface DeviceOverride {
  // Sous-chaîne de `Gamepad.id` : un même modèle peut s'annoncer sur deux
  // entrées `Gamepad` distinctes au même `id` (base + interface boutons
  // séparée, cas réel constaté sur une base Fanatec ClubSport V2.5) — matcher
  // les deux plutôt que de dépendre d'un `index` non garanti stable d'un
  // redémarrage à l'autre.
  idIncludes: string;
  confirmButton: number;
  backButton: number;
  // Croix directionnelle rapportée comme un seul axe à valeurs discrètes
  // (hat switch matériel) plutôt que 4 boutons ou un vrai stick 2D — valeurs
  // lues empiriquement, propres à ce modèle/pilote précis.
  hat: { up: number; down: number; left: number; right: number };
}

const HAT_TOLERANCE = 0.08;

const DEVICE_OVERRIDES: DeviceOverride[] = [
  {
    idIncludes: "FANATEC ClubSport Wheel Base V2.5",
    confirmButton: 0,
    backButton: 1,
    hat: { up: -1, right: -3 / 7, down: 1 / 7, left: 5 / 7 },
  },
];

function findDeviceOverride(gp: Gamepad): DeviceOverride | undefined {
  return DEVICE_OVERRIDES.find((o) => gp.id.includes(o.idIncludes));
}

// L'axe qui porte le hat switch n'a pas d'index fixe garanti (dépend de
// l'ordre d'énumération HID) : on le retrouve à chaque lecture en cherchant,
// parmi tous les axes du périphérique, celui dont la valeur est proche d'une
// des 4 positions connues — auto-localisation plutôt qu'un index en dur.
function readOverrideButtons(gp: Gamepad, o: DeviceOverride): ButtonEdges {
  let hat: number | undefined;
  for (const v of gp.axes) {
    if (
      Math.abs(v - o.hat.up) < HAT_TOLERANCE ||
      Math.abs(v - o.hat.down) < HAT_TOLERANCE ||
      Math.abs(v - o.hat.left) < HAT_TOLERANCE ||
      Math.abs(v - o.hat.right) < HAT_TOLERANCE
    ) {
      hat = v;
      break;
    }
  }
  return {
    up: hat !== undefined && Math.abs(hat - o.hat.up) < HAT_TOLERANCE,
    down: hat !== undefined && Math.abs(hat - o.hat.down) < HAT_TOLERANCE,
    left: hat !== undefined && Math.abs(hat - o.hat.left) < HAT_TOLERANCE,
    right: hat !== undefined && Math.abs(hat - o.hat.right) < HAT_TOLERANCE,
    confirm: gp.buttons[o.confirmButton]?.pressed ?? false,
    back: gp.buttons[o.backButton]?.pressed ?? false,
  };
}

const NONE: ButtonEdges = { up: false, down: false, left: false, right: false, confirm: false, back: false };

// Répétition en rester appuyé (haut/bas/gauche/droite uniquement — jamais
// confirm/back, un clic répété en boucle n'a pas de sens) : sans ça, parcourir
// une longue liste (tableau bibliothèque…) demandait un appui par ligne — bug
// réel signalé. Décollage après un court délai (évite qu'un simple appui
// ponctuel morde sur la répétition), puis un rythme constant, qui accélère
// après un maintien prolongé — mêmes seuils qu'un répétiteur clavier standard.
const REPEAT_DELAY_MS = 380;
const REPEAT_INTERVAL_MS = 130;
const REPEAT_INTERVAL_FAST_MS = 60;
const REPEAT_ACCEL_AFTER_MS = 1500;

interface DirRepeatState {
  since: number;
  nextFire: number;
}

function makeDirRepeat(): Map<number, Partial<Record<Dir, DirRepeatState>>> {
  return new Map();
}

// `held` = état courant du bouton/axe pour cette direction ce tick ; l'appui
// initial déclenche tout de suite (comme avant), puis la répétition prend le
// relais tant que `held` reste vrai. Un état par (manette, direction) — même
// raison que `lastByGamepad` : ne pas mélanger plusieurs manettes.
function shouldFire(
  repeats: Map<number, Partial<Record<Dir, DirRepeatState>>>,
  gpIndex: number,
  dir: Dir,
  held: boolean,
  now: number,
): boolean {
  let dirs = repeats.get(gpIndex);
  if (!dirs) {
    dirs = {};
    repeats.set(gpIndex, dirs);
  }
  if (!held) {
    delete dirs[dir];
    return false;
  }
  const state = dirs[dir];
  if (!state) {
    dirs[dir] = { since: now, nextFire: now + REPEAT_DELAY_MS };
    return true;
  }
  if (now < state.nextFire) return false;
  const interval = now - state.since > REPEAT_ACCEL_AFTER_MS ? REPEAT_INTERVAL_FAST_MS : REPEAT_INTERVAL_MS;
  state.nextFire = now + interval;
  return true;
}

/** Démarre le scrutin manette global. Retourne une fonction d'arrêt. */
export function startGamepadNav(): () => void {
  initFocusTracking();
  let raf = 0;
  // Un état précédent par manette (clé = `gp.index`), jamais une variable
  // partagée : bug réel constaté — avec une seule variable `last` réutilisée
  // pour toutes les manettes du tableau, la manette fantôme que Windows/
  // Chromium ajoute parfois (Bluetooth, Steam, pilotes de volant…) écrase
  // `last` à chaque frame, ce qui fait relire un front montant à une manette
  // réelle dont le bouton confirm reste simplement appuyé — `.click()` part
  // en boucle sur l'élément focus (ici une case à cocher, qui bascule et
  // émet un vrai `change` à chaque appel) au lieu d'un seul déclenchement.
  const lastByGamepad = new Map<number, ButtonEdges>();
  const dirRepeats = makeDirRepeat();

  // Champ « entré » (confirm appuyé dessus une première fois) — liste
  // déroulante ou champ numérique, voir `needsEntry` : tant qu'il n'est pas
  // entré, gauche/droite déplace le focus normalement (un filtre est un champ
  // parmi d'autres) ; une fois entré, gauche/droite change sa valeur, et un
  // nouvel appui sur confirm en ressort. Remise à zéro dès que le focus quitte
  // ce champ par un autre moyen (clic souris, focus posé ailleurs par le code).
  let entered: HTMLSelectElement | HTMLInputElement | null = null;

  function poll() {
    const mode = peekUiPref(GAMEPAD_NAV_MODE_KEY) || "auto";
    if (mode === "off") {
      raf = requestAnimationFrame(poll);
      return;
    }
    const now = performance.now();
    for (const gp of navigator.getGamepads?.() ?? []) {
      if (!gp?.connected) continue;
      const override = findDeviceOverride(gp);
      if (mode === "auto") {
        // Un volant (Fanatec…) n'a pas le layout Xbox que ce module suppose
        // (axes[0..1] = stick gauche, boutons 12-15 = croix) : ses axes
        // correspondent à des pédales/à la rotation du volant, ses boutons à
        // autre chose. Chrome ne marque `mapping === "standard"` que pour les
        // manettes dont il reconnaît le layout — jamais pour un volant. Sans
        // ce filtre, un axe non standard au repos au-dessus du seuil (ou
        // simplement bruité, ce qui refait franchir le seuil à chaque frame)
        // déclenche "bas" en boucle — bug réel constaté : volant Fanatec
        // allumé, aucune autre manette branchée, sélecteur qui ne va que vers
        // le bas, et les boutons gauche/droite/valider (autres indices que
        // sur ce device) restaient sans effet.
        // Exception : un périphérique avec un override connu (ci-dessus) est
        // accepté même sans mapping standard — c'est justement ce que
        // l'override sert à corriger.
        if (!override && gp.mapping !== "standard") continue;
      } else if (gp.id !== mode) {
        // Réglage manuel (Réglages > Général) : seule la manette choisie par
        // l'utilisateur pilote la navigation, même si elle n'a pas le mapping
        // "standard" — l'utilisateur a vérifié lui-même que ça marche pour
        // son périphérique (ou compte sur un override, voir ci-dessus).
        continue;
      }
      const cur = override ? readOverrideButtons(gp, override) : readButtons(gp);
      const last = lastByGamepad.get(gp.index) ?? NONE;

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
        if (entered && active !== entered) entered = null;
        if (needsEntry(active)) {
          if (active === entered) {
            // Entré : gauche/droite change la valeur. Un nouvel appui sur
            // confirm en ressort (bascule), sans agir sur la valeur — même
            // geste que pour y entrer.
            if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) stepAdjustable(active, -1);
            if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) stepAdjustable(active, 1);
            if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
            if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
            if (cur.confirm && !last.confirm) entered = null;
          } else {
            // Pas encore entré : un champ ordinaire parmi d'autres, gauche/
            // droite déplace le focus. Confirm y entre.
            if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
            if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
            if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) moveFocus("left");
            if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) moveFocus("right");
            if (cur.confirm && !last.confirm) entered = active;
          }
        } else if (isAdjustable(active)) {
          // Gauche/droite règle la valeur du champ ciblé plutôt que de
          // changer le focus ; haut/bas continue de naviguer normalement.
          if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) stepAdjustable(active, -1);
          if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) stepAdjustable(active, 1);
          if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
          if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
          if (cur.confirm && !last.confirm) active.click();
        } else {
          if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
          if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
          if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) moveFocus("left");
          if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) moveFocus("right");
          if (cur.confirm && !last.confirm) (document.activeElement as HTMLElement | null)?.click();
        }
      }
      lastByGamepad.set(gp.index, cur);
    }
    raf = requestAnimationFrame(poll);
  }

  raf = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(raf);
}
