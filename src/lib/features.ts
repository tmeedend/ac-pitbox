// Interrupteurs de fonctionnalité — des constantes de source, pas des réglages.
//
// **La distinction est le point de ce fichier.** Un réglage appartient à
// l'utilisateur : il vit dans `ui_prefs.json`, il se change dans un écran, et
// l'app doit rester cohérente dans les deux positions. Un interrupteur de ce
// fichier appartient au projet : il décide si une fonctionnalité **existe** dans
// une version livrée, il se change en éditant cette ligne et en recompilant, et
// personne d'autre que nous ne le voit.
//
// D'où la règle : un interrupteur à `false` ne doit laisser **aucune trace à
// l'écran** — pas d'onglet vide, pas de case grisée, pas de question posée dans
// l'assistant. Une fonctionnalité désactivée qui laisse son bouton derrière elle
// n'est pas désactivée, elle est cassée.
//
// À n'utiliser que pour ce qui est **fini mais pas prêt à être vu**. Ce n'est ni
// un dossier de brouillons ni un remplaçant de branche : du code mort derrière
// un drapeau qu'on ne rallume jamais pourrit exactement comme ailleurs, en
// moins visible.

/**
 * **Vignettes régénérées de la grille** (`docs/SPEC-grille.md` §5 à §8, §11).
 *
 * Éteint le temps que les valeurs par défaut des trois presets embarqués soient
 * arrêtées. Le mécanisme est complet et fusionné dans `main` ; ce qui manque
 * est un travail de réglage à l'œil, long et fastidieux, qui ne peut pas se
 * faire à moitié : trois presets livrés avec des valeurs provisoires seraient
 * jugés sur elles.
 *
 * **Ce que `false` éteint**, et ce sont les trois seuls endroits qui exposaient
 * la fonctionnalité :
 *
 *  - l'onglet `Réglages › Vignettes` (`Settings.svelte`) ;
 *  - la deuxième page de l'assistant de première configuration, celle des trois
 *    profils (`SetupWizard.svelte`) — une nouvelle installation repart donc
 *    directement sur les défauts de l'ancien profil « Normal », qui était de
 *    toute façon le présélectionné ;
 *  - la tâche de fond de génération (`AppShell.svelte`).
 *
 * **Et surtout : `gridThumbsOn()` rend `false`**, ce qui suffit à ce qu'aucune
 * vignette ne soit demandée, produite ni affichée. Les cartes reprennent la
 * `preview.png` du mod, exactement comme avant le chantier — y compris chez
 * quelqu'un qui avait coché l'option, dont le réglage reste écrit sur disque et
 * redeviendra effectif au rallumage.
 *
 * **Rien n'est effacé en s'éteignant.** Les images déjà produites restent dans
 * `app_cache_dir/gridthumbs/`, les presets de l'utilisateur dans
 * `ui_prefs.json` : rallumer retrouve tout en l'état. C'est voulu — un
 * interrupteur qui détruit en passant à `false` n'est plus un interrupteur.
 */
export const FEATURE_GRID_THUMBS = false;
