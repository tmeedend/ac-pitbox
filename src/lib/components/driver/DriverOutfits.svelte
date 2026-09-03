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
    driverFor,
    fallbackName,
    isEmpty,
    setDriverOutfit,
    setFallbackName,
    type CarClass,
  } from "$lib/driverOverride.svelte";
  import ImageSelectDropdown from "../ImageSelectDropdown.svelte";

  let { carId, kind }: { carId: string; kind: CarClass } = $props();

  const prefs = $derived(driverFor(carId || null, kind));
  const outfits = $derived(savedOutfits());
  const currentlyWorn = $derived(wornOutfit(prefs));

  /** Les deux classes, dans l'ordre d'affichage. */
  const FALLBACKS: CarClass[] = ["street", "race"];

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

  /** Les entrées du sélecteur : « Aucune » d'abord, puis les tenues. Sans
   * image — ce sont des noms, pas des livrées — mais avec le même composant
   * que le sélecteur de livrée de la colonne de session : sa liste s'ouvre en
   * `position: fixed` et prend la largeur de son plus long libellé, là où un
   * `<select>` contraint à 170 px dans ce panneau coupait les noms. */
  const fallbackOptions = $derived([
    { id: "", name: t("driver.fallback.none"), image: null },
    ...outfits.map((o) => ({ id: o.name, name: o.name, image: null })),
  ]);

  function remove(name: string) {
    deleteOutfit(name);
    // Une tenue par défaut supprimée ne doit pas laisser l'option pointer dans
    // le vide : le nom se libère avec elle.
    for (const k of FALLBACKS) {
      if (fallbackName(k) === name) setFallbackName(k, "");
    }
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

    <!-- Les tenues par défaut : celles qui habillent les voitures pour
         lesquelles on n'a rien choisi. **Un contrôle par classe** — sur une
         voiture de course la tenue fait partie de la livrée, et beaucoup
         voudront la lui laisser tout en s'habillant sur une voiture de rue.
         Désigner une tenue l'active, « Aucune » la désactive. Absentes tant
         qu'aucune tenue n'est enregistrée : il n'y aurait rien à désigner. -->
    <div class="fallback">
      {#each FALLBACKS as k (k)}
        <div class="fb-row" class:here={k === kind}>
          <span class="lbl-key mono k">{t("driver.fallback.label." + k)}</span>
          <ImageSelectDropdown
            options={fallbackOptions}
            selectedId={fallbackName(k)}
            placeholder={t("driver.fallback.none")}
            emptyText={t("driver.fallback.none")}
            onselect={(id) => setFallbackName(k, id)}
          />
        </div>
      {/each}
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
    gap: 9px;
  }
  .fb-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* Celle qui s'applique à la voiture courante, en clair : sans ce repère,
     deux champs identiques laissent chercher lequel agit ici et maintenant. */
  .fb-row.here .k {
    color: var(--txt2);
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
