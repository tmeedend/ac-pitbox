// Le drapeau d'un pays, par son nom.
//
// La table est celle du jeu (`nationalities.rs` : 221 noms anglais entiers,
// chacun avec son PNG dans `content/gui/NationFlags/`).
//
// **Pourquoi un module et pas la prop du plateau.** La colonne Nationalité du
// plateau (SESSION§4.1) tient déjà la liste entière, parce qu'elle l'OFFRE :
// son menu propose les 221 entrées. Le filtre Pays de la bibliothèque (§6.3),
// lui, n'a besoin que de la correspondance — ses valeurs viennent des mods,
// pas de la table —, et la faire descendre en prop depuis l'écran
// bibliothèque jusqu'à la puce et à son popover traverserait trois composants
// pour une ligne de lecture. D'où ce cache, chargé à la demande.
//
// **Le nom est la clé, et il n'est pas normalisé chez les auteurs de mods.**
// Le jeu, lui, stocke le nom entier — c'est ce que dit `ui_skin.json` comme un
// preset de grille CM — mais un `ui_car.json` de mod écrit ce qu'il veut :
// « Italy », « italy », « ITALY ». La comparaison ignore donc la casse et les
// espaces de bord. Un nom qu'AC ne connaît pas (« Deutschland », « UK ») ne
// rend simplement pas de drapeau : la ligne garde son texte et ne perd rien.
// C'est la bonne issue — inventer une correspondance approchée poserait un
// drapeau faux, ce qui est pire que pas de drapeau du tout.
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

/** L'URL du drapeau de ce pays, ou `null` — table pas encore chargée, nom vide,
 * ou nom qu'Assetto Corsa ne connaît pas. Lecture réactive : la ligne se peint
 * dès que la table tombe. */
export function flagFor(name: string | null | undefined): string | null {
  if (!name) return null;
  return previewSrc(table.byName.get(name.trim().toLowerCase()) ?? null);
}
