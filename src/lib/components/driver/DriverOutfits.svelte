<script lang="ts">
  // Tenues enregistrées et tenue par défaut, en pied du panneau d'essayage.
  //
  // Deux choses de nature différente, dans le même bloc parce qu'elles parlent
  // du même objet : une tenue nommée, et laquelle de ces tenues sert quand une
  // voiture n'a rien de choisi.
  //
  // Reposer une tenue complète, c'est **quatre choix d'un coup**, écrits en une
  // fois : poser le corps puis les pièces les effacerait, `setDriverBody`
  // remettant les trois autres au défaut (§D6).
  import { t } from "$lib/i18n/index.svelte";
  import { deleteOutfit, saveOutfit, savedOutfits, wornOutfit, type SavedOutfit } from "$lib/driverOutfits.svelte";
  import {
    applyFallback,
    driverFor,
    fallbackName,
    isEmpty,
    setApplyFallback,
    setDriverOutfit,
    setFallbackName,
  } from "$lib/driverOverride.svelte";

  let { carId }: { carId: string } = $props();

  const prefs = $derived(driverFor(carId || null));
  const outfits = $derived(savedOutfits());
  const currentlyWorn = $derived(wornOutfit(prefs));

  let naming = $state(false);
  let draft = $state("");
  let input = $state<HTMLInputElement | null>(null);

  /** Rien à enregistrer tant que tout vient de la voiture et de sa livrée. */
  const empty = $derived(isEmpty(prefs));

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
    if (!carId) return;
    setDriverOutfit(carId, {
      body: outfit.body,
      helmet: outfit.helmet,
      suit: outfit.suit,
      gloves: outfit.gloves,
    });
  }

  function worn(outfit: SavedOutfit): boolean {
    return currentlyWorn?.name === outfit.name;
  }

  function remove(name: string) {
    deleteOutfit(name);
    // Une tenue par défaut supprimée ne doit pas laisser l'option pointer dans
    // le vide : le nom se libère avec elle.
    if (fallbackName() === name) setFallbackName("");
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
          <button class="chip-x" type="button" title={t("driver.outfits.delete")} onclick={() => remove(outfit.name)}
            >×</button
          >
        </span>
      {/each}
    </div>

    <!-- La tenue par défaut : ce qui s'applique aux voitures pour lesquelles on
         n'a rien choisi. Absente tant qu'aucune tenue n'est enregistrée — il
         n'y aurait rien à désigner. -->
    <div class="fallback">
      <label class="row">
        <input
          type="checkbox"
          checked={applyFallback()}
          disabled={!fallbackName()}
          onchange={(e) => setApplyFallback(e.currentTarget.checked)}
        />
        <span>{t("driver.fallback.label")}</span>
      </label>
      <select class="input" value={fallbackName()} onchange={(e) => setFallbackName(e.currentTarget.value)}>
        <option value="">{t("driver.fallback.none")}</option>
        {#each outfits as outfit (outfit.name)}
          <option value={outfit.name}>{outfit.name}</option>
        {/each}
      </select>
      <p class="hint">{t("driver.fallback.hint")}</p>
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

  .fallback {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    cursor: pointer;
  }
  .fallback .input {
    height: 26px;
    font-size: 11.5px;
  }
  .hint,
  .none {
    margin: 0;
    font-size: 10.5px;
    color: var(--faint);
    line-height: 1.5;
  }
  .none {
    margin-top: 7px;
  }
</style>
