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
  import { deleteOutfit, saveOutfit, savedOutfits, wornOutfit, type SavedOutfit } from "$lib/driver/driverOutfits.svelte";
  import {
    driverFor,
    fallbackName,
    isEmpty,
    setDriverOutfit,
    setFallbackName,
    type CarClass,
  } from "$lib/driver/driverOverride.svelte";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";

  let { carId, kind }: { carId: string; kind: CarClass } = $props();

  const prefs = $derived(driverFor(carId || null, kind));
  const outfits = $derived(savedOutfits());
  const currentlyWorn = $derived(wornOutfit(prefs));

  /** Les deux classes, dans l'ordre d'affichage. */
  const FALLBACKS: CarClass[] = ["street", "race"];

  let naming = $state(false);
  let draft = $state("");
  let confirmName = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  /** Rien à enregistrer tant que tout vient de la voiture et de sa livrée. */
  const empty = $derived(isEmpty(prefs));

  function open() {
    naming = true;
    draft = "";
    confirmName = null;
    queueMicrotask(() => input?.focus());
  }

  function cancelNaming() {
    naming = false;
    confirmName = null;
  }

  /**
   * Enregistre — ou demande d'abord, si le nom est déjà pris.
   *
   * `saveOutfit` remplace **insensible à la casse**, donc enregistrer « gt »
   * quand « GT » existe effaçait l'ancienne sans un mot. Les deux autres listes
   * nommées de l'app posent la question depuis toujours (`NamedListDialog`) ;
   * c'était la seule des trois à ne pas la poser, et rien ne justifiait
   * l'écart. La comparaison se fait donc dans la casse du **contrat**, pas
   * dans celle de l'affichage : comparer exactement laisserait passer le seul
   * cas dangereux.
   */
  function submit() {
    const name = draft.trim();
    if (!name) {
      cancelNaming();
      return;
    }
    // Même marche que `NamedListDialog` : la confirmation porte sur CE nom-là,
    // si bien que rééditer le texte repose la question au lieu de valider un
    // accord donné pour un autre nom.
    if (outfits.some((o) => o.name.toLowerCase() === name.toLowerCase()) && confirmName !== name) {
      confirmName = name;
      return;
    }
    commit(name);
  }

  function commit(name: string) {
    saveOutfit({ name, body: prefs.body, helmet: prefs.helmet, suit: prefs.suit, gloves: prefs.gloves });
    cancelNaming();
  }

  /**
   * Le champ perd le focus : on valide, **sauf si la question est à l'écran**.
   *
   * Sans cette sortie, cliquer « Annuler » enregistrerait quand même — le blur
   * du champ part avant le clic, et il trouverait `confirmName` déjà égal au
   * nom saisi, donc l'accord déjà donné. La question posée, c'est aux deux
   * boutons d'y répondre.
   */
  function blurred() {
    if (confirmName) return;
    submit();
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
        oninput={() => (confirmName = null)}
        onkeydown={(e) => {
          if (e.key === "Enter") submit();
          if (e.key === "Escape") cancelNaming();
        }}
        onblur={blurred}
      />
    {:else}
      <button class="btn add" type="button" disabled={empty} title={t("driver.outfits.saveHint")} onclick={open}>
        {t("driver.outfits.save")}
      </button>
    {/if}
  </div>

  <!-- La question d'écrasement, posée sur place plutôt que dans une fenêtre :
       les puces juste en dessous montrent déjà la tenue qu'on remplace. Même
       formulation et mêmes mots que les sessions et les grilles — c'est la
       même question, elle ne gagne rien à être posée autrement ici. -->
  {#if confirmName}
    <div class="confirm-overwrite">
      <span>{t("common.overwriteConfirm", { name: confirmName })}</span>
      <button class="btn btn-primary" type="button" onclick={() => commit(confirmName ?? "")}>
        {t("common.overwriteConfirmBtn")}
      </button>
      <button class="btn" type="button" onclick={cancelNaming}>{t("common.cancel")}</button>
    </div>
  {/if}

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
  /* Mêmes jetons que la même question dans `NamedListDialog` — le CSS Svelte
     étant scopé, la ressemblance se réécrit, elle ne s'hérite pas. Seule la
     géométrie change : pas de gouttière latérale, on est déjà dans le panneau.
     Les boutons peuvent passer à la ligne, la colonne étant étroite. */
  .confirm-overwrite {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
  .confirm-overwrite button {
    padding: 3px 10px;
    font-size: 11px;
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
