// Grilles d'adversaires enregistrées (§5) : distinctes des sessions
// enregistrées, et la distinction n'est pas une nuance.
//
// Une **session** enregistre tout — duo, météo, heure, options — dont une
// **copie** de sa grille. Une **grille** n'enregistre que les adversaires, et
// c'est ce qui la rend rejouable ailleurs : le même plateau GT3 sur dix
// circuits. Charger une grille dans une session déjà configurée ne doit donc
// toucher qu'aux adversaires.
//
// **Une copie, jamais un lien** : sinon modifier une grille changerait en
// silence toutes les sessions qui la citent. C'est la règle qui justifie à elle
// seule les deux objets.
//
// Pas de rangement par type de session, contrairement aux sessions : une grille
// ne dépend pas du type de course qu'on fera avec, c'est tout son intérêt.
import { invoke } from "@tauri-apps/api/core";
import type { Opponent } from "./launch";

export interface SavedGrid {
  name: string;
  savedAt: string;
  /** Les adversaires, et rien d'autre. */
  opponents: Opponent[];
  /** Fourchette de force et agressivité : elles font partie du caractère du
   * plateau, pas du circuit ni de la météo. Une grille rejouée ailleurs doit
   * courir comme elle courait. */
  aiLevelMin: number;
  aiLevelMax: number;
  aggression: number;
  /** Importée d'un preset Content Manager (§6) — porte le badge `From CM`, et
   * rien d'autre ne l'en distingue : une fois convertie, c'est une grille
   * Pit Box comme les autres. */
  fromCm?: boolean;
}

/** Persistance durable (§5) : fichier écrit côté Rust (`saved_grids.json`,
 * `std::fs::write` synchrone), jamais `localStorage` — règle d'or n°6. */
async function loadAll(): Promise<Record<string, SavedGrid>> {
  return invoke<Record<string, SavedGrid>>("get_saved_grids").catch(() => ({}));
}

function persist(all: Record<string, SavedGrid>): Promise<void> {
  // Pas d'`invokeSafe` ici : c'est une **écriture**, et un repli muet y rendrait
  // « enregistré » une commande qui n'a rien écrit (règle d'or n°6). Un échec
  // est journalisé, jamais avalé.
  return invoke<void>("save_saved_grids", { all }).catch((e) => {
    console.error("save_saved_grids", e);
    throw e;
  });
}

/** Les grilles, la plus récente d'abord. */
export async function listSavedGrids(): Promise<SavedGrid[]> {
  const all = await loadAll();
  return Object.values(all).sort((a, b) => b.savedAt.localeCompare(a.savedAt));
}

/** Enregistre, ou écrase si le nom existe déjà. */
export async function saveGrid(grid: SavedGrid): Promise<void> {
  const all = await loadAll();
  all[grid.name] = grid;
  await persist(all);
}

export async function deleteSavedGrid(name: string): Promise<void> {
  const all = await loadAll();
  delete all[name];
  await persist(all);
}

/** Ajoute plusieurs grilles d'un coup (import CM, §6) en **une** écriture.
 * Les noms déjà pris sont suffixés plutôt qu'écrasés : un import ne doit
 * jamais détruire ce que l'utilisateur a composé lui-même. Renvoie les noms
 * réellement écrits. */
export async function addGrids(grids: SavedGrid[]): Promise<string[]> {
  const all = await loadAll();
  const written: string[] = [];
  for (const g of grids) {
    let name = g.name;
    let n = 2;
    while (all[name]) name = `${g.name} (${n++})`;
    all[name] = { ...g, name };
    written.push(name);
  }
  await persist(all);
  return written;
}
