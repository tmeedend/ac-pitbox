<script lang="ts">
  // Bloc « Tags » de la fiche détail (§5) : quatre origines distinctes,
  // color-codées, dont une seule est éditable.
  //
  //   rouge  catégorie (#…)   issue des règles, non éditable
  //   vert   règle            déduite par le moteur de tags, non éditable
  //   gris   manuel           ajouté à la main — le seul modifiable ici
  //   bleu   fichier mod      lu dans ui_car.json, lecture seule (règle d'or)
  //
  // Le code couleur est rappelé en légende au pied du bloc : sans elle, il
  // faut connaître le §5 pour comprendre pourquoi certains tags ont une croix
  // et d'autres non.
  //
  // La saisie est locale au composant : c'est de l'état d'interface, il n'a
  // aucune raison de vivre dans la page. La persistance, elle, remonte au
  // parent — lui seul sait relire la fiche et prévenir la bibliothèque.
  import type { ModDetail } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";

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
      {#each detail.tags_from_mod as tag}<span class="tag mod">{tag}</span>{/each}
    </div>

    <input
      class="input manual-input"
      placeholder={t("detail.addTagPlaceholder")}
      bind:value={input}
      onkeydown={(e) => e.key === "Enter" && submit()}
    />

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
  .manual-input {
    width: 100%;
    padding: 9px 11px;
    font-size: 12px;
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
