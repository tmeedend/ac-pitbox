// Tenues complètes enregistrées (docs/SPEC-ecran-pilote.md §13, complément).
//
// Le choix du pilote est déjà global et persistant : rouvrir l'app retrouve
// le casque, la combinaison et les gants d'hier. Ce que ça ne donne pas,
// c'est **plusieurs** tenues — celle qu'on met en GT et celle qu'on met en
// historique — parce qu'en changer veut alors dire refaire quatre choix, dont
// un dans une galerie de cent casques.
//
// D'où cet enregistrement : les quatre pièces d'un coup, sous un nom, et un
// clic pour les reposer toutes. C'est le même geste que les presets de
// session (`saved_sessions.rs`), en plus petit.
//
// Persistance dans `ui_prefs.json` comme les favoris et les récents (règle
// d'or n°6 : jamais `localStorage`). Une liste de quatre chaînes par entrée ne
// justifie pas un fichier Rust dédié.
import { getUiPref, setUiPref } from "./uiPrefs.svelte";

const KEY = "pitbox.driver.outfits";
/** Au-delà, la rangée de pastilles déborde et ne sert plus de raccourci.
 * Enregistrer une tenue de plus retire la plus ancienne, sans le dire —
 * l'inverse (refuser) obligerait à faire le ménage avant de sauver. */
const MAX = 12;

export interface SavedOutfit {
  name: string;
  /** Les quatre pièces, telles que `driverOverride` les stocke. `null` = ce
   * que la voiture ou sa livrée décide. */
  body: string | null;
  helmet: string | null;
  suit: string | null;
  gloves: string | null;
}

const values = $state<{ list: SavedOutfit[] }>({ list: [] });

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPref(KEY).then((raw) => {
    values.list = parse(raw);
  });
  return loaded;
}

void ensureLoaded();

function parse(raw: string | null): SavedOutfit[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is SavedOutfit => {
      const o = entry as Partial<SavedOutfit> | null;
      return !!o && typeof o.name === "string" && o.name.length > 0;
    });
  } catch {
    return [];
  }
}

function persist(): void {
  setUiPref(KEY, JSON.stringify(values.list));
}

/** L'état courant, réactif. */
export function savedOutfits(): SavedOutfit[] {
  return values.list;
}

/**
 * Enregistre la tenue courante sous ce nom, en remplaçant celle du même nom
 * s'il y en a une.
 *
 * Le remplacement est délibéré : réenregistrer sous un nom existant est la
 * façon naturelle de dire « en fait, ma tenue GT c'est plutôt ça », et
 * refuser obligerait à supprimer puis resauver.
 */
export function saveOutfit(outfit: SavedOutfit): void {
  const name = outfit.name.trim();
  if (!name) return;
  const kept = values.list.filter((o) => o.name.toLowerCase() !== name.toLowerCase());
  values.list = [{ ...outfit, name }, ...kept].slice(0, MAX);
  persist();
}

/** Les quatre pièces d'une tenue enregistrée, par son nom. Sert à résoudre la
 * tenue par défaut : c'est le **nom** qui est stocké ailleurs, pas les pièces,
 * pour que modifier une tenue suive partout où elle sert. */
export function outfitByName(name: string): SavedOutfit | null {
  return values.list.find((o) => o.name === name) ?? null;
}

/**
 * La tenue enregistrée que le pilote porte en ce moment, s'il en porte une.
 *
 * Sert au libellé de la colonne de session : quand on a nommé une tenue, c'est
 * son nom qui doit s'afficher, pas un « Mon pilote » qui ne dit rien de ce
 * qu'on a mis.
 */
export function wornOutfit(current: {
  body: string | null;
  helmet: string | null;
  suit: string | null;
  gloves: string | null;
}): SavedOutfit | null {
  return (
    values.list.find(
      (o) =>
        o.body === current.body &&
        o.helmet === current.helmet &&
        o.suit === current.suit &&
        o.gloves === current.gloves,
    ) ?? null
  );
}

export function deleteOutfit(name: string): void {
  values.list = values.list.filter((o) => o.name !== name);
  persist();
}
