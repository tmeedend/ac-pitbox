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
  import {
    articleLang,
    clearWikiLink,
    searchWikiCandidates,
    setWikiLink,
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
    el.scrollTop = 0;
  });

  /** Les liens de l'article ouvrent le navigateur système — jamais une
   * navigation interne, qui ferait sortir l'utilisateur de sa fiche. Les `href`
   * ont été remplacés par des `data-href` validés à la reconstruction. */
  function onArticleClick(event: MouseEvent) {
    const target = (event.target as HTMLElement | null)?.closest("[data-href]");
    const href = target?.getAttribute("data-href");
    if (!href) return;
    event.preventDefault();
    openUrl(href).catch(() => {});
  }

  /** Saute à une section. L'ancre est l'`id` que MediaWiki a posé sur le titre
   * et que la reconstruction a conservé. */
  function goToSection(anchor: string) {
    if (!host || !anchor) return;
    const target = host.querySelector(`#${CSS.escape(anchor)}`);
    target?.scrollIntoView({ block: "start", behavior: "smooth" });
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
    {#if article.sections.length > 1}
      <nav class="toc">
        <span class="toc-title">{t("wiki.contents")}</span>
        {#each article.sections as section (section.anchor + section.line)}
          {#if section.level <= 2}
            <button
              class="toc-item"
              class:sub={section.level === 2}
              type="button"
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
  .wiki {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
  }
  .muted {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .extract {
    flex: 1;
    overflow-y: auto;
    line-height: 1.55;
    /* Largeur de mesure : un article entier se lit, il ne se balaie pas. */
    padding-right: 6px;
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

  /* Sommaire : une bande discrète au-dessus de l'article, pas un panneau. Sur
     dix-sept mille caractères, c'est ce qui manquait le plus. */
  .toc {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }
  .toc-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .toc-item {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 12px;
    color: inherit;
    cursor: pointer;
    text-align: left;
  }
  .toc-item:hover {
    text-decoration: underline;
  }
  .toc-item.sub {
    font-size: 11px;
    color: var(--muted);
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
  /* Le crédit d'auteur : obligatoire sous chaque image (§9), donc jamais
     masqué — discret, mais présent. */
  .article-html :global(.wiki-credit) {
    display: block;
    font-size: 10px;
    color: var(--muted);
    margin-bottom: 10px;
    cursor: pointer;
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
  .article-html :global(figure) {
    margin: 10px 0;
  }
  .article-html :global(figcaption) {
    font-size: 11px;
    color: var(--muted);
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
    min-height: 0;
    flex: 1;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row .input {
    flex: 1;
  }
  .results {
    list-style: none;
    margin: 0;
    padding: 0;
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
