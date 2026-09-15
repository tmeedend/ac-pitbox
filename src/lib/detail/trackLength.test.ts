import { describe, expect, it } from "vitest";
import { trackLength } from "./trackLength";

// Les quatre formes rencontrées sur les 70 `ui_track.json` de l'installation
// de référence, plus les absences. Chaque cas nomme le circuit qui l'a fourni.
describe("trackLength", () => {
  it("reads a bare integer as metres", () => {
    // Spa, le cas courant : 61 fichiers sur 70.
    expect(trackLength("7004")).toBe((7004).toLocaleString() + " m");
    expect(trackLength("20832")).toBe((20832).toLocaleString() + " m");
  });

  it("reads a small decimal as kilometres", () => {
    // Laguna Seca, seul de son espèce — et la raison d'être de ce module :
    // « 3.602 m » serait faux de trois ordres de grandeur.
    expect(trackLength("3.602")).toBe((3602).toLocaleString() + " m");
  });

  it("leaves a value that already carries its unit alone", () => {
    // Sept fichiers : six `165km`, un `190km`, un `4456 m`.
    expect(trackLength("165km")).toBe("165km");
    expect(trackLength("4456 m")).toBe("4456 m");
  });

  it("keeps short real lengths in metres", () => {
    // Aires de drift et de drag : les plus courts tracés réels, et ils sont
    // tous au-dessus du seuil. Les lire en kilomètres ferait un drag de 200 km.
    expect(trackLength("200")).toBe((200).toLocaleString() + " m");
    expect(trackLength("500")).toBe((500).toLocaleString() + " m");
  });

  it("returns nothing when there is nothing to say", () => {
    expect(trackLength(null)).toBeNull();
    expect(trackLength("")).toBeNull();
    expect(trackLength("   ")).toBeNull();
  });

  it("gives back what it cannot read rather than inventing", () => {
    expect(trackLength("0")).toBe("0");
    expect(trackLength("-")).toBe("-");
  });
});
