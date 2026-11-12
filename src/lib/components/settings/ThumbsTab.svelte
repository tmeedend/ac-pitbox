<script lang="ts">
  // Presets de vignettes de la grille (docs/SPEC-grille.md GRILLE§6).
  //
  // **Un onglet à part de l'aperçu 3D, et ce n'est pas un rangement** (GRILLE§6.1).
  // Les deux règlent une caméra et des lumières, mais tourner l'aperçu d'une
  // fiche ne coûte rien et ne dure que le temps qu'on la regarde, alors que
  // toucher à un preset périme les images de toute la grille. C'est cette
  // asymétrie qui rend acceptable qu'un écran affiche une facture et prévienne,
  // et que l'autre n'avertisse jamais.
  //
  // D'où la règle du GRILLE§6.3 : **rien ne s'applique avant Appliquer.** Manipuler
  // les curseurs ne régénère rien, le pied de l'écran chiffre ce que ça
  // coûtera, et Annuler ne coûte rien — ce qui rend l'expérimentation gratuite.
  //
  // **Les embarqués sont en lecture seule, on les duplique.** Un preset vide
  // serait une douzaine de curseurs de rien ; une copie de Vitrine est à un
  // réglage d'être la sienne. C'est aussi ce qui remplace le bouton « rétablir
  // le gabarit d'origine » : l'original est toujours là, juste à côté, intact.
  import Field from "$lib/components/ui/Field.svelte";
  import GridTemplateStudio from "./GridTemplateStudio.svelte";
  import TuningFrame from "./TuningFrame.svelte";
  import Slider from "$lib/components/ui/Slider.svelte";
  import { errorText } from "$lib/errors";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { listLibrary } from "$lib/library/library";
  import {
    clearGridThumbnails,
    gridThumbnailCoverage,
    gridThumbnailStats,
    type GridThumbStats,
  } from "$lib/gridthumbs/gridThumbs";
  import {
    inGameProgress,
    openFailedCar,
    protectedCarsForGame,
    protectedSummary,
    requestInGame,
    type InGameCar,
  } from "$lib/gridthumbs/inGame.svelte";
  import type { GridCoverage, ProtectedSummary } from "$lib/gridthumbs/gridThumbs";
  import { gridThumbProgress } from "$lib/gridthumbs/gridThumbs.svelte";
  import { enqueueGridThumbs } from "$lib/gridthumbs/gridThumbs.svelte";
  import { thumbnailSkinId } from "$lib/preferred";
  import { withoutBrand } from "$lib/library/displayName";
  import {
    GRID_THUMB_RANGES,
    allPresets,
    bindPreset,
    deletePreset,
    duplicatePreset,
    gridThumbPrefs,
    gridThumbsDirty,
    presetFor,
    presetName,
    renamePreset,
    renderTemplate,
    revertGridThumbPrefs,
    saveGridThumbPrefs,
    setPresetMat,
    setPresetSkipStock,
    setPresetValue,
    setGridThumbsEnabled,
    THUMB_SLOTS,
    type ThumbSlot,
    type GridPreset,
  } from "$lib/gridthumbs/gridThumbPrefs.svelte";

  const prefs = $derived(gridThumbPrefs());
  const presets = $derived(allPresets());
  const dirty = $derived(gridThumbsDirty());

  /** Le preset en cours d'édition. Par défaut celui de la grille dense — c'est
   * la vue par défaut, donc celui qu'on vient régler. */
  let selectedId = $state<string | null>(null);
  const selected = $derived(presets.find((p) => p.id === (selectedId ?? prefs.bound.dense)) ?? presets[0]);
  const editable = $derived(selected && !selected.builtin);

  /** Les deux groupes de curseurs, et leur ordre. Le cadrage d'abord — c'est
   * lui qu'on vient régler — la lumière ensuite. */
  const FRAMING = ["azimuth", "elevation", "fov", "margin", "height"] as const;
  const LIGHT = ["ambient", "glass", "chrome", "saturation", "coolTint"] as const;
  /** Le sol est un groupe à lui : c'est ce qui sépare le plus nettement un
   * preset de catalogue — voiture détourée, rien d'autre — d'un preset de
   * vitrine, où la flaque et le reflet sont l'essentiel de l'effet. */
  const GROUND = ["floor", "poolSize", "poolEdge", "reflection", "reflectionBlur", "shadow"] as const;

  let applying = $state(false);
  let stats = $state<GridThumbStats | null>(null);
  let clearing = $state(false);
  let error = $state<string | null>(null);
  let carCount = $state<number | null>(null);

  async function refresh() {
    try {
      stats = await gridThumbnailStats();
    } catch (e) {
      console.error("grid_thumbnail_stats", e);
    }
  }

  /** The protected cars the game can render under the presets in use
   * (GRILLE§7.1) — none when no bound preset is renderable. */
  let protectedCars = $state<InGameCar[]>([]);
  /** The same cars counted with those the game already rendered, by car. */
  let summary = $state<ProtectedSummary | null>(null);
  /** What each view lacks, under the preset saved for it. */
  let coverage = $state<Partial<Record<ThumbSlot, GridCoverage>>>({});

  /** Asked again when a run ends — Pit Box's or the game's — and when the
   * presets of the views change once saved. */
  $effect(() => {
    const slots = THUMB_SLOTS.map((slot) => ({ slot, preset: presetFor(slot) }));
    void genProgress.running;
    void inGame.running;
    void gridThumbnailCoverage(
      slots.map(({ preset }) => ({ template: renderTemplate(preset), skipStock: preset.skipStock })),
    )
      .then((found) => {
        coverage = Object.fromEntries(slots.map(({ slot }, i) => [slot, found[i]]));
      })
      .catch((e) => console.error("couverture des vues", e));
  });

  /** What a view's line says it lacks — nothing when it is complete. */
  function lacking(slot: ThumbSlot): string {
    const c = coverage[slot];
    if (!c) return "";
    if (c.missing === 0 && c.protectedMissing === 0) return t("settings.gridThumbsViewComplete");
    const parts: string[] = [];
    if (c.missing > 0) parts.push(t("settings.gridThumbsViewMissing", { count: String(c.missing) }));
    if (c.protectedMissing > 0) {
      parts.push(t("settings.gridThumbsViewProtected", { count: String(c.protectedMissing) }));
    }
    return parts.join(" · ");
  }
  const inGame = $derived(inGameProgress());
  const genProgress = $derived(gridThumbProgress());

  $effect(() => {
    void refresh();
    void protectedCarsForGame()
      .then((cars) => (protectedCars = cars))
      .catch((e) => console.error("voitures protégées", e));
    void protectedSummary()
      .then((s) => (summary = s))
      .catch((e) => console.error("voitures protégées, décompte", e));
    void listLibrary()
      .then((list) => (carCount = list.filter((c) => c.kind === "Car").length))
      .catch(() => undefined);
  });

  /**
   * La facture du GRILLE§6.3, affichée en continu.
   *
   * **Toutes les voitures**, moins celles qu'on sait protégées : changer un
   * seul degré change l'empreinte du gabarit, donc le nom de toutes les images,
   * donc aucune n'est réutilisable. C'est précisément ce que l'utilisateur doit
   * savoir avant de cliquer.
   */
  const bill = $derived.by(() => {
    if (!dirty || carCount === null) return null;
    const count = Math.max(0, carCount - (stats?.failed ?? 0));
    // Une seconde et demie par voiture, mesurée sur la conversion complète.
    // Approximatif et annoncé comme tel — un ordre de grandeur, pas une
    // promesse.
    const minutes = Math.max(1, Math.round((count * 1.5) / 60));
    return t("settings.gridThumbsBill", { count: String(count), minutes: String(minutes) });
  });

  async function apply() {
    applying = true;
    try {
      await saveGridThumbPrefs();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      applying = false;
    }
    await refresh();
  }

  async function clear() {
    clearing = true;
    try {
      await clearGridThumbnails();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      clearing = false;
    }
    await refresh();
  }

  function duplicate() {
    if (!selected) return;
    const id = duplicatePreset(selected.id, t("settings.gridPresetCopy", { name: presetName(selected) }));
    if (id) selectedId = id;
  }

  function remove() {
    if (!selected || selected.builtin) return;
    deletePreset(selected.id);
    selectedId = null;
  }

  /**
   * « Générer toute la bibliothèque » (GRILLE§5.4).
   *
   * Ce n'est pas le même besoin que la génération au fil de l'eau : à
   * l'installation on exprime une intention, ici on déclenche un travail —
   * typiquement après avoir ajouté cinquante mods. Il met en file **les presets
   * de toutes les vues**, voiture de session comprise : sinon changer de
   * densité juste après relancerait tout, et une voiture protégée ne serait
   * jamais reconnue comme telle sous le preset d'une vue qu'on n'a pas
   * générée — le jeu ne la rendrait donc jamais pour elle (signalé à l'écran).
   */
  async function generateAll() {
    const list = (await listLibrary().catch(() => [])).filter((c) => c.kind === "Car");
    const seen = new Set<string>();
    for (const slot of THUMB_SLOTS) {
      const preset = presets.find((p) => p.id === prefs.bound[slot]);
      if (!preset || seen.has(preset.id)) continue;
      seen.add(preset.id);
      enqueueGridThumbs(
        renderTemplate(preset),
        list
          .filter((c) => !preset.skipStock || !c.is_stock)
          .map((c) => ({
            id: c.id_interne,
            skin: thumbnailSkinId(c.id_interne),
            name: withoutBrand(c.display_name ?? c.id_interne, c.brand ?? null),
          })),
      );
    }
  }

  function degrees(value: number): string {
    return value.toLocaleString(i18n.locale) + "°";
  }
  /** Le flou se règle au dixième : la mécanique travaille sur des entiers, mais
   * un dixième se voit à l'œil. */
  function tenths(value: number): string {
    return (value / 10).toLocaleString(i18n.locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  }
  function percent(value: number): string {
    return value.toLocaleString(i18n.locale) + " %";
  }
  function megabytes(bytes: number): string {
    return Math.round(bytes / (1024 * 1024)).toLocaleString(i18n.locale) + " " + t("settings.gridThumbsMb");
  }
  function unit(key: string, value: number): string {
    return key === "azimuth" || key === "elevation" || key === "fov" ? degrees(value) : percent(value);
  }
</script>

<TuningFrame>
  {#snippet head()}
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridPresets")}</span></div>
      <div class="blk-b">
        <Field label={t("settings.gridPresetEdited")}>
          <select class="input" value={selected?.id} onchange={(e) => (selectedId = e.currentTarget.value)}>
            {#each presets as preset (preset.id)}
              <option value={preset.id}>{presetName(preset)}</option>
            {/each}
          </select>
        </Field>
        <div class="row">
          <button class="btn" type="button" onclick={duplicate}>{t("settings.gridPresetDuplicate")}</button>
          <button class="btn" type="button" onclick={remove} disabled={!editable}>
            {t("settings.gridPresetDelete")}
          </button>
        </div>
        {#if editable && selected}
          <Field label={t("settings.gridPresetName")}>
            <input
              class="input"
              type="text"
              value={selected.name}
              oninput={(e) => renamePreset(selected.id, e.currentTarget.value)}
            />
          </Field>
          <Field hint={t("settings.gridPresetSkipStockHint")}>
            <label class="check">
              <input
                type="checkbox"
                checked={selected.skipStock}
                onchange={(e) => setPresetSkipStock(selected.id, e.currentTarget.checked)}
              />
              <span>{t("settings.gridPresetSkipStock")}</span>
            </label>
          </Field>
        {:else}
          <!-- Un embarqué ne se modifie pas : il est la référence à laquelle on
               revient, donc il doit rester exactement ce qu'il était. -->
          <p class="notice">{t("settings.gridPresetReadOnly")}</p>
        {/if}
      </div>
    </section>
  {/snippet}

  {#snippet preview()}
    <!-- On règle en voyant le résultat, et sur un catalogue puisque c'est un
         catalogue qu'on édite. -->
    {#if selected}
      <GridTemplateStudio template={renderTemplate(selected)} mat={selected.mat} />
    {/if}
  {/snippet}

  {#snippet side()}
    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{t("settings.gridThumbsGroup")}</span>
        {#if stats && stats.generated > 0}
          <span class="blk-n">{t("settings.gridThumbsSize", { size: megabytes(stats.bytes) })}</span>
        {/if}
      </div>
      <div class="blk-b">
        <Field hint={t("settings.gridThumbsHint")}>
          <label class="check">
            <input
              type="checkbox"
              checked={prefs.enabled}
              onchange={(e) => setGridThumbsEnabled(e.currentTarget.checked)}
            />
            <span>{t("settings.gridThumbs")}</span>
          </label>
        </Field>

        <!-- **Le preset suit la densité de la grille.** La densité est déjà une
             déclaration d'intention : dense = « je cherche », confortable =
             « je regarde ». Les deux jeux d'images coexistent, donc rebasculer
             est instantané une fois les deux produits. The session car has a
             preset of its own (GRILLE§5.6). -->
        {#each THUMB_SLOTS as slot (slot)}
          <Field label={t("settings.gridThumbsFor_" + slot)} hint={prefs.enabled ? lacking(slot) : undefined}>
            <select class="input" value={prefs.bound[slot]} onchange={(e) => bindPreset(slot, e.currentTarget.value)}>
              {#each presets as preset (preset.id)}
                <option value={preset.id}>{presetName(preset)}</option>
              {/each}
            </select>
          </Field>
        {/each}

        <Field hint={t("settings.gridThumbsClearHint")}>
          <div class="row">
            <button class="btn" type="button" onclick={generateAll} disabled={!prefs.enabled}>
              {t("settings.gridThumbsGenerateAll")}
            </button>
            <button
              class="btn"
              type="button"
              onclick={clear}
              disabled={clearing || !stats || (stats.generated === 0 && stats.failed === 0)}
            >
              {t("settings.gridThumbsClear")}
            </button>
            <!-- Protected cars are counted by car, with those the game already
                 rendered: they are not impossible, the button beside renders
                 the rest (GRILLE§7.1). Said once, with no tone of failure. -->
            {#if summary && summary.pending + summary.rendered > 0}
              <span class="muted">
                {t("settings.gridThumbsProtected", {
                  total: String(summary.pending + summary.rendered),
                  rendered: String(summary.rendered),
                  pending: String(summary.pending),
                })}
              </span>
            {/if}
            {#if protectedCars.length > 0}
              <button class="btn" type="button" onclick={() => requestInGame(protectedCars)}>
                {protectedCars.length > 1
                  ? t("gridThumbs.inGameActionN", { count: protectedCars.length })
                  : t("gridThumbs.inGameAction")}
              </button>
            {/if}
          </div>
        </Field>
        {#if error}<div class="errbox">{errorText(error)}</div>{/if}
      </div>
    </section>

    <!-- What the last run in the game did not photograph, until the next run
         (in memory only: a failure in the game is not remembered). Each car
         leads to its sheet, where its thumbnails are. -->
    {#if inGame.failures.length > 0}
      <section class="blk">
        <div class="blk-h">
          <span class="blk-t">{t("settings.gridThumbsInGameFailures")}</span>
          <span class="blk-n">{inGame.failures.length}</span>
        </div>
        <div class="blk-b">
          <ul class="failures">
            {#each inGame.failures as failure (failure.id)}
              <li>
                <button class="car-link" type="button" onclick={() => void openFailedCar(failure.id)}>
                  {failure.name}
                </button>
                <span class="muted">{errorText(failure.reason)}</span>
              </li>
            {/each}
          </ul>
        </div>
      </section>
    {/if}
  {/snippet}

  {#if selected}
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsFraming")}</span></div>
      <div class="blk-b">
        <Field hint={t("settings.gridThumbHint_gameFraming")}>
          <label class="check">
            <input
              type="checkbox"
              disabled={!editable}
              checked={selected.template.gameFraming > 0}
              onchange={(e) => setPresetValue(selected.id, "gameFraming", e.currentTarget.checked ? 100 : 0)}
            />
            <span>{t("settings.gridThumb_gameFraming")}</span>
          </label>
        </Field>
        <Field hint={t("settings.gridThumbHint_trueScale")}>
          <label class="check">
            <input
              type="checkbox"
              disabled={!editable || selected.template.gameFraming > 0}
              checked={selected.template.trueScale > 0}
              onchange={(e) => setPresetValue(selected.id, "trueScale", e.currentTarget.checked ? 100 : 0)}
            />
            <span>{t("settings.gridThumb_trueScale")}</span>
          </label>
        </Field>
        {#each FRAMING as key (key)}
          <Slider
            label={t("settings.gridThumb_" + key)}
            value={selected.template[key]}
            min={GRID_THUMB_RANGES[key].min}
            max={GRID_THUMB_RANGES[key].max}
            step={GRID_THUMB_RANGES[key].step}
            display={unit(key, selected.template[key])}
            hint={t("settings.gridThumbHint_" + key)}
            disabled={!editable || selected.template.gameFraming > 0}
            oninput={(v) => setPresetValue(selected.id, key, v)}
          />
        {/each}
      </div>
    </section>
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsLight")}</span></div>
      <div class="blk-b">
        {#each LIGHT as key (key)}
          <Slider
            label={t("settings.gridThumb_" + key)}
            value={selected.template[key]}
            min={GRID_THUMB_RANGES[key].min}
            max={GRID_THUMB_RANGES[key].max}
            step={GRID_THUMB_RANGES[key].step}
            display={percent(selected.template[key])}
            hint={t("settings.gridThumbHint_" + key)}
            disabled={!editable}
            oninput={(v) => setPresetValue(selected.id, key, v)}
          />
        {/each}
        <Field hint={t("settings.gridThumbHint_autoExposure")}>
          <label class="check">
            <input
              type="checkbox"
              disabled={!editable}
              checked={selected.template.autoExposure > 0}
              onchange={(e) => setPresetValue(selected.id, "autoExposure", e.currentTarget.checked ? 100 : 0)}
            />
            <span>{t("settings.gridThumb_autoExposure")}</span>
          </label>
        </Field>
      </div>
    </section>
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsGround")}</span></div>
      <div class="blk-b">
        {#each GROUND as key (key)}
          <Slider
            label={t("settings.gridThumb_" + key)}
            value={selected.template[key]}
            min={GRID_THUMB_RANGES[key].min}
            max={GRID_THUMB_RANGES[key].max}
            step={GRID_THUMB_RANGES[key].step}
            display={key === "reflectionBlur" ? tenths(selected.template[key]) : percent(selected.template[key])}
            hint={t("settings.gridThumbHint_" + key)}
            disabled={!editable || (key === "poolEdge" && selected.template.poolSize === 0)}
            oninput={(v) => setPresetValue(selected.id, key, v)}
          />
        {/each}
      </div>
    </section>
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsBackdrop")}</span></div>
      <div class="blk-b">
        <!-- Le fond de carte : deux couleurs, celle du centre et celle des
             bords. Sur un preset détouré c'est un réglage gratuit — le mat
               est du CSS. Sur un preset à fond cuit, ce sont les couleurs
               peintes dans l'image, donc les changer régénère : c'est la seule
               exception à « le mat n'entre pas dans l'empreinte ». -->
        <Field label={t("settings.gridPresetMat")} hint={t("settings.gridPresetMatHint")}>
          <div class="mat">
            <label>
              <input
                type="color"
                disabled={!editable}
                value={selected.mat.hi}
                oninput={(e) => setPresetMat(selected.id, { hi: e.currentTarget.value })}
              />
              <span>{t("settings.gridPresetMatHi")}</span>
            </label>
            <label>
              <input
                type="color"
                disabled={!editable}
                value={selected.mat.lo}
                oninput={(e) => setPresetMat(selected.id, { lo: e.currentTarget.value })}
              />
              <span>{t("settings.gridPresetMatLo")}</span>
            </label>
            <span
              class="swatch"
              style:background="radial-gradient(ellipse at 50% 44%, {selected.mat.hi} 0%, {selected.mat.lo} 76%)"
            ></span>
          </div>
        </Field>
        <!-- **Une case et non un curseur.** Un fond à moitié cuit n'a pas de
             sens : ou bien la carte possède le fond — la vignette est
             détourée et le fond suit le thème — ou bien l'image le porte,
             pour ne pas se distinguer d'une preview d'origine. Entre les
             deux, on empile deux fonds l'un sur l'autre. -->
        <Field hint={t("settings.gridThumbHint_background")}>
          <label class="check">
            <input
              type="checkbox"
              disabled={!editable}
              checked={selected.template.background > 0}
              onchange={(e) => setPresetValue(selected.id, "background", e.currentTarget.checked ? 100 : 0)}
            />
            <span>{t("settings.gridThumb_background")}</span>
          </label>
        </Field>
      </div>
    </section>
  {/if}
</TuningFrame>

<footer>
  {#if bill}<span class="bill">{bill}</span>{/if}
  <button class="btn" type="button" onclick={revertGridThumbPrefs} disabled={applying || !dirty}>
    {t("settings.discard")}
  </button>
  <button class="btn btn-primary" type="button" onclick={apply} disabled={applying || !dirty}>
    {applying ? t("settings.saving") : t("settings.gridThumbsApply")}
  </button>
</footer>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
    flex-wrap: wrap;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .mat {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 6px;
  }
  .mat label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--txt2);
    cursor: pointer;
  }
  .mat input {
    width: 34px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--line);
    background: none;
    cursor: pointer;
  }
  /* Le dégradé tel qu'il sera, à côté de ses deux bouts : deux carrés de
     couleur ne disent pas ce que donne leur mélange. */
  .swatch {
    flex: 1;
    height: 22px;
    border: 1px solid var(--mat-line);
  }
  .muted {
    color: var(--muted2);
    font-size: 11.5px;
  }
  .failures {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .failures li {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .car-link {
    align-self: flex-start;
    padding: 0;
    background: none;
    border: none;
    color: var(--txt);
    font-size: 12.5px;
    text-decoration: underline;
    text-decoration-color: var(--faint);
    cursor: pointer;
  }
  .car-link:hover {
    color: var(--rosso-bright);
  }
  .notice {
    margin: 14px 0 0;
    padding: 8px 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
  }
  .errbox {
    margin-top: 10px;
  }
  /* Même barre que les autres onglets de Réglages. La facture est à gauche,
     les boutons à droite : on lit ce que ça coûte avant d'atteindre le bouton
     qui le déclenche. */
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
    /* In sight too: what Apply will cost, and the button, whatever the
       settings' scroll. */
    position: sticky;
    bottom: 0;
    z-index: 4;
    background: var(--bg);
  }
  .bill {
    margin-right: auto;
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
