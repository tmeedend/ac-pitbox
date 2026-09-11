import { describe, expect, it } from "vitest";
import { splitProvenance } from "./provenance";

describe("splitProvenance", () => {
  // Le cas de tous les mods reconnus : l'archive entière était la livraison.
  it("leaves a plain archive name alone", () => {
    expect(splitProvenance("RSS_Formula_Supreme_25.rar")).toEqual({
      archive: "RSS_Formula_Supreme_25.rar",
      inside: null,
    });
  });

  // Le cas mesuré sur la bibliothèque réelle : le dossier `content/fonts` d'un
  // pack de neuf NSX, resté sur le carreau et nommé d'après son chemin.
  it("splits a leftover into its archive and its place inside it", () => {
    expect(splitProvenance(String.raw`SOME1_NSX.7z__SOME1_NSX\content\fonts`)).toEqual({
      archive: "SOME1_NSX.7z",
      inside: "SOME1_NSX/content/fonts",
    });
  });

  // Un simple `_` sépare des mots ; seul le double sépare l'archive du reste.
  it("does not cut on a single underscore", () => {
    expect(splitProvenance("Sound_by_Marti.zip")).toEqual({ archive: "Sound_by_Marti.zip", inside: null });
  });

  // Rien avant le séparateur : on ne fabrique pas un nom d'archive vide.
  it("keeps the whole string when nothing precedes the separator", () => {
    expect(splitProvenance("__content_fonts")).toEqual({ archive: "__content_fonts", inside: null });
  });

  it("ignores a trailing separator", () => {
    expect(splitProvenance("archive.rar__")).toEqual({ archive: "archive.rar", inside: null });
  });

  it("returns nothing for an absent or blank provenance", () => {
    expect(splitProvenance(null)).toBeNull();
    expect(splitProvenance("   ")).toBeNull();
  });
});
