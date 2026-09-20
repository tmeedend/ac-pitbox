// Quel nom de la table du jeu correspond au pays d'un mod.
//
// La partie **pure** de `$lib/flags` : aucune E/S, aucun cache, donc testable.
//
// **La normalisation n'est pas ici, et c'est délibéré.** Les orthographes
// libres des auteurs (`U.S.A.`, `Great Britain`, une valeur cassée par un
// guillemet) sont corrigées **à l'écriture**, dans `harmonize::store` côté
// Rust, avec une table que l'utilisateur édite dans l'Atelier (§5). Ce qui
// arrive jusqu'ici est donc déjà canonique — deux tables d'alias, une en Rust
// éditable et une en dur ici, auraient fini par ne plus dire la même chose, et
// c'est la seconde qui l'aurait emporté en silence alors que l'utilisateur
// avait modifié la première.

/**
 * La clé de la table du jeu pour ce pays, ou `null` s'il n'y est pas.
 *
 * Insensible à la casse et aux espaces de bord : la valeur vient d'un mod, et
 * une normalisation faite ailleurs ne dispense pas d'une comparaison prudente
 * — une bibliothèque peut porter des valeurs rangées avant que la table
 * d'alias n'existe, tant qu'elle n'a pas été réharmonisée.
 */
export function countryKey(name: string, isKnown: (key: string) => boolean): string | null {
  const key = name.trim().toLowerCase();
  return isKnown(key) ? key : null;
}
