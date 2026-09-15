// Les deux moitiés du modèle « centre ± écart » (SETUP§2.9) : la migration depuis
// deux bornes, et le bornage de ce qui en ressort.
import { describe, expect, it } from "vitest";
import { band, centerSpreadOf } from "./aiBand";

describe("centerSpreadOf", () => {
  it("retrouve le réglage par défaut d'avant", () => {
    expect(centerSpreadOf(92, 98)).toEqual({ center: 95, spread: 3 });
  });

  // Un écart demi-entier s'arrondit vers le HAUT : une fourchette d'un point
  // trop large est moins grave qu'une dispersion écrasée sans un mot.
  it("arrondit l'écart vers le haut sur un intervalle impair", () => {
    expect(centerSpreadOf(90, 95)).toEqual({ center: 93, spread: 3 });
  });

  it("accepte des bornes dans le désordre", () => {
    expect(centerSpreadOf(98, 92)).toEqual(centerSpreadOf(92, 98));
  });

  it("rend un écart nul sur une valeur unique", () => {
    expect(centerSpreadOf(88, 88)).toEqual({ center: 88, spread: 0 });
  });
});

describe("band", () => {
  it("encadre le centre", () => {
    expect(band(95, 3, 70, 100)).toEqual([92, 98]);
  });

  // Le libellé affiche la plage BORNÉE, jamais la plage théorique : un centre
  // de 3 avec ±5 donne (0–8), pas (−2–8). Sinon l'écran annonce ce que le jeu
  // ne recevra pas.
  it("borne plutôt que de déborder sa plage", () => {
    expect(band(3, 5, 0, 100)).toEqual([0, 8]);
    expect(band(72, 20, 70, 100)).toEqual([70, 92]);
    expect(band(98, 20, 70, 100)).toEqual([78, 100]);
  });

  it("rend deux fois le centre quand l'écart est nul", () => {
    expect(band(95, 0, 70, 100)).toEqual([95, 95]);
  });
});
