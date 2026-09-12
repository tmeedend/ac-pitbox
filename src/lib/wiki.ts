// Onglet Wikipédia de la fiche (docs/SPEC-wikipedia-fiche-detail.md §7).
//
// **L'absence n'est jamais une erreur** (§1) : pas d'article veut dire pas
// d'onglet, sans message ni icône. C'est pourquoi tout ce qui échoue ici rend
// `null` plutôt que de propager — le seul reproche qu'on puisse faire à cette
// fonctionnalité, c'est de déranger.
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
}

/** Clé de la langue de lecture préférée (§5.1) : **globale**, pas par mod. */
const LANG_KEY = "pitbox.wiki.lang";

/** La langue dans laquelle demander l'article : celle choisie dans les
 * réglages, sinon celle de l'application. */
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
 * L'article d'un mod, ou `null` s'il n'y en a pas.
 *
 * Peut prendre plusieurs secondes au premier appel (appariement + réseau), et
 * c'est assumé : la §7.5 veut que la fiche s'affiche **complète et
 * immédiatement**, l'onglet apparaissant quand le contenu arrive.
 */
export async function getWikiArticle(modKey: string, lang = wikiLang()): Promise<WikiArticle | null> {
  try {
    return await invoke<WikiArticle | null>("get_wiki_article", { modKey, lang });
  } catch (e) {
    // Un échec ici n'a rien à dire à l'utilisateur (§1) ; il a tout à dire au
    // journal, sans quoi une install packagée ne laisse aucune trace.
    console.warn("get_wiki_article", modKey, e);
    return null;
  }
}

/** La langue réellement servie, lue sur l'URL — `fr.wikipedia.org` → `fr`.
 * Le backend fait le même calcul ; c'est l'URL qui fait foi, elle vient de
 * l'API. */
export function articleLang(article: WikiArticle): string | null {
  const m = /^https:\/\/([a-z-]+)\.wikipedia\.org\//.exec(article.articleUrl);
  return m ? m[1] : null;
}
