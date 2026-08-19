// Navigation manette dans toute l'application : la croix directionnelle (ou
// le stick gauche) déplace le focus clavier parmi les éléments interactifs
// visibles (cartes de la bibliothèque, boutons du menu latéral…), le bouton
// principal (A sur Xbox, Croix/X sur PlayStation — button[0] dans le mapping
// "standard" du navigateur, donc le même code marche pour la plupart des
// manettes/volants reconnus) valide l'élément ciblé, et le bouton secondaire
// (B/Rond — button[1]) ferme la fiche pleine page d'un mod si elle est ouverte.
//
// QUI pilote l'interface ne se décide pas ici : un périphérique n'agit que si
// l'utilisateur l'a désigné, une fois, explicitement (`gamepadDevices.svelte.ts`,
// §7.4). Ce module ne fait que résoudre le profil du périphérique adopté
// (calibré → livré → layout standard → rien) et exiger son retour au neutre
// avant le premier événement.
//
// Approche générique par géométrie (plus proche voisin dans la direction
// visée), plutôt que du câblage par écran : marche pour n'importe quelle vue
// sans code spécifique, y compris pour passer de la grille au menu latéral en
// allant à gauche. Limite connue : ne « piège » pas le focus dans une modale
// ouverte par-dessus (BulkImport, sélection d'adversaire…) — hors périmètre
// demandé pour l'instant.

import { nav } from "$lib/nav.svelte";
import { deviceRecords, gamepadEnabled } from "$lib/gamepadDevices.svelte";
import { cycleTab, navigateMod } from "$lib/screenActions";
import {
  EMPTY_REST,
  bindingActive,
  deviceKey,
  hasRest,
  measureRest,
  sampleEqual,
  scrollAmount,
  type Action,
  type Binding,
  type DeviceRecord,
  type Direction,
  type NavProfile,
  type RestSnapshot,
} from "$lib/gamepadProfile";

type Dir = Direction;

// Inclut les champs de formulaire (curseurs, nombres, listes déroulantes) —
// sans ça, les réglages de session (carburant, dégâts, ghost car…) sont
// invisibles à la navigation manette : ni atteignables, ni modifiables.
const FOCUSABLE =
  'button:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled]), a[href], ' +
  'input:not([type="hidden"]):not([disabled]), select:not([disabled])';

/** Focusable au clavier mais **pas** une étape de navigation manette. Posé sur
 * les poignées de redimensionnement de colonnes : le motif WAI-ARIA impose de
 * les rendre focusables, et le curseur manette tombait donc sur un trait de
 * quelques pixels entre deux entêtes — le repère jaune n'y ressemblait plus
 * qu'à une bordure, et l'entête lui-même restait inatteignable. */
export const GP_SKIP_ATTR = "data-gp-skip";

function focusableElements(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null && !el.hasAttribute(GP_SKIP_ATTR),
  );
}

// Champs qui ont besoin d'un geste d'« entrée » avant que gauche/droite
// change leur valeur : listes déroulantes (`<select>`, popup native que la
// manette ne peut pas piloter), champs numériques (`type="number"`, ex. année
// min/max de la bibliothèque) et curseurs (`type="range"`). Sans ce geste,
// gauche/droite au simple survol changeait la valeur au lieu de déplacer le
// focus — deux bugs réels signalés : les filtres de bibliothèque changeaient
// en passant dessus, et le champ année ne pouvait plus être quitté par
// gauche/droite (piégé, valeur qui grimpe sans fin).
//
// Les curseurs y ont rejoint les deux autres après le même signalement : trois
// curseurs alignés sur une ligne (dégâts/carburant/pneus) sont un cul-de-sac
// si gauche/droite règle au lieu de déplacer — on ne peut atteindre ni le
// voisin, ni rien à droite du dernier. Une même règle pour tout ce qui porte
// une valeur vaut mieux qu'une exception par type de champ : confirm entre,
// annuler (ou confirm à nouveau) sort. Voir `entered` dans `startGamepadNav`.
function needsEntry(el: Element | null): el is HTMLSelectElement | HTMLInputElement {
  if (!el) return false;
  if (el instanceof HTMLSelectElement) return true;
  return el instanceof HTMLInputElement && (el.type === "number" || el.type === "range");
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
/** Champ « entré » : gauche/droite y règle la valeur au lieu de déplacer le
 * curseur. Sans repère distinct, rien à l'écran ne dit pourquoi la croix ne
 * déplace plus rien — c'est le même geste qui a deux effets selon l'état. */
const GP_EDIT_CLASS = "gp-editing";
let gpFocusEl: HTMLElement | null = null;

function setGamepadFocus(el: HTMLElement) {
  gpFocusEl?.classList.remove(GP_CLASS);
  gpFocusEl = el;
  lastConfirmed = null;
  el.classList.add(GP_CLASS);
  rememberInRegion(el);
  el.focus();
}

/**
 * Valide l'élément ciblé. Une **deuxième validation d'affilée sur le même
 * élément vaut double-clic** (§7.4bis) : c'est exactement la convention de la
 * souris — cliquer sélectionne, double-cliquer ouvre — donc une carte de
 * bibliothèque s'ouvre en fiche pleine page d'un second appui, sans avoir à
 * aller chercher le bouton « Agrandir ». Gratuit partout où un `ondblclick`
 * existe déjà (cartes et lignes de bibliothèque, slots de session).
 *
 * Le clic part **dans tous les cas**, y compris à la deuxième pression : sans
 * lui, un bouton qui n'écoute que `click` (une flèche d'ordre, un « + ») ne
 * répondrait qu'un appui sur deux. Le double-clic s'ajoute, il ne remplace pas.
 *
 * `bubbles` est indispensable : Svelte 5 délègue `dblclick` à la racine, un
 * événement qui ne remonte pas n'atteint aucun gestionnaire.
 */
let lastConfirmed: HTMLElement | null = null;

function activate(el: HTMLElement) {
  const repeat = lastConfirmed === el;
  el.click();
  if (repeat) el.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  lastConfirmed = el;
}

/** Pose le curseur manette sur un élément précis, depuis un écran qui sait
 * mieux que la géométrie où l'utilisateur doit commencer (§7.4bis : la fiche
 * détail le pose sur le skin sélectionné à l'ouverture). Passe par le même
 * chemin que le scrutin — donc le même repère visuel, et le même effacement
 * automatique dès qu'une souris reprend la main. */
export function focusGamepadElement(el: HTMLElement | null | undefined): void {
  if (el) setGamepadFocus(el);
}

/** Vrai quand un périphérique adopté est effectivement en train de piloter
 * l'interface. Un écran ne déplace le curseur d'autorité que dans ce cas :
 * sans manette, poser le focus par surprise fait sauter le défilement et vole
 * le curseur du clavier. */
let driving = false;
export function isGamepadDriving(): boolean {
  return driving;
}

// Toute prise de focus qui ne vient pas de nous (clic souris, Tab clavier…)
// efface le repère — sinon il resterait collé sur le dernier élément visé
// à la manette même après une interaction souris.
function initFocusTracking() {
  document.addEventListener("focusin", (e) => {
    if (e.target !== gpFocusEl) {
      gpFocusEl?.classList.remove(GP_CLASS);
      gpFocusEl = null;
      // Le compteur de validations suit le curseur : revenir sur une carte
      // déjà validée plus tôt ne doit pas l'ouvrir dès le premier appui.
      lastConfirmed = null;
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

/** Déplace le curseur d'un cran. Exportée pour le clavier : les flèches font
 * exactement ce que fait la croix directionnelle (§7.4bis) — c'est une même
 * intention exprimée sur deux périphériques, pas deux comportements. */
export function moveFocus(dir: Dir) {
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

// --- Zones (§7.4bis) ------------------------------------------------------
//
// Les gâchettes hautes changent d'onglet quand l'écran affiché en a
// (`cycleTab`) ; sinon elles changent de ZONE. C'est ce qui manquait à la
// bibliothèque, qui n'a pas d'onglets : rien n'y permettait de passer du menu
// latéral aux filtres, ni des filtres à la fiche de droite, autrement qu'en
// traversant toute la liste à la croix directionnelle.
//
// Marquées par un attribut plutôt que par leur classe, pour la même raison que
// le bouton de lancement : une classe est du style, elle se renomme sans qu'on
// pense à ce fichier.
export const REGION_ATTR = "data-gp-region";

/** Les zones les plus internes seulement. La zone de contenu d'`AppShell` en
 * est une pour les écrans d'un seul tenant (Lancement, Réglages) ; dans la
 * bibliothèque, elle en contient deux (liste et fiche), et c'est ce découpage-
 * là qui compte — garder les deux niveaux ferait toujours répondre le plus
 * extérieur. */
function regions(): HTMLElement[] {
  const all = Array.from(document.querySelectorAll<HTMLElement>(`[${REGION_ATTR}]`)).filter(
    (el) => el.offsetParent !== null,
  );
  return all.filter((z) => !all.some((other) => other !== z && z.contains(other)));
}

// Dernier élément visé dans chaque zone : revenir sur la liste des mods doit
// rendre le curseur là où on l'avait laissé, pas en haut de la liste — sans
// quoi l'aller-retour vers la fiche de droite coûte le défilement.
const lastInRegion = new WeakMap<Element, HTMLElement>();

function rememberInRegion(el: HTMLElement) {
  const zone = el.closest(`[${REGION_ATTR}]`);
  if (zone) lastInRegion.set(zone, el);
}

/** Zone suivante/précédente, curseur posé là où on l'avait laissé. */
function cycleRegion(delta: 1 | -1): void {
  const zones = regions();
  if (zones.length < 2) return;
  const current = document.activeElement as HTMLElement | null;
  const from = current ? zones.findIndex((z) => z.contains(current)) : -1;
  // Curseur hors de toute zone (ou nulle part) : `-1 + 1 = 0` amène sur la
  // première, ce qui est exactement ce qu'on veut d'un premier appui.
  const next = zones[(from + delta + zones.length) % zones.length];
  const remembered = lastInRegion.get(next);
  const target =
    remembered && next.contains(remembered) && remembered.offsetParent !== null
      ? remembered
      : focusableElements().find((el) => next.contains(el));
  if (target) setGamepadFocus(target);
}

/** Bouton « Démarrer la session » de la barre latérale (§7.4bis) : le bouton
 * Start y **amène le curseur**, il ne lance pas. Lancer d'une pression depuis
 * n'importe quel écran, sans avoir vu ce qu'on lance, serait le contraire d'un
 * raccourci utile. La barre latérale est toujours montée, donc la cible existe
 * quel que soit l'écran ouvert.
 *
 * Repéré par un attribut dédié plutôt que par sa classe : un nom de classe est
 * du style, il se renomme sans qu'on pense à ce fichier. */
export const LAUNCH_BUTTON_ATTR = "data-gp-launch";

function focusLaunchButton() {
  const el = document.querySelector<HTMLElement>(`[${LAUNCH_BUTTON_ATTR}]`);
  if (el) setGamepadFocus(el);
}

type ButtonEdges = Record<Dir | "confirm" | "back" | Action, boolean>;

const AXIS_THRESHOLD = 0.6;

// Layout standard (Xbox) : croix sur les boutons 12-15, stick gauche sur les
// axes 0/1, A/B sur 0/1. Volontairement lu tel quel plutôt que traduit en
// `NavProfile` : une direction y a DEUX sources (croix et stick), qu'un
// `Binding` unique par direction ne sait pas représenter — traduire ferait
// perdre le stick sur toutes les manettes normales, régression pour gagner
// une uniformité que personne ne voit.
//
// Raccourcis (§7.4bis) placés là où les interfaces de console les mettent :
// les **gâchettes hautes** (LB/RB, 4/5) changent d'onglet, les **basses**
// (LT/RT, 6/7) changent de mod, Start (9) saute au bouton de lancement. Les
// deux paires sont voisines et ne font pas la même chose : les mettre dans
// cet ordre-là (onglets au-dessus, contenu en-dessous) est ce qui rend le
// couple mémorisable.
function readButtons(gp: Gamepad): ButtonEdges {
  const ax = gp.axes[0] ?? 0;
  const ay = gp.axes[1] ?? 0;
  const btn = (i: number) => gp.buttons[i]?.pressed ?? false;
  return {
    up: btn(12) || ay < -AXIS_THRESHOLD,
    down: btn(13) || ay > AXIS_THRESHOLD,
    left: btn(14) || ax < -AXIS_THRESHOLD,
    right: btn(15) || ax > AXIS_THRESHOLD,
    confirm: btn(0),
    back: btn(1),
    tabPrev: btn(4),
    tabNext: btn(5),
    modPrev: btn(6),
    modNext: btn(7),
    start: btn(9),
  };
}

// Profils livrés, dans le format EXACT que produit la calibration guidée
// (`NavProfile`) : un profil reçu d'un utilisateur doit pouvoir être collé ici
// tel quel, sinon chaque contribution demande une traduction manuelle — donc
// une occasion de se tromper. Chaque volant a un layout qui lui est propre,
// sans norme : pas de généralisation tentée, une entrée par modèle constaté.
interface DeviceOverride {
  // Sous-chaîne de `Gamepad.id` : un même modèle peut s'annoncer sur deux
  // entrées `Gamepad` distinctes au même `id` (base + interface boutons
  // séparée, cas réel constaté sur une base Fanatec ClubSport V2.5) — matcher
  // les deux plutôt que de dépendre d'un `index` non garanti stable d'un
  // redémarrage à l'autre.
  idIncludes: string;
  profile: NavProfile;
}

const DEVICE_OVERRIDES: DeviceOverride[] = [
  {
    idIncludes: "FANATEC ClubSport Wheel Base V2.5",
    profile: {
      // Croix rapportée comme un seul axe à valeurs discrètes (hat switch
      // matériel) plutôt que 4 boutons ou un vrai stick 2D — valeurs lues
      // empiriquement, propres à ce modèle/pilote. `hint` n'est qu'un point
      // de départ : l'index de l'axe n'est pas garanti stable, la
      // reconnaissance se fait par valeur (voir `bindingActive`).
      dirs: {
        up: { kind: "axis", hint: 9, mode: "equals", value: -1 },
        right: { kind: "axis", hint: 9, mode: "equals", value: -3 / 7 },
        down: { kind: "axis", hint: 9, mode: "equals", value: 1 / 7 },
        left: { kind: "axis", hint: 9, mode: "equals", value: 5 / 7 },
      },
      confirm: { kind: "button", index: 0 },
      back: { kind: "button", index: 1 },
      // Pas de repos livré : il dépend de la position du volant et des
      // pédales au moment du branchement, donc il est mesuré à l'exécution
      // (voir `armDevice`). C'est ce qui empêche une pédale au repos à -1 de
      // répondre à la place du hat dont « haut » vaut aussi -1.
      rest: EMPTY_REST,
    },
  },
];

function findDeviceOverride(gp: { id: string }): DeviceOverride | undefined {
  return DEVICE_OVERRIDES.find((o) => gp.id.includes(o.idIncludes));
}

/** D'où vient le profil qui pilote ce périphérique. Ordre de résolution
 * (§7.4) : calibré ici (gagne toujours) → livré → layout standard si le
 * périphérique se déclare `mapping === "standard"` → rien, il reste inerte. */
export type ProfileSource = "calibrated" | "override" | "standard" | "none";

export interface ResolvedProfile {
  source: ProfileSource;
  /** `null` pour le layout standard : il n'est pas exprimable en `NavProfile`
   * (voir `readButtons`). */
  profile: NavProfile | null;
}

export function resolveProfile(gp: { id: string; mapping: string }, rec: DeviceRecord | undefined): ResolvedProfile {
  if (rec?.profile) return { source: "calibrated", profile: rec.profile };
  const override = findDeviceOverride(gp);
  if (override) return { source: "override", profile: override.profile };
  if (gp.mapping === "standard") return { source: "standard", profile: null };
  return { source: "none", profile: null };
}

function readProfileButtons(gp: Gamepad, profile: NavProfile, rest: RestSnapshot): ButtonEdges {
  const on = (b: Binding | undefined) => !!b && bindingActive(gp, b, rest);
  const dir = (d: Direction) => on(profile.dirs[d]);
  const act = (a: Action) => on(profile.actions?.[a]);
  return {
    up: dir("up"),
    down: dir("down"),
    left: dir("left"),
    right: dir("right"),
    confirm: on(profile.confirm),
    back: on(profile.back),
    tabPrev: act("tabPrev"),
    tabNext: act("tabNext"),
    modPrev: act("modPrev"),
    modNext: act("modNext"),
    start: act("start"),
  };
}

function readEdges(gp: Gamepad, resolved: ResolvedProfile, rest: RestSnapshot): ButtonEdges {
  return resolved.profile ? readProfileButtons(gp, resolved.profile, rest) : readButtons(gp);
}

// --- Défilement analogique (§7.4bis) -------------------------------------
//
// La croix directionnelle parcourt les éléments un par un et emmène le
// défilement avec elle ; c'est juste pour choisir, beaucoup trop lent pour
// traverser une bibliothèque de plusieurs centaines de mods. Un axe dédié
// (stick droit d'une manette) fait défiler la liste sans déplacer le curseur,
// à la vitesse de la poussée.
//
// Layout standard : axes 2/3 = stick droit, l'axe 3 est sa verticale (positif
// vers le bas, comme `deltaY`). Un profil calibré, lui, porte son propre axe.
const STANDARD_SCROLL_AXIS = 3;

/** Vitesse maximale, en pixels par seconde, stick à fond. Calé sur « une
 * hauteur d'écran par demi-seconde » environ : assez pour traverser une
 * longue liste, pas au point de la rendre illisible en chemin. */
const SCROLL_SPEED_PX_S = 1800;

function readScroll(gp: Gamepad, resolved: ResolvedProfile, rest: RestSnapshot): number {
  const axis = resolved.profile
    ? resolved.profile.scroll
    : { index: STANDARD_SCROLL_AXIS, invert: false };
  return scrollAmount(gp, axis, rest);
}

/** Le conteneur qui défile réellement autour du curseur. Remonté depuis
 * l'élément ciblé plutôt que deviné par écran : la même touche doit défiler la
 * liste de la bibliothèque, le corps d'une fiche ou un panneau de réglages
 * sans que rien de tout cela ait à se déclarer. */
function scrollableAround(el: Element | null): Element | null {
  let node: Element | null = el;
  while (node && node !== document.body) {
    const style = getComputedStyle(node);
    const scrollable = /(auto|scroll)/.test(style.overflowY);
    if (scrollable && node.scrollHeight > node.clientHeight + 1) return node;
    node = node.parentElement;
  }
  return document.scrollingElement;
}

function applyScroll(amount: number, dtMs: number): void {
  if (!amount) return;
  const target = scrollableAround(document.activeElement);
  if (!target) return;
  // Réponse quadratique : le premier tiers de la course sert au réglage fin,
  // la fin de course à traverser la liste. Un rapport linéaire donne un
  // défilement soit trop nerveux au début, soit trop lent au bout.
  const curve = amount * Math.abs(amount);
  target.scrollTop += (curve * SCROLL_SPEED_PX_S * dtMs) / 1000;
}

const NONE: ButtonEdges = {
  up: false,
  down: false,
  left: false,
  right: false,
  confirm: false,
  back: false,
  tabPrev: false,
  tabNext: false,
  modPrev: false,
  modNext: false,
  start: false,
};

function anyEdge(e: ButtonEdges): boolean {
  return Object.values(e).some(Boolean);
}

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

// --- Armement : retour au neutre exigé (§7.4) ----------------------------
//
// Un périphérique ne produit son premier événement qu'après avoir été VU au
// repos — à l'adoption comme à chaque reconnexion. Sans ça, une pédale
// enfoncée au branchement (ou un volant tourné) vaut « bas » maintenu dès la
// première image, et le focus dérive tout seul sans que rien à l'écran ne
// l'explique. C'est le correctif de ce bug-là ; le consentement explicite
// (§7.4) répond à une autre question, les deux sont complémentaires.
//
// Le repos se MESURE, jamais ne se suppose : un hat DirectInput normalisé par
// Chromium repose *hors* de [-1, 1] (~3,2 constaté), les pédales à -1, un
// volant là où on l'a laissé. On attend donc que le périphérique cesse de
// bouger pendant `STABLE_MS`, on prend cet instantané comme référence (sauf
// profil calibré, qui porte le sien), et on n'arme que si rien de ce que le
// profil écoute n'est actif à ce moment-là.
const STABLE_MS = 500;

interface ArmState {
  sample: RestSnapshot | null;
  stableSince: number;
  /** Non nul = armé ; c'est aussi le repos de référence des lectures. */
  rest: RestSnapshot | null;
}

function armDevice(
  arms: Map<number, ArmState>,
  gp: Gamepad,
  resolved: ResolvedProfile,
  now: number,
): RestSnapshot | null {
  let a = arms.get(gp.index);
  if (!a) {
    a = { sample: null, stableSince: now, rest: null };
    arms.set(gp.index, a);
  }
  if (a.rest) return a.rest;
  const sample = measureRest(gp);
  if (!a.sample || !sampleEqual(a.sample, sample)) {
    a.sample = sample;
    a.stableSince = now;
    return null;
  }
  if (now - a.stableSince < STABLE_MS) return null;
  const rest = resolved.profile && hasRest(resolved.profile.rest) ? resolved.profile.rest : sample;
  // Immobile ne veut pas dire relâché : une pédale maintenue à fond est
  // parfaitement stable. Le profil, lui, sait la reconnaître — et tant qu'elle
  // l'est, le périphérique reste inerte plutôt que de faire défiler le focus.
  if (anyEdge(readEdges(gp, resolved, rest))) return null;
  // Même raison pour l'axe de défilement : un stick maintenu au branchement
  // ferait défiler la liste toute seule dès la première image.
  if (readScroll(gp, resolved, rest) !== 0) return null;
  a.rest = rest;
  return rest;
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
  const arms = new Map<number, ArmState>();
  // Le défilement analogique est une vitesse, donc il lui faut la durée de
  // l'image : à 60 Hz comme à 144, la liste doit défiler à la même allure.
  let lastFrame = 0;

  // Champ « entré » (confirm appuyé dessus une première fois) — liste
  // déroulante, champ numérique ou curseur, voir `needsEntry` : tant qu'il
  // n'est pas entré, gauche/droite déplace le focus normalement (un filtre est
  // un champ parmi d'autres) ; une fois entré, gauche/droite change sa valeur,
  // et annuler (ou confirm à nouveau) en ressort. Remise à zéro dès que le
  // focus quitte ce champ par un autre moyen (clic souris, focus posé ailleurs
  // par le code).
  let entered: HTMLSelectElement | HTMLInputElement | null = null;

  function setEntered(el: HTMLSelectElement | HTMLInputElement | null) {
    entered?.classList.remove(GP_EDIT_CLASS);
    entered = el;
    el?.classList.add(GP_EDIT_CLASS);
  }

  function poll() {
    // Coupe-circuit global (Réglages), et capture exclusive des entrées par le
    // panneau de configuration du périphérique : sa zone d'essai consomme
    // elle-même les entrées du périphérique calibré, sinon « haut » validerait
    // un bouton derrière le panneau. Tout est remis à zéro plutôt que gelé —
    // sans ça, le premier front lu au retour serait celui d'avant.
    if (!gamepadEnabled() || nav.inputCapture === "controller") {
      arms.clear();
      lastByGamepad.clear();
      driving = false;
      raf = requestAnimationFrame(poll);
      return;
    }
    const now = performance.now();
    // Bornée : revenir d'un onglet en arrière-plan (ou d'une longue pause de
    // rendu) ne doit pas faire sauter la liste d'un coup.
    const dt = lastFrame ? Math.min(50, now - lastFrame) : 0;
    lastFrame = now;
    const records = deviceRecords();
    const seen = new Set<number>();
    let anyDriving = false;
    for (const gp of navigator.getGamepads?.() ?? []) {
      if (!gp?.connected) continue;
      seen.add(gp.index);
      // Défaut fermé (§7.4) : un périphérique que l'utilisateur n'a pas
      // désigné ne pilote rien. `mapping === "standard"` est *déclaré* par le
      // périphérique, pas vérifié — un volant en « mode Xbox » ou derrière un
      // adaptateur XInput s'annonce standard, et le layout Xbox place « haut/
      // bas » sur l'axe 1, qui sur un volant est une pédale.
      const rec = records[deviceKey(gp.id)];
      if (!rec?.use) continue;
      const resolved = resolveProfile(gp, rec);
      if (resolved.source === "none") continue;
      const rest = armDevice(arms, gp, resolved, now);
      if (!rest) continue;
      const cur = readEdges(gp, resolved, rest);
      const last = lastByGamepad.get(gp.index) ?? NONE;
      // Au moins un périphérique adopté, armé et lu : c'est ce qui autorise un
      // écran à poser lui-même le curseur (voir `isGamepadDriving`).
      anyDriving = true;

      // Hors visionneuse : le défilement analogique tourne quel que soit
      // l'élément ciblé, y compris pendant l'édition d'un champ — il ne
      // déplace pas le curseur, il ne peut donc rien perturber.
      if (nav.inputCapture !== "lightbox") applyScroll(readScroll(gp, resolved, rest), dt);

      if (nav.inputCapture === "lightbox") {
        // Visionneuse plein écran ouverte par-dessus la fiche (§6.1,
        // Lightbox.svelte) : elle gère elle-même tout son input manette
        // (gauche/droite/B), y compris la fermeture — ne rien faire ici, sous
        // peine qu'un même B ferme à la fois la visionneuse et la fiche.
      } else {
        // Raccourcis (§7.4bis), lus AVANT la navigation et quel que soit
        // l'écran : ils ne dépendent ni de l'élément ciblé, ni du fait qu'une
        // fiche pleine page soit ouverte. Front montant seulement, jamais de
        // répétition au maintien — changer de mod recharge toute une fiche
        // (et reconvertit un modèle 3D), une rafale n'a rien d'un service.
        // Onglets d'abord, zones à défaut : `cycleTab` répond `false` quand
        // aucun écran à onglets n'est monté (bibliothèque), et c'est là que
        // passer d'une zone à l'autre est le seul chemin praticable.
        if (cur.tabPrev && !last.tabPrev && !cycleTab(-1)) cycleRegion(-1);
        if (cur.tabNext && !last.tabNext && !cycleTab(1)) cycleRegion(1);
        if (cur.modPrev && !last.modPrev) navigateMod(-1);
        if (cur.modNext && !last.modNext) navigateMod(1);
        if (cur.start && !last.start) focusLaunchButton();

        const active = document.activeElement;
        if (entered && active !== entered) setEntered(null);
        const backPressed = cur.back && !last.back;
        // Annuler sort d'abord du champ en cours d'édition, et ne fait que ça
        // cette image-là : sinon le même appui refermerait aussi la fiche
        // pleine page derrière, alors que l'utilisateur voulait juste rendre
        // la croix à la navigation.
        const leftField = backPressed && entered !== null;
        if (leftField) setEntered(null);

        // La fiche pleine page se navigue comme n'importe quel écran : la
        // croix directionnelle y déplace le curseur (elle changeait de mod
        // avant, ce qui rendait ses propres commandes — skins, onglets,
        // boutons — inatteignables à la manette). Mod précédent/suivant a
        // désormais ses deux boutons dédiés, ci-dessus.
        if (nav.openFull && backPressed && !leftField) nav.openFull = null;

        if (needsEntry(active)) {
          if (active === entered) {
            // Entré : gauche/droite change la valeur. Annuler en ressort (ou
            // un nouvel appui sur confirm, même geste que pour y entrer) sans
            // agir sur la valeur. Haut/bas continue de déplacer le curseur :
            // c'est la sortie de secours de qui a oublié quel bouton sort.
            if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) stepAdjustable(active, -1);
            if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) stepAdjustable(active, 1);
            if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
            if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
            if (cur.confirm && !last.confirm) setEntered(null);
          } else {
            // Pas encore entré : un champ ordinaire parmi d'autres, gauche/
            // droite déplace le focus. Confirm y entre.
            if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
            if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
            if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) moveFocus("left");
            if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) moveFocus("right");
            if (cur.confirm && !last.confirm) setEntered(active);
          }
        } else {
          if (shouldFire(dirRepeats, gp.index, "up", cur.up, now)) moveFocus("up");
          if (shouldFire(dirRepeats, gp.index, "down", cur.down, now)) moveFocus("down");
          if (shouldFire(dirRepeats, gp.index, "left", cur.left, now)) moveFocus("left");
          if (shouldFire(dirRepeats, gp.index, "right", cur.right, now)) moveFocus("right");
          // Seule branche à passer par `activate` : c'est celle des éléments
          // ordinaires (cartes, lignes, boutons). Les champs de saisie ont
          // leur propre sémantique de validation juste au-dessus, un
          // double-clic n'y voudrait rien dire.
          if (cur.confirm && !last.confirm) {
            const el = document.activeElement as HTMLElement | null;
            if (el) activate(el);
          }
        }
      }
      lastByGamepad.set(gp.index, cur);
    }
    driving = anyDriving;
    // Un slot libéré au débranchement est réattribué à un autre périphérique :
    // ne jamais lui laisser hériter de l'armement ni du dernier front de son
    // prédécesseur — c'est ce qui garantit qu'une reconnexion repasse par le
    // retour au neutre.
    for (const idx of arms.keys()) if (!seen.has(idx)) arms.delete(idx);
    for (const idx of lastByGamepad.keys()) if (!seen.has(idx)) lastByGamepad.delete(idx);
    raf = requestAnimationFrame(poll);
  }

  raf = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(raf);
}
