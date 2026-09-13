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

/** Les seules classes MediaWiki qu'on reconnaît, traduites en classes à nous.
 *
 * L'attribut `class` d'origine n'est **jamais** recopié : leurs noms changent
 * au gré de leurs refontes, et en garder des centaines reviendrait à hériter
 * d'une feuille de style qu'on n'écrit pas. On en retient deux, celles qui
 * portent une vraie information de mise en page — le reste du balisage se met
 * en forme par sa balise.
 *
 * L'infobox est la raison d'être de cette table : sans elle, impossible de la
 * distinguer d'un tableau ordinaire, donc impossible de la faire flotter à
 * droite comme sur Wikipédia. Elle s'étalait sur toute la largeur en tête
 * d'article et repoussait le texte d'un écran entier. */
const CLASS_MAP: [string, string][] = [
  ["infobox", "wiki-infobox"],
  ["wikitable", "wiki-table"],
  // **L'alignement des vignettes est une donnee, pas une devinette.** Mesure
  // sur `Audi TT` : MediaWiki emet
  // `<figure class="mw-default-size mw-halign-right" typeof="mw:File/Thumb">`.
  // Sans ces trois classes, toutes les illustrations flottaient a droite --- y
  // compris celles que l'article veut dans le fil --- et rien ne disait
  // lesquelles.
  ["mw-halign-right", "wiki-right"],
  ["mw-halign-left", "wiki-left"],
  ["mw-halign-center", "wiki-center"],
  // **Et le balisage ancien, que Wikipedia sert encore selon les pages** :
  // `<div class="thumb tright">` au lieu de `<figure class="mw-halign-right">`.
  // Ne reconnaitre que le moderne a coute un tour : les vignettes cessaient de
  // flotter au lieu de former une colonne, donc elles se posaient au milieu du
  // texte --- pire que le defaut qu'on corrigeait.
  ["tright", "wiki-right"],
  ["tleft", "wiki-left"],
  ["tnone", "wiki-center"],
  ["thumb", "wiki-thumb"],
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

/** Largeur, en pixels, au-dela de laquelle une image est du **contenu** et non
 * une icone posee dans le fil du texte.
 *
 * MediaWiki ecrit la taille voulue sur chaque `<img>`, et la difference est
 * franche : une note ANCAP est faite de cinq etoiles de vingt pixels cote a
 * cote, une photo d'infobox en fait deux cent cinquante. Ignorer cette taille
 * et servir la vignette de 640 px a tout le monde transformait ces cinq
 * etoiles en cinq affiches empilees, chacune avec sa ligne de credit --- vu
 * sur l'article `Audi TT`. */
const ICON_MAX_WIDTH = 64;

/** Une image affichable, avec ce que sa licence impose (§9). */
export interface AllowedImage {
  file: string;
  url: string;
  descriptionUrl: string;
  artist: string;
  licence: string;
  /** Taille d'affichage de `url`, en pixels CSS. 0 quand elle est inconnue. */
  width: number;
  height: number;
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

/** Cette image est-elle une icone du fil du texte plutot qu'une illustration ?
 *
 * Decide sur la taille que **la page** demande, jamais sur le fichier : la meme
 * etoile sert d'icone ici et d'illustration ailleurs. Sans dimension declaree
 * on tranche pour « contenu » --- se tromper dans ce sens donne une image trop
 * grande, l'inverse la rend illisible. */
export function isIconSized(width: string | null, height: string | null): boolean {
  const w = Number(width);
  const h = Number(height);
  if (!Number.isFinite(w) || w <= 0) return false;
  if (Number.isFinite(h) && h > ICON_MAX_WIDTH) return false;
  return w <= ICON_MAX_WIDTH;
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
  const source = el.getAttribute("class");
  if (source) {
    const list = source.split(/\s+/);
    const mapped = CLASS_MAP.filter(([from]) => list.includes(from)).map(([, to]) => to);
    if (mapped.length) copy.setAttribute("class", mapped.join(" "));
  }
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

  // Le crédit est pose juste apres l'image, donc **avant** la legende de
  // l'article. A la lecture, l'ordre naturel est l'inverse : la legende dit ce
  // qu'on voit, le credit dit d'ou ca vient. On le renvoie en fin de figure.
  if (tag === "figure") {
    for (const credit of Array.from(copy.querySelectorAll(":scope > .wiki-credit"))) {
      copy.appendChild(credit);
    }
  }
  into.appendChild(copy);
}

/** Pose l'image **et son crédit**, ou ne pose rien. */
function appendImage(el: Element, into: Node, images: Map<string, AllowedImage>): void {
  const src = el.getAttribute("src") ?? "";
  const file = fileNameFromSrc(src);
  const credit = file ? images.get(file) : undefined;
  if (!credit) return;

  // **La taille que la page demande decide de tout ce qui suit.** Une icone
  // garde sa propre source et ses propres dimensions : la vignette de 640 px
  // qu'on demande pour les illustrations n'a aucun sens pour une etoile de
  // notation, et la servir la rendait enorme.
  const width = el.getAttribute("width");
  const height = el.getAttribute("height");
  if (isIconSized(width, height)) {
    const icon = document.createElement("img");
    icon.className = "wiki-icon";
    icon.setAttribute("src", src);
    icon.setAttribute("alt", el.getAttribute("alt") ?? "");
    if (width) icon.setAttribute("width", width);
    if (height) icon.setAttribute("height", height);
    // **L'attribution ne disparait pas, elle change de forme.** Une ligne de
    // credit sous une etoile de vingt pixels est illisible et casse la ligne de
    // texte ; l'auteur et la licence passent en infobulle, et l'icone mene a sa
    // page Commons. C'est ce que fait Wikipedia elle-meme pour ses images en
    // ligne, et ce que « crediter de maniere raisonnable » autorise.
    icon.setAttribute("title", `${credit.artist} - ${credit.licence}`);
    if (credit.descriptionUrl) icon.setAttribute("data-href", credit.descriptionUrl);
    into.appendChild(icon);
    return;
  }

  const img = document.createElement("img");
  img.setAttribute("src", credit.url);
  img.setAttribute("alt", el.getAttribute("alt") ?? "");
  img.setAttribute("loading", "lazy");
  // **La place est réservée avant que le fichier arrive**, et ce n'est pas un
  // détail de confort : une `<img>` sans dimensions n'occupe rien tant qu'elle
  // n'est pas chargée, et `loading="lazy"` garantit qu'elle ne l'est pas tant
  // qu'on ne s'en approche pas. Un article de trente photos était donc mis en
  // page un écran trop court — le sommaire sautait à côté de la section
  // demandée, puis chaque image qui se chargeait *au-dessus* du lecteur
  // grandissait d'un coup, ce à quoi l'ancrage de défilement de Chromium
  // répond en repoussant la position vers le bas : impossible de remonter.
  // Les deux attributs valent `width`/`height` en CSS, que `height: auto` et
  // `max-width: 100%` reprennent ensuite — la boîte réservée et l'image
  // chargée ont donc exactement la même taille, donc aucun saut. Bug réel,
  // rapporté sur l'onglet Wikipédia d'une fiche voiture.
  if (credit.width > 0 && credit.height > 0) {
    img.setAttribute("width", String(credit.width));
    img.setAttribute("height", String(credit.height));
  }
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
