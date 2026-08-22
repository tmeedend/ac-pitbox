// Système i18n minimal (aucune dépendance externe) : dictionnaires JSON par
// langue, recherche de clé en points (`section.cle`), interpolation simple
// `{nom}`. Ajouter une langue = copier un fichier locales/xx.json et le
// déclarer dans `locales` ci-dessous.
import fr from "./locales/fr.json";
import en from "./locales/en.json";
import it from "./locales/it.json";
import de from "./locales/de.json";
import es from "./locales/es.json";
import pt from "./locales/pt.json";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Dict = Record<string, any>;

export const locales: Record<string, Dict> = { fr, en, it, de, es, pt };

// Chaque langue s'écrit dans sa propre langue : c'est ainsi qu'on la reconnaît
// dans une liste dont on ne comprend pas, par définition, la langue courante.
export const localeNames: Record<string, string> = {
  fr: "Français",
  en: "English",
  it: "Italiano",
  de: "Deutsch",
  es: "Español",
  pt: "Português",
};

export const availableLocales = Object.keys(locales);

const FALLBACK = "en";

/** Langue du système, réduite à son code à deux lettres.
 *
 * `pt-BR` et `pt-PT` retombent donc tous deux sur `pt`, `zh-CN` sur `zh` : le
 * dictionnaire `pt` est écrit en portugais **brésilien**, de très loin la plus
 * grosse communauté Assetto Corsa lusophone. Distinguer les variantes
 * demanderait de comparer la balise complète avant de tronquer — à faire le
 * jour où une variante est réellement traduite à part, pas avant. */
function systemLocale(): string {
  const nav = typeof navigator !== "undefined" ? navigator.language : FALLBACK;
  const code = (nav ?? FALLBACK).slice(0, 2).toLowerCase();
  return availableLocales.includes(code) ? code : FALLBACK;
}

export const i18n = $state<{ locale: string }>({ locale: systemLocale() });

/** Force une langue (ou repasse à la détection système si `null`). */
export function setLocale(code: string | null): void {
  i18n.locale = code && availableLocales.includes(code) ? code : systemLocale();
}

function lookup(dict: Dict, key: string): string | undefined {
  let cur: unknown = dict;
  for (const part of key.split(".")) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Dict)[part];
  }
  return typeof cur === "string" ? cur : undefined;
}

/** Traduit `key` (chemin en points, ex. "library.filters.author") dans la
 * langue courante, avec repli sur l'anglais puis sur la clé elle-même.
 * `params` interpole `{nom}` dans le texte trouvé. */
export function t(key: string, params?: Record<string, string | number>): string {
  const str = lookup(locales[i18n.locale], key) ?? lookup(locales[FALLBACK], key) ?? key;
  if (!params) return str;
  return Object.entries(params).reduce((s, [k, v]) => s.replaceAll(`{${k}}`, String(v)), str);
}
