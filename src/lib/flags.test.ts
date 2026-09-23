import { describe, expect, it } from "vitest";
import { countryDisplayName, countryKey } from "./flags";

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

describe("countryDisplayName", () => {
  const byKey = (code: string) => (code === "SCT" ? "Écosse" : null);

  // INDEX§11: translated from the code, never from the mod's string.
  it("translates through the game's code", () => {
    expect(countryDisplayName("Japan", { code: "JPN", iso2: "JP" }, "fr", byKey)).toBe("Japon");
    expect(countryDisplayName("United Kingdom", { code: "GBR", iso2: "GB" }, "de", byKey)).toBe("Vereinigtes Königreich");
    // The names the runtime spells differently are exactly why the code is used.
    expect(countryDisplayName("Czech Republic", { code: "CZE", iso2: "CZ" }, "fr", byKey)).toBe("Tchéquie");
    expect(countryDisplayName("Russian Federation", { code: "RUS", iso2: "RU" }, "fr", byKey)).toBe("Russie");
  });

  it("drops the administrative status of Hong Kong, and abbreviates nothing else", () => {
    expect(countryDisplayName("Hong Kong", { code: "HKG", iso2: "HK" }, "fr", byKey)).toBe("Hong Kong");
    expect(countryDisplayName("United States", { code: "USA", iso2: "US" }, "fr", byKey)).toBe("États-Unis");
  });

  it("translates the British nations by key, and keeps an unknown value as written", () => {
    expect(countryDisplayName("Scotland", { code: "SCT", iso2: null }, "fr", byKey)).toBe("Écosse");
    expect(countryDisplayName("Wales", { code: "WLS", iso2: null }, "fr", byKey)).toBe("Wales");
    expect(countryDisplayName("Freedonia", undefined, "fr", byKey)).toBe("Freedonia");
  });
});
