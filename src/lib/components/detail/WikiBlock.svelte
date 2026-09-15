<script lang="ts">
  // L'onglet Wikipédia de la fiche (docs/SPEC-wikipedia-fiche-detail.md §7).
  //
  // **L'onglet est permanent**, et c'est un écart assumé avec la §7.1, décidé
  // avec l'utilisateur : un onglet absent ne se distingue ni d'une recherche en
  // cours, ni d'une fonctionnalité qui n'existe pas. Or le cas où l'utilisateur
  // a le plus besoin d'agir est précisément celui où rien n'a été trouvé — et
  // il n'avait alors nulle part où aller.
  //
  // Ce que la §1 garde, et qui compte : **aucun de ces états n'est une
  // erreur.** Pas d'icône d'alerte, pas d'encart rouge, pas de ton d'échec. Une
  // phrase qui dit ce qui s'est passé, et une proposition d'agir.
  //
  // **Contrainte juridique** (§2), elle non négociable : le texte n'est jamais
  // fondu dans la description du mod (deux sous-onglets, deux blocs), il est
  // affiché tel que l'API le rend — pas de reformulation, pas de résumé, pas de
  // traduction — et l'attribution en pied est obligatoire.
  //
  // L'article est affiché **en entier**, demandé par l'utilisateur, et le
  // raisonnement juridique s'en trouve simplifié : la §7.4 exigeait le mot
  // « extrait » parce que ne montrer qu'un fragment est une modification, qui
  // doit être signalée. Reproduire le texte intégralement, tel quel, avec son
  // attribution et sa licence, est exactement ce que CC BY-SA autorise.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { localeNames } from "$lib/i18n/index.svelte";
  import { parseExtract } from "$lib/wikiText";
  import { renderArticle } from "$lib/wikiHtml";
  import WikiImageViewer, { type ViewerImage } from "./WikiImageViewer.svelte";
  import { zoomFactor } from "$lib/zoom.svelte";
  import { pinShell } from "$lib/shellScroll";
  import {
    articleLang,
    clearWikiLink,
    searchWikiCandidates,
    setWikiLink,
    type WikiImage,
    type WikiPanel,
    type WikiSuggestion,
  } from "$lib/wiki";

  interface Props {
    modKey: string;
    panel: WikiPanel | null;
    /** Rechargement demandé au parent (langue changée, article associé). */
    onreload: (lang?: string) => void;
  }
  const { modKey, panel, onreload }: Props = $props();

  const article = $derived(panel?.article ?? null);
  const shown = $derived(article ? articleLang(article) : null);

  // Repli en texte brut : utilisé seulement quand le rendu HTML n'a pas pu
  // être obtenu. Les deux sont récupérés côté Rust pour cette raison — un
  // article dégradé vaut mieux qu'un onglet vide (§1).
  const blocks = $derived(article && !article.html ? parseExtract(article.extract) : []);

  /** Le conteneur où l'article reconstruit est posé. */
  let host = $state<HTMLDivElement | null>(null);

  /** Pose l'article dans le DOM.
   *
   * `renderArticle` ne fait **jamais** d'`innerHTML` : il reconstruit un arbre
   * neuf à partir d'une liste blanche (voir `wikiHtml.ts`). Le HTML de
   * Wikipédia n'entre donc jamais tel quel dans une webview qui a accès à
   * `invoke`.
   *
   * Toutes les dépendances sont lues **en tête**, avant la moindre sortie :
   * une garde placée avant elles tronquerait la liste au premier passage, et
   * le premier passage a lieu au montage, quand `host` est encore nul. */
  $effect(() => {
    const el = host;
    const html = article?.html ?? "";
    const images = article?.images ?? [];
    const lang = shown ?? article?.lang ?? "en";
    if (!el) return;
    el.replaceChildren();
    if (!html) return;
    el.appendChild(renderArticle(html, lang, images));
    pinShellHere();
  });

  /** The article's photographs, in reading order, and which one is open. */
  let viewerImages = $state<ViewerImage[]>([]);
  let viewerIndex = $state<number | null>(null);

  /** Les liens de l'article ouvrent le navigateur système — jamais une
   * navigation interne, qui ferait sortir l'utilisateur de sa fiche. Les `href`
   * ont été remplacés par des `data-href` validés à la reconstruction.
   *
   * Une image fait exception, et c'est le seul clic qui reste chez nous : elle
   * s'ouvre dans la visionneuse (§9). */
  function onArticleClick(event: MouseEvent) {
    // **A photograph is tested first, and it must be.** MediaWiki wraps most
    // illustrations in a link to their file page, so the enclosing `[data-href]`
    // would otherwise win and send the reader to their browser — the one place
    // they did not ask to go when clicking an image inside the app.
    const photo = (event.target as HTMLElement | null)?.closest("img.wiki-photo");
    if (photo && host && article) {
      event.preventDefault();
      openViewer(photo as HTMLImageElement);
      return;
    }

    const target = (event.target as HTMLElement | null)?.closest("[data-href]");
    const href = target?.getAttribute("data-href");
    if (!href) return;
    event.preventDefault();
    openUrl(href).catch(() => {});
  }

  /** Builds the viewer's list from the DOM rather than from `article.images`.
   *
   * The two are not the same list: the API returns every file the page uses,
   * the article shows those the rebuild kept, and only reading order makes
   * « next image » mean anything. Walking the rendered tree is therefore the
   * only source that matches what the reader sees. */
  function openViewer(clicked: HTMLImageElement) {
    if (!host || !article) return;
    const byFile = new Map(article.images.map((i) => [i.file, i]));
    const list: ViewerImage[] = [];
    let at = 0;
    for (const el of Array.from(host.querySelectorAll<HTMLImageElement>("img.wiki-photo"))) {
      const credit = byFile.get(el.dataset.file ?? "");
      if (!credit) continue;
      if (el === clicked) at = list.length;
      list.push({ ...credit, caption: el.dataset.caption ?? "" });
    }
    if (list.length === 0) return;
    viewerImages = list;
    viewerIndex = at;
  }

  /** Le conteneur qui défile réellement au-dessus de nous.
   *
   * Cherché à l'exécution plutôt que codé en dur : l'article vit dans la boîte
   * de texte de la fiche, qui n'a **aucune contrainte de hauteur** — c'est un
   * ancêtre bien plus haut qui porte le défilement, et le savoir de mémoire
   * serait un pari sur une structure qui bouge. */
  function scrollParent(el: HTMLElement): HTMLElement | null {
    let node = el.parentElement;
    // **On s'arrête avant `<body>` et `<html>`, et ce n'est pas de la
    // prudence.** `global.css` les met en `overflow: hidden` exprès : « le
    // document lui-même ne défile jamais, un scroll de page entraînait toute la
    // coquille — barre de titre comprise — hors champ ». Or `scrollTo()`
    // fonctionne **même** sur un élément en `overflow: hidden`, alors que la
    // molette, elle, ne peut plus le ramener. Les atteindre décalait donc la
    // fenêtre entière, définitivement : bande noire en bas, coquille coincée,
    // et aucun geste pour revenir. `hidden` ne passe de toute façon pas le
    // test ci-dessous, mais la garde reste explicite — c'est l'ancêtre à ne
    // jamais toucher.
    while (node && node !== document.body && node !== document.documentElement) {
      const overflow = getComputedStyle(node).overflowY;
      if (/(auto|scroll|overlay)/.test(overflow) && node.scrollHeight > node.clientHeight) return node;
      node = node.parentElement;
    }
    return null;
  }

  /** Marge au-dessus du titre visé, en pixels CSS : collé au bord haut, un
   * titre se lit mal et on ne voit pas ce qui le précède. */
  const SCROLL_MARGIN = 12;

  /** Rétablit l'invariant de la coquille depuis cet onglet : le clic sur une
   * entrée du sommaire donne le focus au titre visé, et le navigateur fait
   * défiler le document pour l'amener dans la fenêtre. Appelé au montage (il
   * répare une fenêtre déjà coincée) et après chaque saut. Le pourquoi complet
   * est dans `shellScroll.ts`, qui porte la fonction — elle était écrite ici
   * une première fois, avant qu'un second chemin ne produise le même
   * décalage. */
  const pinShellHere = () => pinShell(host);

  /** Saute à une section. L'ancre est l'`id` que MediaWiki a posé sur le titre
   * et que la reconstruction a conservé.
   *
   * **Jamais `scrollIntoView`** : il fait défiler *tous* les ancêtres
   * scrollables pour amener l'élément dans la **fenêtre**, et le titre visé
   * finissait sous la barre de titre (celle qui porte réduire/agrandir/Big
   * Picture). On fait donc défiler le conteneur, dont le haut est déjà sous
   * cette barre — le problème disparaît au lieu d'être compensé.
   *
   * **La division par `zoomFactor()` n'est pas décorative** : le zoom
   * d'interface est un `zoom` CSS posé sur `<html>`, donc
   * `getBoundingClientRect` rend des pixels de fenêtre déjà multipliés quand
   * `scrollTop` est en pixels CSS. Reporter l'un dans l'autre appliquerait le
   * facteur deux fois — même piège que le menu contextuel décalé et les listes
   * déroulantes hors écran (voir CLAUDE.md). */
  function goToSection(anchor: string) {
    if (!host || !anchor) return;
    const target = host.querySelector<HTMLElement>(`#${CSS.escape(anchor)}`);
    const scroller = scrollParent(host);
    if (!target || !scroller) return;
    const delta = (target.getBoundingClientRect().top - scroller.getBoundingClientRect().top) / zoomFactor();
    scroller.scrollTo({ top: scroller.scrollTop + delta - SCROLL_MARGIN, behavior: "smooth" });
    // Le défilement du focus, lui, a déjà eu lieu — en synchrone, avant ce
    // clic. On remet la coquille d'aplomb derrière.
    pinShellHere();
  }

  // Les langues où l'article existe vraiment, jamais une liste en dur : sur les
  // JDM, l'article japonais est souvent le plus complet (§5.4).
  const langs = $derived.by(() => {
    if (!article) return [];
    const all = [...new Set(article.availableLangs)];
    return all.sort((a, b) => (a === shown ? -1 : b === shown ? 1 : a.localeCompare(b)));
  });

  let searching = $state(false);
  let query = $state("");
  let results = $state<WikiSuggestion[] | null>(null);
  let busy = $state(false);

  /** Ouvre le panneau de recherche, pré-rempli avec ce qui a été cherché —
   * l'utilisateur corrige des mots-clés, il ne repart pas d'une case vide. */
  function openSearch() {
    query = panel?.query ?? "";
    results = null;
    searching = true;
  }

  async function runSearch() {
    if (!query.trim()) return;
    busy = true;
    results = await searchWikiCandidates(query);
    busy = false;
  }

  async function choose(entityId: string) {
    busy = true;
    await setWikiLink(modKey, entityId);
    searching = false;
    results = null;
    busy = false;
    onreload();
  }

  async function detach() {
    busy = true;
    await clearWikiLink(modKey);
    searching = false;
    results = null;
    busy = false;
    onreload();
  }

  /** La phrase qui dit ce qui s'est passé. Une phrase, pas un diagnostic : ce
   * qui compte pour l'utilisateur, c'est de savoir s'il peut agir. */
  const explanation = $derived.by(() => {
    switch (panel?.state) {
      case "ambiguous":
        return t("wiki.stateAmbiguous");
      case "noArticle":
        return t("wiki.stateNoArticle");
      case "offline":
        return t("wiki.stateOffline");
      case "unavailable":
        return t("wiki.stateUnavailable");
      default:
        return panel?.query ? t("wiki.stateNoneFound", { query: panel.query }) : t("wiki.stateNoneFoundPlain");
    }
  });
</script>

<div class="wiki">
  {#if !panel}
    <!-- Le seul état qui n'est pas un verdict : la recherche tourne encore.
         Le dire est tout l'intérêt d'un onglet permanent — sans ça, « je
         cherche » et « il n'y a rien » sont le même vide. -->
    <p class="muted">{t("wiki.searching")}</p>
  {:else if searching}
    <div class="search">
      <div class="row">
        <input
          class="input"
          bind:value={query}
          placeholder={t("wiki.searchPlaceholder")}
          onkeydown={(e) => e.key === "Enter" && runSearch()}
        />
        <button class="btn" type="button" onclick={runSearch} disabled={busy}>{t("wiki.searchAction")}</button>
        <button class="btn" type="button" onclick={() => (searching = false)}>{t("common.cancel")}</button>
      </div>

      {#if panel.entityId}
        <!-- « Aucun de ceux-ci » (§7.6). Il manquait ici : depuis un article
             affiché, on ne pouvait que **remplacer** l'appariement, jamais le
             retirer — alors qu'un mod sans article est une réponse valable
             (§1), et que c'est la seule façon de dire « celui-ci est faux et
             je n'en connais pas de bon ». -->
        <button class="btn link detach" type="button" onclick={detach} disabled={busy}>{t("wiki.detach")}</button>
      {/if}

      {#if busy}
        <p class="muted">{t("wiki.searching")}</p>
      {:else if results && results.length === 0}
        <p class="muted">{t("wiki.noResults")}</p>
      {:else if results}
        <!-- La description courte de Wikidata est ce qui lève l'ambiguïté d'un
             coup d'œil (§7.6) : « modèle d'automobile Toyota, 1983-1987 » en
             dit plus qu'un score. -->
        <ul class="results">
          {#each results as r (r.entityId)}
            <li>
              <button class="hit" type="button" onclick={() => choose(r.entityId)} disabled={busy}>
                <span class="hit-label">{r.label}</span>
                {#if r.description}<span class="hit-desc">{r.description}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {:else if article}
    {#if article.parentEntity}
      <!-- §7.3 : affichée UNIQUEMENT en cas de repli parent. L'utilisateur lit
           l'article du modèle générique et non celui de sa variante — le taire
           serait lui laisser croire qu'on parle de son mod. -->
      <p class="muted">{t("wiki.generalArticle", { title: article.articleTitle })}</p>
    {/if}

    <!-- L'article **entier**, tel que MediaWiki le rend : sections, tableaux,
         infobox, et les images dont la licence permet l'affichage (§9). Le mot
         « extrait » a quitté l'attribution avec l'introduction seule : ne
         montrer qu'un fragment était la modification qu'il fallait signaler. -->
    <!-- Deux colonnes, comme sur Wikipédia et pour la même raison : un sommaire
         posé en bande horizontale au-dessus d'un article de quarante sections
         déborde sur deux lignes et cesse d'être lisible. Il descend donc à
         gauche, et il tient tout seul pendant qu'on fait défiler. -->
    <div class="body">
      {#if article.sections.length > 1}
        <nav class="toc">
          <span class="toc-title">{t("wiki.contents")}</span>
          {#each article.sections as section (section.anchor + section.line)}
            {#if section.level <= 2}
              <!-- `preventDefault` sur `mousedown` : c'est ce qui empêche le
                   bouton de prendre le focus, donc le navigateur de faire
                   défiler la coquille pour le rendre visible. Le clic, lui,
                   part normalement — et le clavier garde son chemin, `Tab`
                   puis `Entrée` continuant de fonctionner. -->
              <button
                class="toc-item"
                class:sub={section.level === 2}
                type="button"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => goToSection(section.anchor)}>{section.line}</button
              >
            {/if}
          {/each}
        </nav>
      {/if}

      {#if article.html}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="extract article-html" bind:this={host} onclick={onArticleClick}></div>
      {:else}
        <!-- Repli : le rendu n'a pas pu être obtenu, le texte brut porte l'onglet. -->
        <div class="extract">
          {#each blocks as block, i (i)}
            {#if block.kind === "heading"}
              <p class="h" class:h3={block.level >= 3}>{block.text}</p>
            {:else}
              <p class="para">{block.text}</p>
            {/if}
          {/each}
        </div>
      {/if}
    </div>

    <div class="foot">
      {#if langs.length > 1}
        <label class="langs">
          <span class="sr-only">{t("wiki.languageLabel")}</span>
          <select
            class="input"
            value={shown ?? article.lang}
            onchange={(e) => onreload((e.currentTarget as HTMLSelectElement).value)}
          >
            {#each langs as code (code)}
              <option value={code}>{localeNames[code] ?? code.toUpperCase()}</option>
            {/each}
          </select>
        </label>
      {/if}
      <!-- Navigateur système, jamais une webview interne (§7.3) : elle casserait
           le mode hors ligne, imposerait leur CSP et ferait perdre l'identité
           visuelle de l'app. -->
      <button class="btn" type="button" onclick={() => openUrl(article.articleUrl).catch(() => {})}>
        {t("wiki.readFull")}
      </button>
      <button class="btn link" type="button" onclick={openSearch}>{t("wiki.wrongArticle")}</button>
    </div>

    {#if viewerIndex !== null && viewerImages.length > 0}
      <WikiImageViewer
        images={viewerImages}
        index={viewerIndex}
        onclose={() => (viewerIndex = null)}
        onnavigate={(i) => (viewerIndex = i)}
      />
    {/if}

    <p class="attribution">
      {t("wiki.attribution", { title: article.articleTitle })} ·
      <a
        href="https://creativecommons.org/licenses/by-sa/4.0/"
        onclick={(e) => {
          e.preventDefault();
          openUrl("https://creativecommons.org/licenses/by-sa/4.0/").catch(() => {});
        }}>{t("wiki.license")}</a
      >
    </p>
  {:else}
    <!-- L'état vide : une phrase et une porte, jamais une alerte (§1). -->
    <div class="empty">
      <p class="muted">{explanation}</p>
      <div class="foot">
        <button class="btn" type="button" onclick={openSearch}>{t("wiki.associate")}</button>
        {#if panel.entityId}
          <button class="btn link" type="button" onclick={detach}>{t("wiki.detach")}</button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* **L'article coule avec la page, il n'a pas son propre ascenseur.**
     La boîte de texte de la fiche n'impose aucune hauteur : un conteneur en
     `overflow: auto` posé dedans ne borne donc rien, il grandit — et on se
     retrouvait avec deux défilements qui se marchent dessus, dont un qui
     laissait une bande noire sous la fenêtre. Un seul ascenseur, celui de la
     fiche, comme pour tout le reste de l'écran. */
  /* **A bounded reading column, exactly like Wikipedia's.** Not a matter of
     taste: the column width is what decides where thumbnails land. On a wide
     window the prose reached 1400px, where a 280px figure weighs nothing, a
     paragraph fits in three lines, and figures — which queue up behind each
     other through `clear: right` — end up scattered along the text instead of
     forming a column of images. *That* was the defect we kept mistaking for a
     float problem: it vanishes as soon as the window is narrowed to the width
     of a Wikipedia page, which is how the user found it, after three fixes
     aimed at the floats that could not have helped.
     Bounded **and centred**: otherwise the article hugs the left edge and
     leaves a gap on the right that reads as broken. */
  .wiki {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    max-width: 950px;
    margin-inline: auto;
  }
  .muted {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .extract {
    flex: 1;
    min-width: 0;
    /* A single-section article has no table of contents: without this bound
       its prose would take the whole block width on its own, and the thumbnail
       placement defect would come back through that door. */
    max-width: 760px;
    line-height: 1.55;
    padding-right: 6px;
  }
  /* Le flottant de l'infobox ne doit pas déborder sur ce qui suit l'article. */
  .article-html::after {
    content: "";
    display: block;
    clear: both;
  }
  .para {
    margin: 0 0 10px;
    white-space: pre-wrap;
  }
  /* Titres de section : la hiérarchie de l'article, pas celle de l'écran —
     d'où une graduation discrète plutôt que les niveaux de `.lbl-*`. */
  .h {
    margin: 16px 0 6px;
    font-size: 13px;
    font-weight: 600;
  }
  .h.h3 {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .h:first-child {
    margin-top: 0;
  }

  /* Les deux colonnes. Le défilement appartient à l'article, pas à la page :
     le sommaire doit rester sous les yeux pendant qu'on descend. */
  .body {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    /* Centres the contents + article pair when the prose does not use all the
       available width (an article without a table of contents). */
    justify-content: center;
  }

  /* Collant : c'est ce qui remplace l'ascenseur propre au sommaire. Il suit
     la lecture au lieu de disparaître en haut de l'article. */
  .toc {
    flex: 0 0 170px;
    position: sticky;
    top: 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 70vh;
    overflow-y: auto;
    padding-right: 10px;
    border-right: 1px solid var(--line);
  }
  /* Un seuil de mise en page est une `@container`, pas une `@media` : la
     fenêtre ne dit rien de la largeur réellement disponible ici, le rail et la
     colonne de session en ayant déjà pris leur part (convention du projet).
     Sous cette largeur, l'article vaut mieux que son sommaire. */
  @container detail (max-width: 700px) {
    .toc {
      display: none;
    }
  }
  .toc-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin-bottom: 3px;
  }
  .toc-item {
    background: none;
    border: none;
    padding: 1px 0;
    font: inherit;
    font-size: 12px;
    color: inherit;
    cursor: pointer;
    text-align: left;
    line-height: 1.35;
  }
  .toc-item:hover {
    text-decoration: underline;
  }
  .toc-item.sub {
    font-size: 11px;
    color: var(--muted);
    padding-left: 10px;
  }

  /* --- L'article reconstruit -------------------------------------------
     Ces sélecteurs visent un arbre créé en JavaScript, donc invisible au
     compilateur Svelte : sans `:global`, le CSS scopé ne l'atteindrait pas.
     C'est ici qu'on reprend la main sur l'apparence — le balisage vient de
     MediaWiki, la mise en forme est celle de Pit Box. */
  .article-html :global(p) {
    margin: 0 0 10px;
  }
  .article-html :global(h2),
  .article-html :global(h3),
  .article-html :global(h4) {
    margin: 16px 0 6px;
    font-size: 13px;
    font-weight: 600;
  }
  .article-html :global(h3),
  .article-html :global(h4) {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .article-html :global(ul),
  .article-html :global(ol) {
    margin: 0 0 10px;
    padding-left: 18px;
  }
  .article-html :global(li) {
    margin-bottom: 3px;
  }
  .article-html :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    border-radius: 6px;
    margin: 10px 0 2px;
  }
  /* A photograph opens in the viewer; the cursor is the only thing that says
     so before the click. Icons keep the default cursor — they open nothing. */
  .article-html :global(img.wiki-photo) {
    cursor: zoom-in;
  }
  /* Les icones restent dans le fil du texte, a la taille que la page demande :
     cinq etoiles de notation forment une note, pas cinq affiches. */
  .article-html :global(img.wiki-icon) {
    display: inline-block;
    vertical-align: middle;
    border-radius: 0;
    margin: 0 1px;
  }
  /* Le crédit d'auteur : obligatoire sous chaque image (§9), donc jamais
     masqué — discret, mais présent. */
  .article-html :global(.wiki-credit) {
    display: block;
    font-size: 10px;
    color: var(--muted);
    margin-bottom: 10px;
    cursor: pointer;
  }
  /* **L'infobox flotte à droite**, comme sur Wikipédia — et c'est ce qui
     change tout : pleine largeur en tête d'article, elle repoussait le texte
     d'un écran entier avant qu'on en lise la première ligne. Largeur fixe
     plutôt que relative : c'est une fiche technique, ses libellés ne gagnent
     rien à s'étirer. */
  .article-html :global(.wiki-infobox) {
    float: right;
    width: 290px;
    max-width: 45%;
    margin: 0 0 12px 16px;
    font-size: 11px;
  }
  .article-html :global(.wiki-infobox caption) {
    font-size: 12px;
  }
  .article-html :global(.wiki-infobox img) {
    margin-top: 6px;
  }
  /* Sur un conteneur étroit, un flottant de 290 px ne laisse plus de place au
     texte : l'infobox reprend le fil normal. */
  @container detail (max-width: 620px) {
    .article-html :global(.wiki-infobox) {
      float: none;
      width: auto;
      max-width: 100%;
      margin-left: 0;
    }
  }
  .article-html :global(table) {
    border-collapse: collapse;
    margin: 0 0 12px;
    font-size: 12px;
    max-width: 100%;
  }
  .article-html :global(th),
  .article-html :global(td) {
    border: 1px solid var(--line);
    padding: 3px 6px;
    text-align: left;
    vertical-align: top;
  }
  .article-html :global(th) {
    background: rgba(127, 127, 127, 0.08);
    font-weight: 600;
  }
  .article-html :global(caption) {
    font-size: 12px;
    font-weight: 600;
    padding-bottom: 4px;
  }
  /* **Une colonne d'illustrations à droite, et le `clear` en est la clé.**
     Sans lui, trois vignettes consécutives se rangent côte à côte et écrasent
     le texte en une bande étroite — c'est ce qu'on voyait. Avec, chacune passe
     sous la précédente : la colonne de Wikipédia.

     Et seules les figures que l'article **marque** comme alignées flottent :
     `mw-halign-right` est écrit dans le HTML, le deviner pour toutes faisait
     flotter jusqu'à celles qui doivent rester dans le fil. */
  .article-html :global(figure),
  .article-html :global(.wiki-thumb) {
    float: right;
    clear: right;
    max-width: min(280px, 45%);
    margin: 4px 0 10px 16px;
  }
  .article-html :global(figure.wiki-left),
  .article-html :global(.wiki-thumb.wiki-left) {
    float: left;
    clear: left;
    margin: 4px 16px 10px 0;
  }
  /* Explicitement « dans le fil » : l'article le demande, on ne fait pas flotter. */
  .article-html :global(figure.wiki-center),
  .article-html :global(.wiki-thumb.wiki-center) {
    float: none;
    clear: both;
    max-width: 100%;
    margin: 10px 0;
  }
  .article-html :global(figcaption) {
    font-size: 11px;
    color: var(--muted);
  }
  /* Trop étroit pour un flottant : l'image reprend le fil, centrée. */
  @container detail (max-width: 620px) {
    .article-html :global(figure),
    .article-html :global(.wiki-thumb) {
      float: none;
      max-width: 100%;
      margin-left: 0;
      margin-right: 0;
    }
  }
  .article-html :global([data-href]) {
    color: var(--accent, inherit);
    cursor: pointer;
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 2px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .search {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .detach {
    align-self: flex-start;
  }
  .row .input {
    flex: 1;
  }
  .results {
    list-style: none;
    margin: 0;
    padding: 0;
    /* Le seul ascenseur restant du bloc, et il est borné : une liste de dix
       candidats ne doit pas allonger la page de la fiche. */
    max-height: 320px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .hit {
    width: 100%;
    text-align: left;
    background: none;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 5px 7px;
    cursor: pointer;
    color: inherit;
    font: inherit;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .hit:hover {
    border-color: var(--line);
    background: var(--hover, rgba(255, 255, 255, 0.04));
  }
  .hit-label {
    font-size: 13px;
  }
  .hit-desc {
    font-size: 11px;
    color: var(--muted);
  }
  /* Action secondaire : elle existe, elle ne se dispute pas l'attention. */
  .btn.link {
    background: none;
    border-color: transparent;
    color: var(--muted);
  }
  .attribution {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }
  .attribution a {
    color: inherit;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
</style>
