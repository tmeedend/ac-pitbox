<script lang="ts">
  // Tenues enregistrées, en pied du panneau d'essayage.
  //
  // Reposer une tenue complète, c'est **quatre choix d'un coup** — d'où
  // l'ordre d'application ci-dessous, qui n'est pas indifférent : le corps
  // commande, et le poser fait tomber les trois autres (§D6). Il faut donc
  // le poser en premier, puis les pièces, jamais l'inverse.
  import { t } from "$lib/i18n/index.svelte";
  import { deleteOutfit, saveOutfit, savedOutfits, wornOutfit, type SavedOutfit } from "$lib/driverOutfits.svelte";
  import { driverOverride, setDriverBody, setDriverPiece } from "$lib/driverOverride.svelte";

  const prefs = $derived(driverOverride());
  const outfits = $derived(savedOutfits());

  let naming = $state(false);
  let draft = $state("");
  let input = $state<HTMLInputElement | null>(null);

  /** Rien à enregistrer tant que tout vient de la voiture et de sa livrée. */
  const empty = $derived(!prefs.body && !prefs.helmet && !prefs.suit && !prefs.gloves);

  function open() {
    naming = true;
    draft = "";
    queueMicrotask(() => input?.focus());
  }

  function confirm() {
    const name = draft.trim();
    if (name) {
      saveOutfit({ name, body: prefs.body, helmet: prefs.helmet, suit: prefs.suit, gloves: prefs.gloves });
    }
    naming = false;
  }

  function apply(outfit: SavedOutfit) {
    // Le corps d'abord : `setDriverBody` remet les trois pièces au défaut,
    // donc les poser avant reviendrait à les effacer aussitôt.
    setDriverBody(outfit.body);
    setDriverPiece("helmet", outfit.helmet);
    setDriverPiece("suit", outfit.suit);
    setDriverPiece("gloves", outfit.gloves);
  }

  /** Une tenue est « portée » quand ses quatre pièces sont celles en place. */
  const currentlyWorn = $derived(wornOutfit(prefs));
  function worn(outfit: SavedOutfit): boolean {
    return currentlyWorn?.name === outfit.name;
  }
</script>

<div class="outfits">
  <div class="head">
    <span class="lbl-key mono k">{t("driver.outfits.title")}</span>
    {#if naming}
      <input
        class="input name"
        bind:this={input}
        bind:value={draft}
        placeholder={t("driver.outfits.placeholder")}
        maxlength="28"
        onkeydown={(e) => {
          if (e.key === "Enter") confirm();
          if (e.key === "Escape") naming = false;
        }}
        onblur={confirm}
      />
    {:else}
      <button class="btn add" type="button" disabled={empty} title={t("driver.outfits.saveHint")} onclick={open}>
        {t("driver.outfits.save")}
      </button>
    {/if}
  </div>

  {#if outfits.length}
    <div class="chips">
      {#each outfits as outfit (outfit.name)}
        <span class="chip" class:on={worn(outfit)}>
          <button class="chip-name" type="button" onclick={() => apply(outfit)}>{outfit.name}</button>
          <button class="chip-x" type="button" title={t("driver.outfits.delete")} onclick={() => deleteOutfit(outfit.name)}>×</button>
        </span>
      {/each}
    </div>
  {:else if !naming}
    <p class="none">{t("driver.outfits.none")}</p>
  {/if}
</div>

<style>
  .outfits {
    margin-top: 12px;
    padding-top: 11px;
    border-top: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* Couleur, taille et interlettrage viennent de `.lbl-key` : ne restent ici
     que les majuscules. */
  .k {
    text-transform: uppercase;
  }
  /* Désactivé et non masqué : quand rien n'est choisi il n'y a rien à
     enregistrer, et l'infobulle le dit — un bouton disparu laisse chercher. */
  .add {
    margin-left: auto;
    padding: 4px 10px;
    font-size: 11px;
  }
  .name {
    margin-left: auto;
    height: 26px;
    max-width: 170px;
    font-size: 11.5px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 8px;
  }
  /* Même vocabulaire que les groupes segmentés de la barre d'outils : fond de
     carte au repos, rouge plein pour ce qui est en place. */
  .chip {
    display: flex;
    align-items: center;
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  .chip.on {
    background: var(--rosso);
    border-color: var(--rosso);
  }
  .chip-name,
  .chip-x {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    padding: 4px 8px;
  }
  .chip.on .chip-name,
  .chip.on .chip-x {
    color: #fff;
  }
  .chip-name:hover {
    color: var(--txt);
  }
  .chip-x {
    padding-left: 0;
    color: var(--faint);
    font-size: 13px;
    line-height: 1;
  }
  .chip-x:hover {
    color: var(--rosso-bright);
  }
  .none {
    margin: 7px 0 0;
    font-size: 10.5px;
    color: var(--faint);
    line-height: 1.5;
  }
</style>
