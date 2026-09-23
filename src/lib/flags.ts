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

/** English region name → ISO 3166-1 alpha-2 code, built once from the
 * runtime's own table rather than shipped: 676 two-letter probes, of which the
 * ~250 real regions answer. */
let byEnglishName: Map<string, string> | null = null;

function regionCodes(): Map<string, string> {
  if (byEnglishName) return byEnglishName;
  byEnglishName = new Map();
  const en = new Intl.DisplayNames(["en"], { type: "region", fallback: "none" });
  const A = "A".charCodeAt(0);
  for (let i = 0; i < 26; i++) {
    for (let j = 0; j < 26; j++) {
      const code = String.fromCharCode(A + i, A + j);
      let name: string | undefined;
      try {
        name = en.of(code);
      } catch {
        name = undefined;
      }
      if (name) byEnglishName.set(name.toLowerCase(), code);
    }
  }
  return byEnglishName;
}

/**
 * A country name as the user reads it in `locale` (INDEX§11) —
 * `Japan` becomes `Japon`.
 *
 * Translated **from the code, never from the mod's string** (TAXO§12):
 * the stored value is the game's English name, already normalised, and the
 * runtime knows every region by its code. A name it cannot match exactly
 * (Scotland, which AC flags on its own, or a spelling the runtime does not
 * share) keeps its English form — an approximate match would put a wrong name
 * on a tile, which is worse than an untranslated one.
 */
export function localizedCountry(name: string, locale: string): string {
  const code = regionCodes().get(name.trim().toLowerCase());
  if (!code) return name;
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(code) ?? name;
  } catch {
    return name;
  }
}
