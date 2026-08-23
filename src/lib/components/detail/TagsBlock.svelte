<script lang="ts">
  // Bloc « Tags » de la fiche détail (§5) : quatre origines distinctes,
  // color-codées, dont une seule est éditable.
  //
  //   rouge  catégorie (#…)   issue des règles, non éditable
  //   vert   règle            déduite par le moteur de tags, non éditable
  //   gris   manuel           ajouté à la main — le seul modifiable ici
  //   bleu   fichier mod      lu dans ui_car.json, lecture seule (règle d'or)
  //
  // Le bleu couvre tout ce qu'aucune règle n'a reconnu, pas seulement les tags
  // recopiés tels quels : depuis que le vocabulaire est une vraie liste blanche
  // (§5), une catégorie qu'aucune règle ne déclare n'est plus promue en rouge —
  // elle retombe ici, avec le reste du brut, et se masque avec lui.
  //
  // Le code couleur est rappelé en légende au pied du bloc : sans elle, il
  // faut connaître le §5 pour comprendre pourquoi certains tags ont une croix
  // et d'autres non.
  //
  // La saisie est locale au composant : c'est de l'état d'interface, il n'a
  // aucune raison de vivre dans la page. La persistance, elle, remonte au
  // parent — lui seul sait relire la fiche et prévenir la bibliothèque.
  import { onMount } from "svelte";
  import type { ModDetail } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";

  let {
    detail,
    onaddtag,
    onremovetag,
  }: {
    detail: ModDetail;
    onaddtag: (tag: string) => void;
    onremovetag: (tag: string) => void;
  } = $props();

  let input = $state("");

  // Défaut synchrone, le temps que la préférence sauvegardée réponde (§6.2).
  // Même clé que le panneau latéral : c'est un seul réglage, pas deux.
  let showRawTags = $state(true);

  onMount(async () => {
    const saved = await getUiPref(StorageKey.showFileTags);
    if (saved != null) showRawTags = saved !== "false";
  });

  function toggleRawTags() {
    showRawTags = !showRawTags;
    setUiPref(StorageKey.showFileTags, String(showRawTags));
  }

  // Les catégories (#) passent devant : ce sont les tags structurants (§5bis).
  const categories = $derived(detail.tags_from_rule.filter((tag) => tag.startsWith("#")));
  const fromRules = $derived(detail.tags_from_rule.filter((tag) => !tag.startsWith("#")));

  function submit() {
    // Normalisé en minuscules ici comme à l'import : sinon « GT3 » et « gt3 »
    // cohabiteraient dans la même liste.
    const tag = input.trim().toLowerCase();
    input = "";
    if (tag) onaddtag(tag);
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.tagsLabel")}</span>
  </header>
  <div class="blk-b">
    <div class="tags">
      {#each categories as tag}<span class="tag cat">{tag}</span>{/each}
      {#each fromRules as tag}<span class="tag rule">{tag}</span>{/each}
      {#each detail.tags_manual as tag}
        <span class="tag manual">
          {tag}<button class="x" type="button" onclick={() => onremovetag(tag)} title={t("common.remove")}>×</button>
        </span>
      {/each}
      {#if showRawTags}
        {#each detail.tags_from_mod as tag}<span class="tag mod">{tag}</span>{/each}
      {/if}
    </div>

    <div class="tag-actions">
      <input
        class="input manual-input"
        placeholder={t("detail.addTagPlaceholder")}
        bind:value={input}
        onkeydown={(e) => e.key === "Enter" && submit()}
      />
      <label class="raw-toggle">
        <input type="checkbox" checked={showRawTags} onchange={toggleRawTags} />
        <span>{t("detail.rawModTags")}</span>
      </label>
    </div>

    <div class="legend">
      <span class="lg cat">{t("detail.tagLegendCategory")}</span>
      <span class="lg rule">{t("detail.tagLegendRule")}</span>
      <span class="lg manual">{t("detail.tagLegendManual")}</span>
      <span class="lg mod">{t("detail.tagLegendMod")}</span>
    </div>
  </div>
</section>

<style>
  /* Habillage propre au bloc. L'encadré, le bandeau et la marge intérieure
     viennent des classes globales `.blk*` (voir global.css). */
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }
  .tag {
    font-size: 11px;
    padding: 3px 10px;
    font-family: var(--mono);
    border: 1px solid var(--line);
  }
  .tag.cat {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .tag.rule {
    background: var(--green-dim);
    color: var(--green);
    border-color: var(--green-border);
  }
  .tag.manual {
    background: var(--raised);
    color: var(--txt2);
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .tag.mod {
    background: var(--blue-dim);
    color: var(--blue);
    border-color: var(--blue-border);
  }
  .tag .x {
    background: transparent;
    color: var(--muted);
    font-size: 13px;
    line-height: 1;
    padding: 0;
  }
  .tag .x:hover {
    color: var(--rosso-bright);
  }
  .tag-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .manual-input {
    flex: 1;
    min-width: 0;
    padding: 9px 11px;
    font-size: 12px;
  }
  .raw-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--txt2);
    white-space: nowrap;
    cursor: pointer;
  }
  /* Légende du code couleur : même teintes que les puces, sans leur fond —
     elle informe, elle ne doit pas peser autant qu'un tag réel. */
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 12px;
    font-family: var(--mono);
    font-size: 11px;
  }
  .lg.cat {
    color: var(--rosso-bright);
  }
  .lg.rule {
    color: var(--green);
  }
  .lg.manual {
    color: var(--txt2);
  }
  .lg.mod {
    color: var(--blue);
  }
</style>
