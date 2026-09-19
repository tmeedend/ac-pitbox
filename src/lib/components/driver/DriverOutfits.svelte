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
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { deleteOutfit, saveOutfit, savedOutfits, wornOutfit, type SavedOutfit } from "$lib/driver/driverOutfits.svelte";
  import type { DriverChoices, WardrobeOption } from "$lib/driver/driver";
  import { bodyThumb, requestBodyThumb } from "$lib/driver/driverThumbs.svelte";
  import { previewSrc } from "$lib/library/library";
  import {
    driverFor,
    fallbackName,
    isEmpty,
    setDriverOutfit,
    setFallbackName,
    type CarClass,
  } from "$lib/driver/driverOverride.svelte";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";

  let {
    carId,
    kind,
    skinId = null,
    choices = null,
  }: {
    carId: string;
    kind: CarClass;
    /** Livrée en place : elle habille le mannequin du rendu de vignette, donc
     * deux livrées ne donnent pas la même image du même corps. */
    skinId?: string | null;
    /** Les garde-robes de cette voiture — c'est là que vivent les vignettes
     * qu'Assetto Corsa range à côté de ses `.dds`. */
    choices?: DriverChoices | null;
  } = $props();

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

  /** La vignette d'une pièce de garde-robe, si cette voiture la propose. */
  function pieceThumb(list: WardrobeOption[] | undefined, id: string | null): string | null {
    if (!id || !list) return null;
    return previewSrc(list.find((w) => w.id === id)?.thumbnail ?? null);
  }

  /**
   * L'image d'une tenue enregistrée.
   *
   * **Le casque d'abord** : c'est la pièce qui distingue deux tenues au premier
   * regard, et sa vignette est un fichier qu'AC range déjà à côté de ses
   * textures. La combinaison puis les gants ensuite, même raisonnement.
   *
   * **Le corps en dernier**, et seulement faute de mieux : son image est un
   * rendu 3D du mannequin habillé par la LIVRÉE, pas par la tenue enregistrée
   * — deux tenues qui ne diffèrent que par le casque y seraient identiques.
   * Elle ne dit donc juste que pour une tenue qui n'est qu'une substitution de
   * mannequin, ce qui est exactement le cas où les trois autres manquent.
   *
   * `null` reste normal : une tenue dont les pièces viennent d'une autre
   * voiture n'a rien à montrer ici, et la case vide du sélecteur suffit.
   */
  function outfitThumb(o: SavedOutfit): string | null {
    return (
      pieceThumb(choices?.helmets, o.helmet) ??
      pieceThumb(choices?.suits, o.suit) ??
      pieceThumb(choices?.gloves, o.gloves) ??
      (o.body ? bodyThumb(carId + "|" + o.body) : null)
    );
  }

  // Le rendu d'un corps coûte une conversion la première fois, d'où la
  // discipline de `driverThumbs` : on ne demande que ce qui va s'afficher.
  // Ici, seulement les tenues dont aucune pièce n'a d'image — une poignée au
  // plus, là où demander les douze coûterait douze conversions pour rien.
  // Toutes les dépendances se lisent avant la première sortie : une garde en
  // tête tronquerait la liste des abonnements dès le montage.
  $effect(() => {
    const car = carId;
    const skin = skinId;
    const list = outfits;
    const wardrobe = choices;
    if (!car) return;
    // `untrack` : `requestBodyThumb` LIT le cache des vignettes pour savoir si
    // la demande est déjà faite. Sans lui, cet effet s'abonnerait à ce cache
    // et se redéclencherait à chaque vignette qui tombe, y compris celles des
    // autres écrans. Demander n'est pas une dépendance.
    untrack(() => {
      for (const o of list) {
        if (!o.body) continue;
        const dressed =
          pieceThumb(wardrobe?.helmets, o.helmet) ??
          pieceThumb(wardrobe?.suits, o.suit) ??
          pieceThumb(wardrobe?.gloves, o.gloves);
        if (!dressed) requestBodyThumb(car, skin, o.body);
      }
    });
  });

  /** Les entrées du sélecteur : « Aucune » d'abord, puis les tenues, chacune
   * avec sa vignette — même composant que le sélecteur de livrée de la
   * colonne de session : sa liste s'ouvre en `position: fixed` et prend la
   * largeur de son plus long libellé, là où un `<select>` contraint à 170 px
   * dans ce panneau coupait les noms. */
  const fallbackOptions = $derived([
    { id: "", name: t("driver.fallback.none"), image: null },
    ...outfits.map((o) => ({ id: o.name, name: o.name, image: outfitThumb(o) })),
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
        {@const thumb = outfitThumb(outfit)}
        <span class="chip" class:on={worn(outfit)}>
          <!-- La même image que dans le sélecteur juste en dessous : ce sont
               les mêmes tenues, elles doivent se reconnaître d'une liste à
               l'autre. La case est posée même vide, sans quoi les pastilles
               n'auraient pas toutes la même hauteur. -->
          <button class="chip-name" type="button" onclick={() => apply(outfit)}>
            <span class="chip-thumb">{#if thumb}<img src={thumb} alt="" />{/if}</span>
            {outfit.name}
          </button>
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
  .chip-name {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-left: 5px;
  }
  /* 18 px : la pastille fait 24 px de haut, l'image en prend ce qu'elle peut
     sans la faire grandir — une rangée de pastilles plus hautes coûterait de
     la hauteur à un panneau qui n'en a pas de reste. */
  .chip-thumb {
    flex: none;
    width: 18px;
    height: 18px;
    background: var(--raised);
    overflow: hidden;
  }
  .chip-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  /* Sur la pastille en place, le fond rouge remplace le gris de la case vide :
     un carré gris s'y lirait comme une image qui n'a pas chargé. */
  .chip.on .chip-thumb {
    background: rgba(255, 255, 255, 0.18);
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
