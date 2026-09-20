import { describe, expect, it } from "vitest";
import { countryKey } from "./flags";

/** Un extrait fidèle de la table du jeu : les noms utiles aux cas ci-dessous,
 * dont les deux qui portent une virgule. */
const TABLE = new Set([
  "france",
  "italy",
  "united states",
  "united kingdom",
  "scotland",
  "england",
  "netherlands",
  "germany",
  "tanzania, {united republic of}",
  "micronesia, {federated states of}",
]);
const known = (key: string) => TABLE.has(key);

describe("countryKey", () => {
  it("reconnaît le nom du jeu, quelle que soit la casse et les espaces", () => {
    expect(countryKey("Italy", known)).toBe("italy");
    expect(countryKey("  ITALY ", known)).toBe("italy");
  });

  /** Mesuré : 80 des 395 mods qui portent un pays s'écrivaient d'une façon
   * que la table du jeu ne connaît pas. */
  it("traduit les orthographes relevées sur une vraie bibliothèque", () => {
    expect(countryKey("U.S.A.", known)).toBe("united states");
    expect(countryKey("USA", known)).toBe("united states");
    expect(countryKey("United States of America", known)).toBe("united states");
    expect(countryKey("Great Britain", known)).toBe("united kingdom");
  });

  /** Le jeu leur donne leur propre drapeau : les renvoyer sur le Royaume-Uni
   * remplacerait un drapeau juste par un autre, et perdrait ce que l'auteur du
   * mod a pris la peine d'écrire. */
  it("laisse les nations britanniques à leur propre drapeau", () => {
    expect(countryKey("Scotland", known)).toBe("scotland");
    expect(countryKey("England", known)).toBe("england");
  });

  /** Bug réel, `le_lancone` : son `ui_track.json` porte
   * `"country": "France\", \"Corsica"`, l'auteur ayant voulu en écrire deux. */
  it("lit un nom avant le guillemet d'une valeur cassée par son auteur", () => {
    expect(countryKey('France", "Corsica', known)).toBe("france");
  });

  /**
   * **La règle est le guillemet, jamais la virgule** — et c'est tout l'objet de
   * ce test. Couper à la virgule paraîtrait faire la même chose et rendrait
   * introuvables les deux noms que la table du jeu écrit ainsi.
   */
  it("ne coupe jamais à la virgule, que la table du jeu utilise", () => {
    expect(countryKey("Tanzania, {United Republic of}", known)).toBe("tanzania, {united republic of}");
    expect(countryKey("Micronesia, {Federated States of}", known)).toBe("micronesia, {federated states of}");
  });

  it("rend null plutôt que d'approcher un pays qu'il ne connaît pas", () => {
    expect(countryKey("Corsica", known)).toBeNull();
    expect(countryKey("Freedonia", known)).toBeNull();
    expect(countryKey('"', known)).toBeNull();
    expect(countryKey("", known)).toBeNull();
  });

  /** Un alias ne vaut que si sa cible est dans la table : sur une install dont
   * la table serait plus courte, il ne doit pas rendre une clé absente. */
  it("n'invente pas une cible que la table n'a pas", () => {
    expect(countryKey("USA", (k) => k === "italy")).toBeNull();
  });
});
