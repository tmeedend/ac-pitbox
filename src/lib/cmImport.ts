// Import des presets de grille de Content Manager (§6).
//
// **Import, jamais synchronisation** : on lit une fois, on convertit, et on ne
// dépend plus jamais du fichier. Le classement présent / absent / désactivé se
// fait ici et non côté Rust, parce que la bibliothèque est déjà chargée côté
// écran : deux sources de vérité sur « ce mod est-il activé » finiraient par
// se contredire, et c'est la carte que l'utilisateur voit qui fait foi.
import { invoke } from "@tauri-apps/api/core";
import type { ModCard } from "./library";
import type { Opponent } from "./launch";
import type { SavedGrid } from "./savedGrids";

export interface CmOpponent {
  car_id: string;
  car_skin: string | null;
  /** `null` = `Auto`, le `-1` de CM. */
  ai_level: number | null;
  driver_name: string | null;
  nationality: string | null;
  ballast: number;
  restrictor: number;
}

export interface CmGrid {
  name: string;
  /** Chemin relatif au dossier Presets — ce qui permet de dire lequel. */
  source: string;
  opponents: CmOpponent[];
  ai_level_min: number;
  ai_level_max: number;
  aggression: number;
}

export interface CmSkipped {
  name: string;
  source: string;
  /** Clé i18n, jamais une phrase : `errors.cmNotAGrid` / `errors.cmUnreadable`. */
  reason: string;
}

export interface CmScan {
  grids: CmGrid[];
  skipped: CmSkipped[];
  /** `null` quand Content Manager n'est pas installé — un non-résultat, pas
   * une panne (§6.1). */
  root: string | null;
}

export function scanCmPresets(): Promise<CmScan> {
  return invoke<CmScan>("scan_cm_presets");
}

/** Ce qu'un import a donné, pour le rapport (§6.3). */
export interface CmImportReport {
  /** Noms réellement écrits — un nom déjà pris est suffixé, pas écrasé. */
  imported: string[];
  /** Voitures citées par les presets et absentes de la bibliothèque. */
  missing: string[];
  /** Présentes mais désactivées : la garde d'activation les répare d'un clic
   * dès qu'une grille qui les contient est chargée en session. */
  disabled: string[];
  skipped: CmSkipped[];
}

/**
 * Convertit les grilles lues en grilles Pit Box, et classe les voitures citées.
 *
 * **L'import aboutit toujours** (§6.3). Une voiture absente de la bibliothèque
 * est retirée de la grille en le disant — importer une ligne qui pointe vers
 * rien produirait une session qui échoue, ce qui est exactement le contraire du
 * service rendu. Une voiture simplement désactivée, elle, **reste** : le mod est
 * là, et la garde d'activation le rallume d'un clic.
 */
export function convertCmGrids(grids: CmGrid[], cars: ModCard[]): { grids: SavedGrid[]; missing: string[]; disabled: string[] } {
  const byId = new Map(cars.map((c) => [c.id_interne, c]));
  const missing = new Set<string>();
  const disabled = new Set<string>();
  const out: SavedGrid[] = [];
  for (const g of grids) {
    const opponents: Opponent[] = [];
    for (const o of g.opponents) {
      const card = byId.get(o.car_id);
      if (!card) {
        missing.add(o.car_id);
        continue;
      }
      if (!card.active) disabled.add(o.car_id);
      opponents.push({
        car_id: o.car_id,
        car_skin: o.car_skin,
        ai_level: o.ai_level,
        driver_name: o.driver_name,
        nationality: o.nationality,
        ballast: o.ballast,
        restrictor: o.restrictor,
      });
    }
    // Une grille dont aucune voiture ne subsiste n'est pas une grille vide :
    // c'est un import sans objet, et l'écrire donnerait une ligne trompeuse
    // dans la liste. Ses voitures sont déjà comptées comme absentes.
    if (!opponents.length) continue;
    out.push({
      name: g.name,
      savedAt: new Date().toISOString(),
      opponents,
      aiLevelMin: g.ai_level_min,
      aiLevelMax: g.ai_level_max,
      aggression: g.aggression,
      fromCm: true,
    });
  }
  return { grids: out, missing: [...missing].sort(), disabled: [...disabled].sort() };
}
