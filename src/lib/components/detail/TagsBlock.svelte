<script lang="ts">
  // Bloc « Tags » de la fiche détail (§5) : quatre origines distinctes,
  // color-codées, dont une seule est éditable.
  //
  //   rouge  catégorie (#…)   issue des règles — ou choisie par l'utilisateur
  //                           (REGLES§3 niveau 4) : la puce porte alors le fanion
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
  import type { ModDetail } from "$lib/library/library";
  import { t } from "$lib/i18n/index.svelte";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import ManualMark from "$lib/components/ui/ManualMark.svelte";
  import {
    listCategoryChoices,
    matchCategories,
    type CategoryOp,
  } from "$lib/library/categoryDecision";
  import CategoryPicker from "./CategoryPicker.svelte";

  let {
    detail,
    onaddtag,
    onremovetag,
    oncategory,
  }: {
    detail: ModDetail;
    onaddtag: (tag: string) => void;
    onremovetag: (tag: string) => void;
    /** A gesture on the category: force it (car), add or take one off (track),
     * go back to the computed one. */
    oncategory: (op: CategoryOp) => void;
  } = $props();

  let input = $state("");
  const isCar = $derived(detail.kind === "Car");

  // The tag field offers the categories its text points at, above the plain
  // tag it has always made. Read at the first focus: a field one never touches
  // costs nothing.
  let choices = $state<string[]>([]);
  let focused = $state(false);
  let dismissed = $state(false);
  /** The highlighted line: a category, or `null` for the plain tag (Enter
   * keeps making it, as before). */
  let highlight = $state<number | null>(null);
  const suggestions = $derived(matchCategories(input, choices));
  const suggesting = $derived(focused && !dismissed && suggestions.length > 0);

  function onfocus() {
    focused = true;
    if (!choices.length) {
      listCategoryChoices(detail.kind)
        .then((list) => (choices = list))
        .catch((e) => console.warn("category choices", e));
    }
  }

  function oninput() {
    dismissed = false;
    highlight = null;
  }

  /** The category line chosen: forced on a car, added to a track - never a
   * plain tag. */
  function pickCategory(tag: string) {
    input = "";
    highlight = null;
    oncategory(isCar ? { op: "force", tag } : { op: "add", tag });
  }

  function onkeydown(e: KeyboardEvent) {
    if (suggesting && e.key === "ArrowUp") {
      e.preventDefault();
      highlight = highlight === null ? suggestions.length - 1 : Math.max(0, highlight - 1);
    } else if (suggesting && e.key === "ArrowDown") {
      e.preventDefault();
      highlight = highlight === null || highlight + 1 >= suggestions.length ? null : highlight + 1;
    } else if (e.key === "Escape" && suggesting) {
      e.preventDefault();
      dismissed = true;
    } else if (e.key === "Enter") {
      if (suggesting && highlight !== null) pickCategory(suggestions[highlight]);
      else submit();
    }
  }

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

  // Les catégories (#) passent devant : ce sont les tags structurants (§5).
  //
  // A car shows the category the user forced, else the `#` tags the rules gave
  // it; a track shows its categories as they are now - the computed list with the
  // user's additions, less what was taken off - so a category taken off is not
  // drawn.
  const categories = $derived(
    isCar
      ? detail.category_forced
        ? [detail.category_forced]
        : detail.tags_from_rule.filter((tag) => tag.startsWith("#"))
      : detail.categories,
  );
  const decided = (tag: string) => (isCar ? tag === detail.category_forced : detail.categories_added.includes(tag));
  const computedText = $derived(
    t("categoryDecision.computed", {
      value:
        (isCar ? (detail.category_auto ? [detail.category_auto] : []) : detail.categories_auto).join(", ") ||
        t("categoryDecision.computedNone"),
    }),
  );
  const trackEdited = $derived(!isCar && (detail.categories_added.length > 0 || detail.categories_removed.length > 0));
  // A car has ONE category: once the user forced it, the `#` tags the rules gave
  // are still tags of the car - drawn as the rules' (green), not as a second
  // category. A track's `#` tags ARE its computed categories, already shown (or
  // taken off) above.
  const fromRules = $derived(
    isCar
      ? detail.tags_from_rule.filter((tag) => !categories.includes(tag))
      : detail.tags_from_rule.filter((tag) => !tag.startsWith("#")),
  );

  function submit() {
    // Normalisé en minuscules ici comme à l'import : sinon « GT3 » et « gt3 »
    // cohabiteraient dans la même liste.
    const tag = input.trim().toLowerCase();
    input = "";
    highlight = null;
    if (tag) onaddtag(tag);
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.tagsLabel")}</span>
    {#if trackEdited}
      <ManualMark
        flagTitle={t("categoryDecision.manual")}
        resetTitle={computedText}
        onreset={() => oncategory({ op: "reset" })}
      />
    {/if}
  </header>
  <div class="blk-b">
    <div class="tags">
      {#each categories as tag (tag)}
        <span class="tag cat">
          {tag}
          {#if decided(tag)}
            <!-- The way back is on the car's chip itself; a track has one for the
                 whole block (above), and a chip of its own only to take away. -->
            <ManualMark
              flagTitle={t("categoryDecision.manual")}
              resetTitle={isCar ? computedText : undefined}
              onreset={isCar ? () => oncategory({ op: "reset" }) : undefined}
            />
          {/if}
          {#if !isCar}
            <button
              class="x"
              type="button"
              onclick={() => oncategory({ op: "remove", tag })}
              title={t("categoryDecision.removeFromTrack")}>×</button
            >
          {/if}
        </span>
      {/each}
      {#if !isCar}
        <CategoryPicker
          choices={() => listCategoryChoices(detail.kind)}
          onpick={(tag) => oncategory({ op: "add", tag })}
        >
          {#snippet trigger(toggle)}
            <button class="tag add" type="button" onclick={toggle} title={t("categoryDecision.add")}>+</button>
          {/snippet}
        </CategoryPicker>
      {/if}
      {#each fromRules as tag}<span class="tag rule">{tag}</span>{/each}
      {#each detail.tags_manual as tag}
        <span class="tag manual" title={tag.startsWith("#") ? t("categoryDecision.tagOnly") : undefined}>
          {tag}<button class="x" type="button" onclick={() => onremovetag(tag)} title={t("common.remove")}>×</button>
        </span>
      {/each}
      {#if showRawTags}
        {#each detail.tags_from_mod as tag}<span class="tag mod">{tag}</span>{/each}
      {/if}
    </div>

    <div class="tag-actions">
      <div class="manual-field">
        <input
          class="input manual-input"
          autocomplete="off"
          placeholder={t("detail.addTagPlaceholder")}
          bind:value={input}
          {onfocus}
          onblur={() => (focused = false)}
          {oninput}
          {onkeydown}
        />
        {#if suggesting}
          <ul class="suggest" role="listbox">
            {#each suggestions as tag, i (tag)}
              <li>
                <button
                  class="sg"
                  class:hi={highlight === i}
                  type="button"
                  role="option"
                  aria-selected={highlight === i}
                  onmousedown={(e) => {
                    e.preventDefault();
                    pickCategory(tag);
                  }}
                >
                  <span class="tag cat">{tag}</span>
                  <span class="to">{t("categoryDecision.toCategory")}</span>
                </button>
              </li>
            {/each}
            <!-- The usual line: what Enter makes when nothing above is chosen. -->
            <li>
              <button
                class="sg"
                class:hi={highlight === null}
                type="button"
                role="option"
                aria-selected={highlight === null}
                onmousedown={(e) => {
                  e.preventDefault();
                  submit();
                }}
              >
                <span class="tag manual">{input.trim().toLowerCase()}</span>
              </button>
            </li>
          </ul>
        {/if}
      </div>
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
    display: inline-flex;
    align-items: center;
    gap: 5px;
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
  .manual-field {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .manual-input {
    width: 100%;
    padding: 9px 11px;
    font-size: 12px;
  }
  /* The suggestions hang off the field - no measured pixel, so the interface
     zoom has nothing to double. */
  .suggest {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 2px);
    z-index: 20;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.45);
  }
  .sg {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: transparent;
    border: none;
    padding: 4px 10px;
    text-align: left;
    cursor: pointer;
  }
  .sg.hi,
  .sg:hover {
    background: var(--raised);
  }
  .sg .to {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
  }
  .tag.add {
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .tag.add:hover {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
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
