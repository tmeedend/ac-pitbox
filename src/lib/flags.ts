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

/** Regions whose long name carries an administrative status nobody says out
 * loud: "R.A.S. chinoise de Hong Kong" on a tile of 124 px. Their short form
 * is the plain name - and only theirs: the short form of the others
 * abbreviates ("R.-U.", "É.-U."). */
const SHORT_FORM = new Set(["HK", "MO"]);

/** What the game table says about a country, as far as naming goes. */
export interface CountryEntry {
  /** ISO 3166-1 alpha-3, the game's own code. */
  code: string;
  /** ISO 3166-1 alpha-2, `null` for the British nations. */
  iso2: string | null;
}

/**
 * A country name as the user reads it in `locale` (INDEX§11, TAXO§12) —
 * `Japan` becomes `Japon`.
 *
 * **Translated from the code, never from the string.** The code comes from the
 * game table (`nationalities.rs`, alpha-3 → alpha-2). An earlier version
 * matched the English name against the runtime's region names instead: 33 of
 * the 221 names of the game did not match (`Czech Republic`, `Russian
 * Federation`, `Turkey`…), and the reverse lookup handed out retired codes for
 * others. The four British nations have no ISO code and are translated by
 * key (`byKey`); a value unknown to the game keeps its own spelling — a
 * guessed translation would be a wrong one.
 */
export function countryDisplayName(
  name: string,
  entry: CountryEntry | undefined,
  locale: string,
  byKey: (code3: string) => string | null,
): string {
  if (!entry) return name;
  if (!entry.iso2) return byKey(entry.code) ?? name;
  try {
    const style = SHORT_FORM.has(entry.iso2) ? "short" : "long";
    return new Intl.DisplayNames([locale], { type: "region", style }).of(entry.iso2) ?? name;
  } catch {
    return name;
  }
}
