// Signal générique « la bibliothèque a peut-être changé » : activation,
// désactivation, suppression, import. Les vues ouvertes (Library, AppShell,
// DetailPage) s'y abonnent pour se resynchroniser sans avoir à savoir QUI a
// changé quoi — même mécanisme que `importState.version` avant lui, mais qui
// ne couvrait que l'import.
//
// Bug réel corrigé par cette généralisation : désactiver un mod depuis sa
// fiche ne rafraîchissait l'avertissement « mod désactivé » du bloc SESSION
// (`AppShell.svelte`) qu'au prochain changement de sélection — l'effet qui le
// charge n'était abonné qu'à l'id du mod choisi, jamais à son état
// d'activation, qui peut changer sans que l'id bouge.
let value = $state(0);

export function libraryVersion(): number {
  return value;
}

export function bumpLibraryVersion(): void {
  value++;
}
