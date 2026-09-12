// Onglet Wikipédia de la fiche (docs/SPEC-wikipedia-fiche-detail.md §7).
//
// **L'onglet est permanent**, écart assumé avec la §7.1 et décidé avec
// l'utilisateur : un onglet absent ne se distingue ni d'une recherche en cours,
// ni d'une fonctionnalité inexistante — et le moment où l'on a le plus besoin
// d'agir est justement celui où rien n'a été trouvé. D'où `WikiPanel`, qui
// porte un **état** et non un simple `article | null`.
//
// Ce que la §1 garde : aucun de ces états n'est une erreur. Rien ici ne
// propage ; tout échec rend un panneau ou une liste vide.
import { invoke } from "@tauri-apps/api/core";
import { i18n } from "./i18n/index.svelte";
import { getUiPref, peekUiPref, setUiPref } from "./uiPrefs.svelte";

/** Une entrée de cache telle que le backend la rend (§3.2). */
export interface WikiArticle {
  entityId: string;
  /** Langue **demandée**, pas forcément celle du texte — voir `articleLang`. */
  lang: string;
  articleTitle: string;
  articleUrl: string;
  revisionId: number | null;
  extract: string;
  /** Non nul quand le texte est celui de l'entité parente (§5.3). */
  parentEntity: string | null;
  availableLangs: string[];
  fetchedAt: string;
  /** L'article rendu par MediaWiki. Vide = le rendu n'a pas pu être obtenu, et
   * `extract` (texte brut) porte l'onglet à lui seul. */
  html: string;
  sections: WikiSection[];
  /** Les seules images affichables : Commons, licence et auteur connus (§9). */
  images: WikiImage[];
}

/** Une entrée de la table des matières. */
export interface WikiSection {
  level: number;
  line: string;
  anchor: string;
}

/** Une image, avec ce que sa licence impose d'afficher à côté. */
export interface WikiImage {
  file: string;
  url: string;
  descriptionUrl: string;
  artist: string;
  licence: string;
}

/** Pourquoi l'onglet montre ce qu'il montre. Aucun n'est une erreur. */
export type WikiState = "article" | "offline" | "ambiguous" | "noCandidate" | "noArticle" | "unavailable";

export interface WikiPanel {
  article: WikiArticle | null;
  state: WikiState;
  /** Ce qui a été cherché : le champ de recherche part de là, pour que
   * l'utilisateur corrige des mots-clés au lieu de tout retaper. */
  query: string;
  entityId: string | null;
}

/** Un candidat de la recherche libre (§7.6). */
export interface WikiSuggestion {
  entityId: string;
  label: string;
  /** La description courte de Wikidata, celle qui lève l'ambiguïté d'un
   * coup d'œil. */
  description: string | null;
}

/** Clé de la langue de lecture préférée (§5.1) : **globale**, pas par mod. */
const LANG_KEY = "pitbox.wiki.lang";

export function wikiLang(): string {
  return peekUiPref(LANG_KEY) || i18n.locale;
}

export async function setWikiLang(lang: string | null): Promise<void> {
  await setUiPref(LANG_KEY, lang ?? "");
}

/** Précharge la préférence pour que `wikiLang()` soit juste au premier rendu. */
export async function loadWikiLang(): Promise<void> {
  await getUiPref(LANG_KEY);
}

/**
 * Ce que l'onglet affiche pour un mod.
 *
 * Peut prendre quelques secondes au premier appel (appariement + article), et
 * c'est assumé : la §7.5 veut que la fiche s'affiche **complète et
 * immédiatement**, le contenu de l'onglet arrivant ensuite. `null` pendant ce
 * temps-là, ce que l'onglet dit explicitement.
 */
export async function getWikiPanel(modKey: string, lang = wikiLang()): Promise<WikiPanel> {
  try {
    return await invoke<WikiPanel>("get_wiki_panel", { modKey, lang });
  } catch (e) {
    // Un échec ici n'a rien à dire à l'utilisateur (§1) ; il a tout à dire au
    // journal, sans quoi une install packagée ne laisse aucune trace.
    console.warn("get_wiki_panel", modKey, e);
    return { article: null, state: "unavailable", query: "", entityId: null };
  }
}

/** Recherche libre, filtre de type relâché (§7.6). Accepte aussi une URL
 * Wikipédia collée, résolue en Q-id côté backend. */
export async function searchWikiCandidates(query: string, lang = wikiLang()): Promise<WikiSuggestion[]> {
  try {
    return await invoke<WikiSuggestion[]>("search_wiki_candidates", { query, lang });
  } catch (e) {
    console.warn("search_wiki_candidates", e);
    return [];
  }
}

/** Associe un article à la main : enregistré en `manual`, donc protégé de
 * toute reprise automatique et de la table livrée (§3.1). */
export async function setWikiLink(modKey: string, entityId: string): Promise<void> {
  try {
    await invoke<void>("set_wiki_link", { modKey, entityId });
  } catch (e) {
    console.warn("set_wiki_link", modKey, entityId, e);
  }
}

/** Détache l'article : un mod sans appariement est une réponse valable (§1). */
export async function clearWikiLink(modKey: string): Promise<void> {
  try {
    await invoke<void>("clear_wiki_link", { modKey });
  } catch (e) {
    console.warn("clear_wiki_link", modKey, e);
  }
}

/** §8 — vide le cache d'articles et le cache négatif, garde les appariements. */
export async function purgeWikiCache(): Promise<void> {
  try {
    await invoke<void>("purge_wiki_cache");
  } catch (e) {
    console.warn("purge_wiki_cache", e);
  }
}

/** §10 — écrit `rules/wiki-links.json` (corrections locales fondues dedans) à
 * l'endroit choisi, prêt à recoller dans le dépôt. Rend `false` en cas
 * d'échec. */
export async function exportWikiLinks(path: string): Promise<boolean> {
  try {
    await invoke<void>("export_wiki_links", { path });
    return true;
  } catch (e) {
    console.error("export_wiki_links", e);
    return false;
  }
}

/** Combien de corrections manuelles l'export contiendrait. Sert à ne pas
 * proposer d'exporter le vide. */
export async function countWikiManualLinks(): Promise<number> {
  try {
    return await invoke<number>("count_wiki_manual_links");
  } catch (e) {
    console.warn("count_wiki_manual_links", e);
    return 0;
  }
}

/** La langue réellement servie, lue sur l'URL — `fr.wikipedia.org` → `fr`.
 * Le backend fait le même calcul ; c'est l'URL qui fait foi, elle vient de
 * l'API. */
export function articleLang(article: WikiArticle): string | null {
  const m = /^https:\/\/([a-z-]+)\.wikipedia\.org\//.exec(article.articleUrl);
  return m ? m[1] : null;
}
