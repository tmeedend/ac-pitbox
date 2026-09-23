import { describe, expect, it } from "vitest";
import { countryKey, localizedCountry } from "./flags";

/** Un extrait fidèle de la table du jeu : les noms utiles aux cas ci-dessous,
 * dont les deux qui portent une virgule. */
const TABLE = new Set(["france", "italy", "united states", "scotland", "tanzania, {united republic of}"]);
const known = (key: string) => TABLE.has(key);

describe("countryKey", () => {
  it("reconnaît le nom du jeu, quelle que soit la casse et les espaces", () => {
    expect(countryKey("Italy", known)).toBe("italy");
    expect(countryKey("  ITALY ", known)).toBe("italy");
  });

  /** Le jeu leur donne leur propre drapeau, et la table d'alias côté Rust se
   * garde bien de les replier sur le Royaume-Uni. */
  it("connaît les nations britanniques que le jeu distingue", () => {
    expect(countryKey("Scotland", known)).toBe("scotland");
  });

  /** Les virgules de la table du jeu ne sont pas des séparateurs : la clé est
   * le nom entier. */
  it("garde un nom que le jeu écrit avec une virgule", () => {
    expect(countryKey("Tanzania, {United Republic of}", known)).toBe("tanzania, {united republic of}");
  });

  /**
   * **Ce que cette fonction ne fait PAS.** Les orthographes libres sont
   * corrigées à l'écriture, côté Rust (`rules::canonical_country`), avec une
   * table que l'utilisateur édite. En rajouter une ici les ferait diverger —
   * ce test dit donc qu'un nom non normalisé ne rend rien, et c'est voulu.
   */
  it("n'approche rien : un nom non normalisé ne rend pas de clé", () => {
    expect(countryKey("U.S.A.", known)).toBeNull();
    expect(countryKey("Freedonia", known)).toBeNull();
    expect(countryKey("", known)).toBeNull();
  });
});

describe("localizedCountry", () => {
  // Index spec §11: a country is translated from its code, never from the
  // mod's string.
  it("translates the game's English name through its code", () => {
    expect(localizedCountry("Japan", "fr")).toBe("Japon");
    expect(localizedCountry("United Kingdom", "de")).toBe("Vereinigtes Königreich");
    expect(localizedCountry("united states", "fr")).toBe("États-Unis");
  });

  // A wrong name on a tile is worse than an untranslated one.
  it("keeps the name as is when no code matches it exactly", () => {
    expect(localizedCountry("Scotland", "fr")).toBe("Scotland");
    expect(localizedCountry("Nürburgring Land", "fr")).toBe("Nürburgring Land");
  });
});
