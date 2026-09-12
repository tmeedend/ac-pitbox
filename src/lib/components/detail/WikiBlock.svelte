<script lang="ts">
  // L'onglet Wikipédia de la fiche (docs/SPEC-wikipedia-fiche-detail.md §7.3).
  //
  // **Contrainte juridique, pas préférence de mise en page** (§2) : le texte de
  // Wikipédia est sous CC BY-SA, et l'affichage doit rester une *collection* —
  // des œuvres juxtaposées et identifiables — et non une œuvre dérivée, sinon
  // le ShareAlike remonte sur l'application. D'où trois règles qui ne se
  // négocient pas ici :
  //
  // - l'extrait n'est **jamais** fusionné avec la description du mod : ce sont
  //   deux sous-onglets, donc deux blocs qui ne se touchent pas ;
  // - il est affiché **tel que l'API l'a rendu** — pas de reformulation, pas de
  //   résumé, pas de traduction, et surtout rien qui passe par un modèle ;
  // - l'attribution en pied est **obligatoire**, et le mot « extrait » y est
  //   nécessaire : n'afficher qu'un fragment est une modification, qui doit
  //   être signalée.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { localeNames } from "$lib/i18n/index.svelte";
  import { articleLang, type WikiArticle } from "$lib/wiki";

  interface Props {
    article: WikiArticle;
    /** Changement de langue de lecture : le parent recharge (§5.4). */
    onlang: (lang: string) => void;
  }
  const { article, onlang }: Props = $props();

  const shown = $derived(articleLang(article));

  // Les langues où l'article existe vraiment, jamais une liste en dur : sur les
  // JDM, l'article japonais est souvent le plus complet (§5.4). Triées en
  // mettant celle affichée en tête, pour que le sélecteur dise d'abord où on
  // est.
  const langs = $derived.by(() => {
    const all = [...new Set(article.availableLangs)];
    return all.sort((a, b) => (a === shown ? -1 : b === shown ? 1 : a.localeCompare(b)));
  });

  function labelOf(code: string): string {
    return localeNames[code] ?? code.toUpperCase();
  }
</script>

<div class="wiki">
  {#if article.parentEntity}
    <!-- §7.3 : affichée UNIQUEMENT en cas de repli parent. L'utilisateur lit
         l'article du modèle générique et non celui de sa variante — le taire
         serait lui laisser croire qu'on parle de son mod. -->
    <p class="general">{t("wiki.generalArticle", { title: article.articleTitle })}</p>
  {/if}

  <!-- `white-space: pre-wrap` : l'API rend les paragraphes en sauts de ligne,
       et les préserver est déjà une façon de ne pas retoucher le texte. -->
  <p class="extract">{article.extract}</p>

  <div class="foot">
    {#if langs.length > 1}
      <label class="langs">
        <span class="sr-only">{t("wiki.languageLabel")}</span>
        <select
          class="input"
          value={shown ?? article.lang}
          onchange={(e) => onlang((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each langs as code (code)}
            <option value={code}>{labelOf(code)}</option>
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
  </div>

  <p class="attribution">
    {t("wiki.attribution", { title: article.articleTitle })} ·
    <a href="https://creativecommons.org/licenses/by-sa/4.0/deed.fr" onclick={(e) => {
      e.preventDefault();
      openUrl("https://creativecommons.org/licenses/by-sa/4.0/deed.fr").catch(() => {});
    }}>{t("wiki.license")}</a>
  </p>
</div>

<style>
  .wiki {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
  }
  /* Discrète par construction : c'est une précision, pas un avertissement. */
  .general {
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
  }
  .langs {
    display: inline-flex;
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
