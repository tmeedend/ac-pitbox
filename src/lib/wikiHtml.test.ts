import { describe, expect, it } from "vitest";
import { fileNameFromSrc, isIconSized, resolveHref } from "./wikiHtml";

// Seules les deux fonctions **pures** sont testées ici : la reconstruction de
// l'arbre demande un DOM, et le projet a écarté jsdom explicitement (voir la
// section Tests de CLAUDE.md). Ce sont aussi les deux qui décident quelque
// chose — quelle image a le droit d'être affichée, et quelle URL a le droit
// d'être suivie.

describe("fileNameFromSrc", () => {
  // Règle (§9) : le nom de fichier est la clé du crédit. Sans lui, l'image est
  // refusée — mieux vaut une image manquante qu'une image non créditée.
  it("retrouve le fichier derrière une URL de vignette", () => {
    expect(
      fileNameFromSrc(
        "https://upload.wikimedia.org/wikipedia/commons/thumb/9/95/Mazda_Roadster_(MX-5).jpg/640px-Mazda_Roadster_(MX-5).jpg",
      ),
    ).toBe("Mazda Roadster (MX-5).jpg");
  });

  it("retrouve le fichier sans vignettage", () => {
    expect(fileNameFromSrc("//upload.wikimedia.org/wikipedia/commons/9/95/Nürburgring.svg")).toBe("Nürburgring.svg");
  });

  it("décode le pourcentage et rend les espaces", () => {
    expect(
      fileNameFromSrc("https://upload.wikimedia.org/wikipedia/commons/thumb/1/12/N%C3%BCrburgring_2019.jpg/640px-x.jpg"),
    ).toBe("Nürburgring 2019.jpg");
  });

  // Ce test a trouvé un vrai défaut : sans garde sur l'hôte, une `data:` URL
  // rendait « png;base64,AAAA ». Inoffensif — ce nom ne figure dans aucune
  // liste blanche — mais une fonction qui répond n'importe quoi finit par être
  // crue par un appelant futur.
  it("rend null sur tout ce qui ne vient pas de Wikimedia", () => {
    expect(fileNameFromSrc("")).toBeNull();
    expect(fileNameFromSrc("data:image/png;base64,AAAA")).toBeNull();
    expect(fileNameFromSrc("https://evil.example/wikipedia/commons/thumb/9/95/X.jpg/640px-X.jpg")).toBeNull();
    expect(fileNameFromSrc("/wiki/File:X.jpg")).toBeNull();
  });
});

describe("resolveHref", () => {
  // Règle : un lien d'article est suivable, tout le reste ne l'est pas. C'est
  // la seule barrière entre le contenu d'un wiki éditable par n'importe qui et
  // ce que l'application accepte d'ouvrir.
  it("rend absolus les liens internes du wiki", () => {
    expect(resolveHref("/wiki/Mazda_MX-5", "fr")).toBe("https://fr.wikipedia.org/wiki/Mazda_MX-5");
    expect(resolveHref("//commons.wikimedia.org/wiki/X", "fr")).toBe("https://commons.wikimedia.org/wiki/X");
    expect(resolveHref("https://example.org/a", "fr")).toBe("https://example.org/a");
  });

  it("refuse tout ce qui n'est pas http", () => {
    expect(resolveHref("javascript:alert(1)", "fr")).toBeNull();
    expect(resolveHref("data:text/html,<script>", "fr")).toBeNull();
    expect(resolveHref("file:///C:/Windows", "fr")).toBeNull();
    expect(resolveHref("#cite_note-1", "fr")).toBeNull();
  });
});

describe("isIconSized", () => {
  // Regle (§7.3) : c'est la taille demandee par la page qui decide, pas le
  // fichier. Les cinq etoiles d'une note ANCAP font vingt pixels et forment un
  // score ; servies en vignette de 640 px elles devenaient cinq affiches
  // empilees, chacune avec sa ligne de credit — vu sur l'article `Audi TT`.
  it("reconnait une icone posee dans le fil du texte", () => {
    expect(isIconSized("20", "20")).toBe(true);
    expect(isIconSized("64", "12")).toBe(true);
  });

  it("laisse une illustration en contenu", () => {
    expect(isIconSized("250", "180")).toBe(false);
    expect(isIconSized("300", null)).toBe(false);
    // Etroite mais haute : une image, pas une puce.
    expect(isIconSized("40", "400")).toBe(false);
  });

  it("tranche pour le contenu quand la taille est absente", () => {
    expect(isIconSized(null, null)).toBe(false);
    expect(isIconSized("", "")).toBe(false);
    expect(isIconSized("abc", "20")).toBe(false);
  });
});
