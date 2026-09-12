// Rendu de l'article Wikipédia (SPEC-wikipedia-fiche-detail.md §7.3).
//
// **Le HTML de Wikipédia n'est jamais injecté tel quel.** La webview de Pit Box
// a accès à `invoke` : du balisage tiers posé dans notre DOM aurait la même
// portée que notre propre code, et Wikipédia est éditable par n'importe qui.
// Un filtre qui *retire* ce qui déplaît laisse passer ce qu'il n'a pas prévu ;
// ici on fait l'inverse — on **reconstruit** un arbre neuf en n'y recopiant que
// ce qui figure explicitement dans les listes ci-dessous. Ce qui n'est pas
// prévu n'existe pas.
//
// `DOMParser` sert d'analyseur : il produit un document **détaché**, où les
// `<script>` ne s'exécutent pas et les `src` ne se chargent pas. Rien n'entre
// dans le document vivant sans être repassé par `rebuild`.
//
// Aucune dépendance : ni assainisseur côté Rust, ni DOMPurify. La sécurité
// vient de la liste blanche, pas d'une bibliothèque.

/** Balises recopiées telles quelles. Tout le reste est soit déplié (ses enfants
 * remontent), soit abandonné — voir `DROPPED`. */
const ALLOWED = new Set([
  "p",
  "br",
  "b",
  "strong",
  "i",
  "em",
  "u",
  "s",
  "sub",
  "sup",
  "small",
  "abbr",
  "code",
  "kbd",
  "blockquote",
  "q",
  "cite",
  "ul",
  "ol",
  "li",
  "dl",
  "dt",
  "dd",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "table",
  "caption",
  "thead",
  "tbody",
  "tfoot",
  "tr",
  "th",
  "td",
  "figure",
  "figcaption",
  "hr",
  "a",
  "img",
  "span",
  "div",
]);

/** Balises dont **le contenu aussi** disparaît. Le reste de l'inconnu est
 * simplement déplié : perdre un `<section>` ne coûte rien, perdre le texte
 * qu'il contient coûterait l'article. */
const DROPPED = new Set(["script", "style", "noscript", "template", "iframe", "object", "embed", "svg", "math", "audio", "video"]);

/** Classes MediaWiki dont le bloc entier est retiré : navigation, appels à
 * contribution, tables de matières doublonnées, bandeaux de maintenance. Ce
 * n'est pas de la censure, c'est du hors-sujet — l'utilisateur lit une fiche de
 * mod, pas une page de wiki. */
const DROPPED_CLASSES = [
  "navbox",
  "vertical-navbox",
  "metadata",
  "ambox",
  "mw-editsection",
  "toc",
  "sistersitebox",
  "noprint",
  "hatnote",
  "shortdescription",
  "mw-empty-elt",
  "reference",
  "reflist",
  "refbegin",
  "mw-references-wrap",
  "printfooter",
  "catlinks",
];

/** Attributs conservés, par balise. Rien d'autre ne passe — en particulier
 * aucun `on*`, aucun `style`, aucun `srcset`. */
const ATTRS: Record<string, string[]> = {
  a: ["href"],
  img: ["src", "alt", "width", "height"],
  th: ["colspan", "rowspan"],
  td: ["colspan", "rowspan"],
  h2: ["id"],
  h3: ["id"],
  h4: ["id"],
  h5: ["id"],
  h6: ["id"],
};

/** Une image affichable, avec ce que sa licence impose (§9). */
export interface AllowedImage {
  file: string;
  url: string;
  descriptionUrl: string;
  artist: string;
  licence: string;
}

/** Le nom de fichier derrière une balise `<img>` de MediaWiki.
 *
 * Lu sur l'URL du vignettage : `…/commons/thumb/9/95/Nom.jpg/640px-Nom.jpg`, ou
 * `…/commons/9/95/Nom.jpg` sans vignettage. Le nom est celui **avant** la
 * dernière barre quand le chemin passe par `/thumb/`.
 *
 * Pur, donc testable sans DOM. */
export function fileNameFromSrc(src: string): string | null {
  const clean = src.split(/[?#]/)[0];
  // **L'hôte doit être Wikimedia.** Sans cette garde, une `data:` URL rendait
  // « png;base64,AAAA » — inoffensif, puisque ce nom ne figure dans aucune liste
  // blanche, mais une fonction qui répond n'importe quoi finit par être crue.
  // Trouvé par son propre test.
  const withoutScheme = clean.replace(/^https?:/i, "");
  if (!withoutScheme.startsWith("//")) return null;
  const host = withoutScheme.slice(2).split("/")[0].toLowerCase();
  if (host !== "wikimedia.org" && !host.endsWith(".wikimedia.org")) return null;

  const parts = clean.split("/").filter(Boolean);
  if (parts.length < 2) return null;
  const thumbAt = parts.indexOf("thumb");
  // Avec vignettage, le vrai nom est l'avant-dernier segment ; sans, le dernier.
  const name = thumbAt >= 0 && parts.length >= 2 ? parts[parts.length - 2] : parts[parts.length - 1];
  try {
    return decodeURIComponent(name).replace(/_/g, " ");
  } catch {
    return name.replace(/_/g, " ");
  }
}

/** Un lien interne de wiki (`/wiki/Titre`) rendu absolu, ou `null` si le lien
 * n'est pas exploitable. Tout ce qui n'est pas `http(s)` est refusé — plus de
 * `javascript:`, plus de `data:`. */
export function resolveHref(href: string, lang: string): string | null {
  if (href.startsWith("//")) return `https:${href}`;
  if (href.startsWith("/wiki/")) return `https://${lang}.wikipedia.org${href}`;
  if (/^https?:\/\//i.test(href)) return href;
  return null;
}

function hasDroppedClass(el: Element): boolean {
  const cls = el.getAttribute("class");
  if (!cls) return false;
  const list = cls.split(/\s+/);
  return DROPPED_CLASSES.some((bad) => list.includes(bad));
}

/**
 * Reconstruit un fragment sûr à partir du HTML de l'article.
 *
 * `images` est la liste **blanche** : une `<img>` dont le fichier n'y figure pas
 * est retirée avec sa figure. C'est là que se joue la conformité du §9 — ce qui
 * n'a pas de licence et d'auteur connus ne s'affiche pas.
 */
export function renderArticle(html: string, lang: string, images: AllowedImage[]): DocumentFragment {
  const byFile = new Map(images.map((i) => [i.file, i]));
  // Document détaché : rien ne s'exécute, rien ne se charge.
  const parsed = new DOMParser().parseFromString(html, "text/html");
  const out = document.createDocumentFragment();
  for (const child of Array.from(parsed.body.childNodes)) {
    rebuild(child, out, lang, byFile);
  }
  return out;
}

function rebuild(node: Node, into: Node, lang: string, images: Map<string, AllowedImage>): void {
  if (node.nodeType === Node.TEXT_NODE) {
    into.appendChild(document.createTextNode(node.nodeValue ?? ""));
    return;
  }
  if (node.nodeType !== Node.ELEMENT_NODE) return;

  const el = node as Element;
  const tag = el.tagName.toLowerCase();
  if (DROPPED.has(tag) || hasDroppedClass(el)) return;

  if (tag === "img") {
    appendImage(el, into, images);
    return;
  }

  // Inconnue : on déplie. Le texte d'un `<section>` vaut mieux que le silence.
  if (!ALLOWED.has(tag)) {
    for (const child of Array.from(el.childNodes)) rebuild(child, into, lang, images);
    return;
  }

  const copy = document.createElement(tag);
  for (const attr of ATTRS[tag] ?? []) {
    const value = el.getAttribute(attr);
    if (value === null) continue;
    if (attr === "href") {
      const href = resolveHref(value, lang);
      if (href) copy.setAttribute("data-href", href);
      continue;
    }
    copy.setAttribute(attr, value);
  }
  for (const child of Array.from(el.childNodes)) rebuild(child, copy, lang, images);

  // Une figure vidée de son image (non libre) ne laisse pas sa légende
  // orpheline flotter au milieu du texte.
  if (tag === "figure" && !copy.querySelector("img")) return;
  into.appendChild(copy);
}

/** Pose l'image **et son crédit**, ou ne pose rien. */
function appendImage(el: Element, into: Node, images: Map<string, AllowedImage>): void {
  const src = el.getAttribute("src") ?? "";
  const file = fileNameFromSrc(src);
  const credit = file ? images.get(file) : undefined;
  if (!credit) return;

  const img = document.createElement("img");
  img.setAttribute("src", credit.url);
  img.setAttribute("alt", el.getAttribute("alt") ?? "");
  img.setAttribute("loading", "lazy");
  into.appendChild(img);

  // **Obligatoire, pas décoratif** : les fichiers libres de Commons imposent de
  // créditer l'auteur individuellement, à côté de l'œuvre. Sans cette ligne,
  // l'image ne peut pas être affichée du tout.
  const cap = document.createElement("span");
  cap.className = "wiki-credit";
  cap.textContent = `${credit.artist} · ${credit.licence}`;
  if (credit.descriptionUrl) cap.setAttribute("data-href", credit.descriptionUrl);
  into.appendChild(cap);
}
