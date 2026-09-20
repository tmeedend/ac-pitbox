// Le drapeau d'un pays, par son nom.
//
// La table est celle du jeu (`nationalities.rs` : 221 noms anglais entiers,
// chacun avec son PNG dans `content/gui/NationFlags/`). La règle qui décide
// quel nom de cette table correspond à ce qu'un mod a écrit est à côté, dans
// `flags.ts` — pure, et testée pour ça.
//
// **Pourquoi un module et pas la prop du plateau.** La colonne Nationalité du
// plateau (SESSION§3.2) tient déjà la liste entière, parce qu'elle l'OFFRE :
// son menu propose les 221 entrées. Le filtre Pays de la bibliothèque (§6.3),
// lui, n'a besoin que de la correspondance — ses valeurs viennent des mods,
// pas de la table —, et la faire descendre en prop depuis l'écran
// bibliothèque jusqu'à la puce et à son popover traverserait trois composants
// pour une ligne de lecture. D'où ce cache, chargé à la demande.
import { countryKey } from "$lib/flags";
import { invokeSafe } from "$lib/invokeSafe";
import type { Nationality } from "$lib/launch/launch";
import { previewSrc } from "$lib/library/library";

const table = $state<{ byName: Map<string, string | null> }>({ byName: new Map() });

/** Une seule lecture par session : la table du jeu ne bouge pas sous l'app. */
let loading: Promise<void> | null = null;

/**
 * Charge la table si ce n'est pas déjà fait. Idempotent, et sans effet visible
 * tant qu'elle n'est pas là — `flagFor` rend `null`, et la ligne s'affiche
 * sans drapeau plutôt que d'attendre.
 *
 * Appelé par l'écran qui va montrer des drapeaux, jamais au démarrage : lire
 * `ac.utils.js` et vérifier 221 fichiers ne se justifie que si quelqu'un les
 * regarde.
 */
export function loadFlags(): Promise<void> {
  loading ??= invokeSafe<Nationality[]>("nationalities", undefined, []).then((list) => {
    table.byName = new Map(list.map((n) => [n.name.toLowerCase(), n.flag]));
  });
  return loading;
}

/**
 * L'URL du drapeau de ce pays, ou `null` — table pas encore chargée, nom vide,
 * ou nom qu'Assetto Corsa ne connaît pas même par alias. Lecture réactive : la
 * ligne se peint dès que la table tombe.
 *
 * `null` reste une issue normale, et la bonne : inventer une correspondance
 * approchée poserait un drapeau faux, ce qui est pire que pas de drapeau.
 */
export function flagFor(name: string | null | undefined): string | null {
  if (!name) return null;
  const key = countryKey(name, (k) => table.byName.has(k));
  return previewSrc(key ? (table.byName.get(key) ?? null) : null);
}
