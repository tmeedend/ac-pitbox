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
  // traduction — et l'attribution en pied est obligatoire, le mot « extrait »
  // compris : n'en montrer qu'un fragment est une modification, qui doit être
  // signalée.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { localeNames } from "$lib/i18n/index.svelte";
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

    <!-- `pre-wrap` : l'API rend les paragraphes en sauts de ligne, et les
         préserver est déjà une façon de ne pas retoucher le texte. -->
    <p class="extract">{article.extract}</p>

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
    margin: 0;
    flex: 1;
    overflow-y: auto;
    white-space: pre-wrap;
    line-height: 1.55;
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
