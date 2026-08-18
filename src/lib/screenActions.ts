// Actions qu'un écran monté expose au reste de l'application (§7.4bis).
//
// La navigation manette et les raccourcis clavier vivent AU-DESSUS des écrans
// (`gamepadNav.ts`, un scrutin global), mais ce qu'ils déclenchent appartient
// à l'écran affiché : « onglet suivant » n'a de sens que pour celui qui
// possède ses onglets, « mod suivant » que pour la bibliothèque, seule à
// connaître son tri courant et ses filtres.
//
// D'où ce registre plutôt qu'un champ de plus dans `nav` : l'écran s'inscrit à
// son montage et se retire à son démontage, et l'appelant n'a jamais à savoir
// lequel est ouvert. Une **pile**, pas une variable unique — la fiche pleine
// page se monte par-dessus la bibliothèque, et c'est la plus récente qui doit
// répondre.

/** Avance d'un cran dans un sens ou dans l'autre. */
type Cycler = (delta: 1 | -1) => void;

function makeRegistry() {
  const stack: Cycler[] = [];
  return {
    register(fn: Cycler): () => void {
      stack.push(fn);
      return () => {
        const i = stack.indexOf(fn);
        if (i >= 0) stack.splice(i, 1);
      };
    },
    /** `false` = personne pour répondre (aucun écran concerné monté) — laisse
     * l'appelant se rabattre sur autre chose plutôt que d'avaler la touche. */
    run(delta: 1 | -1): boolean {
      const fn = stack[stack.length - 1];
      if (!fn) return false;
      fn(delta);
      return true;
    },
    active(): boolean {
      return stack.length > 0;
    },
  };
}

const tabs = makeRegistry();
const mods = makeRegistry();

/** Posé par `Tabs.svelte` : tout écran à onglets est parcourable de l'extérieur
 * sans le savoir. */
export const registerTabStrip = tabs.register;
export const cycleTab = tabs.run;

/** Posé par la bibliothèque tant qu'une fiche pleine page est ouverte : mod
 * précédent/suivant dans l'ordre affiché (tri et filtres courants). */
export const registerModNav = mods.register;
export const navigateMod = mods.run;
export const modNavActive = mods.active;
