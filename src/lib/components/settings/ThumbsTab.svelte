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
  import Field from "../Field.svelte";
  import GridTemplateStudio from "./GridTemplateStudio.svelte";
  import Slider from "../Slider.svelte";
  import { errorText } from "$lib/errors";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { listLibrary } from "$lib/library";
  import { clearGridThumbnails, gridThumbnailStats, type GridThumbStats } from "$lib/gridThumbs";
  import { enqueueGridThumbs } from "$lib/gridThumbs.svelte";
  import { getPreferredSkin } from "$lib/preferred";
  import { withoutBrand } from "$lib/displayName";
  import {
    GRID_THUMB_RANGES,
    allPresets,
    bindPreset,
    deletePreset,
    duplicatePreset,
    gridThumbPrefs,
    gridThumbsDirty,
    renamePreset,
    renderTemplate,
    revertGridThumbPrefs,
    saveGridThumbPrefs,
    setPresetMat,
    setPresetSkipStock,
    setPresetValue,
    setGridThumbsEnabled,
    type GridDensity,
    type GridPreset,
  } from "$lib/gridThumbPrefs.svelte";

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
  const LIGHT = ["key", "fill", "rim"] as const;
  /** Le sol est un groupe à lui : c'est ce qui sépare le plus nettement un
   * preset de catalogue — voiture détourée, rien d'autre — d'un preset de
   * vitrine, où la flaque et le reflet sont l'essentiel de l'effet. */
  const GROUND = ["floor", "reflection", "reflectionBlur", "shadow"] as const;
  const DENSITIES: GridDensity[] = ["dense", "comfortable"];

  let applying = $state(false);
  let stats = $state<GridThumbStats | null>(null);
  let clearing = $state(false);
  let error = $state<string | null>(null);
  let carCount = $state<number | null>(null);

  /** Le nom affiché d'un preset : sa clé i18n s'il est livré avec l'app — pour
   * qu'il suive la langue — le nom tapé sinon, qui appartient à quelqu'un et ne
   * se traduit donc pas. */
  function presetName(preset: GridPreset): string {
    return preset.builtin ? t("settings.gridPreset_" + preset.id) : preset.name;
  }

  async function refresh() {
    try {
      stats = await gridThumbnailStats();
    } catch (e) {
      console.error("grid_thumbnail_stats", e);
    }
  }

  $effect(() => {
    void refresh();
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
   * typiquement après avoir ajouté cinquante mods. Il met en file **les deux
   * presets en usage**, pas seulement celui de la vue courante : sinon changer
   * de densité juste après relancerait tout, et le bouton aurait menti.
   */
  async function generateAll() {
    const list = (await listLibrary().catch(() => [])).filter((c) => c.kind === "Car");
    const seen = new Set<string>();
    for (const density of DENSITIES) {
      const preset = presets.find((p) => p.id === prefs.bound[density]);
      if (!preset || seen.has(preset.id)) continue;
      seen.add(preset.id);
      enqueueGridThumbs(
        renderTemplate(preset),
        list
          .filter((c) => !preset.skipStock || !c.is_stock)
          .map((c) => ({
            id: c.id_interne,
            skin: getPreferredSkin(c.id_interne)?.id ?? null,
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

<div class="cards">
  <div class="col">
    <!-- L'aperçu en tête, comme l'onglet Aperçu 3D : on règle en voyant le
         résultat, et sur un catalogue puisque c'est un catalogue qu'on édite. -->
    {#if selected}
      <GridTemplateStudio template={renderTemplate(selected)} mat={selected.mat} />
    {/if}

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
             est instantané une fois les deux produits. -->
        {#each DENSITIES as density (density)}
          <Field label={t("settings.gridThumbsFor_" + density)}>
            <select class="input" value={prefs.bound[density]} onchange={(e) => bindPreset(density, e.currentTarget.value)}>
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
            <!-- Le décompte des impossibles se dit une fois, sans tonalité
                 d'échec : l'utilisateur n'y peut rien, aucune action n'est
                 proposable, et ces cartes restent parfaitement utilisables avec
                 leur photo d'origine (GRILLE§7). -->
            {#if stats && stats.failed > 0}
              <span class="muted">{t("settings.gridThumbsFailed", { count: String(stats.failed) })}</span>
            {/if}
          </div>
        </Field>
        {#if error}<div class="errbox">{errorText(error)}</div>{/if}
      </div>
    </section>
  </div>

  <div class="col">
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
          <!-- **Une case et non un curseur.** Un fond à moitié cuit n'a pas de
               sens : ou bien la carte possède le fond — la vignette est
               détourée et le fond suit le thème — ou bien l'image le porte,
               pour ne pas se distinguer d'une preview d'origine. Entre les
               deux, on empile deux fonds l'un sur l'autre. -->
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
                  value={selected.mat.hi}
                  oninput={(e) => setPresetMat(selected.id, { hi: e.currentTarget.value })}
                />
                <span>{t("settings.gridPresetMatHi")}</span>
              </label>
              <label>
                <input
                  type="color"
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
          <Field hint={t("settings.gridThumbHint_background")}>
            <label class="check">
              <input
                type="checkbox"
                checked={selected.template.background > 0}
                onchange={(e) => setPresetValue(selected.id, "background", e.currentTarget.checked ? 100 : 0)}
              />
              <span>{t("settings.gridThumb_background")}</span>
            </label>
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

    {#if selected}
      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsFraming")}</span></div>
        <div class="blk-b">
          {#each FRAMING as key (key)}
            <Slider
              label={t("settings.gridThumb_" + key)}
              value={selected.template[key]}
              min={GRID_THUMB_RANGES[key].min}
              max={GRID_THUMB_RANGES[key].max}
              step={GRID_THUMB_RANGES[key].step}
              display={unit(key, selected.template[key])}
              hint={t("settings.gridThumbHint_" + key)}
              disabled={!editable}
              oninput={(v) => setPresetValue(selected.id, key, v)}
            />
          {/each}
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
              disabled={!editable}
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
        </div>
      </section>
    {/if}
  </div>
</div>

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
  .cards {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
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
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .bill {
    margin-right: auto;
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
