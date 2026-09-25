<script lang="ts">
  // Les quatre chiffres d'un circuit, dans une seule carte (maquette
  // écran 7). Le **nom** du tracé n'y est pas : le sélecteur le dit
  // déjà, à quelques pixels au-dessus, et c'est lui qui le change.
  // L'odomètre y entre comme il entre dans la fiche technique d'une
  // voiture : la carte « Distance » qui le portait à part n'existait
  // que parce qu'un circuit n'avait aucune carte de données où le
  // mettre. Il en a une.
  import type { LayoutItem, ModDetail } from "$lib/library/library";
  import { countryLabel, flagFor, loadFlags } from "$lib/flags.svelte";
  import { odometerText } from "$lib/detail/odometer";
  import { trackLength } from "$lib/detail/trackLength";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    detail: ModDetail;
    /** The selected layout, whose length is shown. */
    layout: LayoutItem | undefined;
    /** How many layouts come from an active layer (REFONTE§7.7). */
    addedLayouts: number;
  }
  let { detail, layout, addedLayouts }: Props = $props();

  const layoutCount = $derived(detail.track?.layouts.length ?? 0);

  // Table des drapeaux : la fiche d'un circuit montre son pays sans passer par
  // `TechSheet`, qui la charge de son côté. Idempotent, donc chaque composant
  // qui montre un drapeau la demande plutôt que de compter sur un voisin.
  $effect(() => {
    void loadFlags();
  });
</script>

<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("detail.trackInfo")}</span></header>
  <div class="specgrid" style="grid-template-columns:1fr 1fr;">
    <div>
      <div class="k lbl-key">{t("detail.lengthLabel")}</div>
      <div class="v">{trackLength(layout?.length) ?? "—"}</div>
    </div>
    <div>
      <div class="k lbl-key">{t("detail.layoutsLabel")}</div>
      <div class="v">
        {layoutCount}
        {#if addedLayouts}<span class="v-sub">{t("detail.layoutsAdded", { count: addedLayouts })}</span>{/if}
      </div>
    </div>
    <!-- Même pastille que la fiche technique d'une voiture : un circuit
         déclare son pays comme elle, et se reconnaît de même. -->
    <div>
      <div class="k lbl-key">{t("columns.country")}</div>
      <div class="v">
        {#if detail.country}{@const flag = flagFor(detail.country)}
          {#if flag}<img class="flag" src={flag} alt="" />{/if}{countryLabel(detail.country)}
        {:else}—{/if}
      </div>
    </div>
    <div><div class="k lbl-key">{t("detail.odometer")}</div><div class="v">{odometerText(detail)}</div></div>
  </div>
</section>

<style>
  .specgrid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    background: var(--line);
    gap: 1px;
  }
  .specgrid > div {
    background: var(--panel2);
    padding: 7px 10px;
  }
  .specgrid .k {
    margin-bottom: 3px;
  }
  /* Complément d'une valeur, dans la même cellule : « 2 » puis « dont 1
     ajouté ». Un second champ l'aurait séparé du chiffre qu'il qualifie. */
  .specgrid .v-sub {
    color: var(--muted);
    font-size: 10px;
    margin-left: 5px;
  }
  .specgrid .v {
    color: var(--txt2);
    font-size: 11px;
    font-family: var(--mono);
  }
  /* Mêmes valeurs que la fiche technique, les filtres et le plateau : une
     seule taille de drapeau dans l'app, et un filet parce que beaucoup ont du
     blanc sur un bord. Le CSS des composants étant scopé, la ressemblance se
     réécrit, elle ne s'hérite pas. */
  .specgrid .flag {
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
    vertical-align: -1px;
    margin-right: 5px;
  }
</style>
