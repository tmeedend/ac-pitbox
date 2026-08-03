<script lang="ts">
  // Bloc « Tags » de la fiche détail (§5) : quatre origines distinctes,
  // color-codées, dont une seule est éditable.
  //
  //   rouge  catégorie (#…)   issue des règles, non éditable
  //   vert   règle            déduite par le moteur de tags, non éditable
  //   gris   manuel           ajouté à la main — le seul modifiable ici
  //   bleu   fichier mod      lu dans ui_car.json, lecture seule (règle d'or)
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

<div class="lbl">{t("detail.tagsLabel")}</div>
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

<style>
  /* Styles repris de la fiche : le CSS Svelte étant scopé par composant, un
     bloc extrait doit emporter les siens (voir l'en-tête de global.css). */
  .lbl {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1.5px;
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    text-transform: uppercase;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 8px;
  }
  .tag {
    font-size: 10px;
    padding: 2px 8px;
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
    gap: 4px;
  }
  .tag.mod {
    background: var(--blue-dim);
    color: var(--blue);
    border-color: var(--blue-border);
  }
  .tag .x {
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
    padding: 0;
  }
  .tag .x:hover {
    color: var(--rosso-bright);
  }
  .manual-input {
    width: 100%;
    padding: 5px 8px;
    font-size: 11px;
  }
</style>
