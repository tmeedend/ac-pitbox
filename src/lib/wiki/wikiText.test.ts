import { describe, expect, it } from "vitest";
import { parseExtract } from "./wikiText";

describe("parseExtract", () => {
  // Règle (WIKI§7.3) : les titres de section arrivent en balisage wiki dans le
  // texte brut de l'API — mesuré sur `Mazda MX-5` : `== Overview ==` et
  // `=== First generation – NA (1989–1997) ===`. Les afficher tels quels serait
  // du bruit, les perdre écraserait la structure.
  it("reconnaît les titres de section et leur niveau", () => {
    const blocks = parseExtract(
      ["Le préambule.", "", "== Overview ==", "", "Un paragraphe.", "=== First generation – NA (1989–1997) ===", "Autre."].join(
        "\n",
      ),
    );
    expect(blocks).toEqual([
      { kind: "paragraph", level: 0, text: "Le préambule." },
      { kind: "heading", level: 2, text: "Overview" },
      { kind: "paragraph", level: 0, text: "Un paragraphe." },
      { kind: "heading", level: 3, text: "First generation – NA (1989–1997)" },
      { kind: "paragraph", level: 0, text: "Autre." },
    ]);
  });

  // Règle (WIKI§2) : le texte n'est jamais réécrit. Seule la ligne de titre est
  // reconnue ; tout le reste ressort à l'identique, y compris un `=` qui
  // n'ouvre pas un titre.
  it("ne touche pas au texte", () => {
    const source = "Une équation : a == b, et rien d'autre.";
    expect(parseExtract(source)).toEqual([{ kind: "paragraph", level: 0, text: source }]);
  });

  it("ignore un titre vide plutôt que d'en faire un bloc", () => {
    expect(parseExtract("==  ==")).toEqual([{ kind: "paragraph", level: 0, text: "==  ==" }]);
  });

  it("rend un tableau vide pour un texte vide", () => {
    expect(parseExtract("")).toEqual([]);
    expect(parseExtract("   \n\n  ")).toEqual([]);
  });
});
