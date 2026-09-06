<script lang="ts">
  // Réglage du gabarit des vignettes de la grille (docs/SPEC-grille.md §6).
  //
  // **Un onglet à part de l'aperçu 3D, et ce n'est pas un rangement** (§6.1).
  // Les deux règlent une caméra et des lumières, mais tourner l'aperçu d'une
  // fiche ne coûte rien et ne dure que le temps qu'on la regarde, alors que
  // toucher à ce gabarit-ci périme les 312 images de la grille. C'est cette
  // asymétrie qui rend acceptable qu'un écran affiche une facture et prévienne,
  // et que l'autre n'avertisse jamais.
  //
  // D'où la règle du §6.3 : **rien ne s'applique avant Appliquer.** Manipuler
  // les curseurs ne régénère rien, le pied de l'écran chiffre ce que ça coûtera,
  // et Annuler ne coûte rien — ce qui rend l'expérimentation gratuite.
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
    editedTemplate,
    gridThumbPrefs,
    gridThumbsDirty,
    newerDefaultTemplate,
    resetGridTemplate,
    revertGridThumbPrefs,
    saveGridThumbPrefs,
    setGridThumbValue,
    setGridThumbsEnabled,
  } from "$lib/gridThumbPrefs.svelte";

  const prefs = $derived(gridThumbPrefs());
  const template = $derived(editedTemplate());
  const dirty = $derived(gridThumbsDirty());

  /** Les deux groupes de curseurs, et leur ordre. Le cadrage d'abord — c'est
   * lui qu'on vient régler — la lumière ensuite. */
  const FRAMING = ["azimuth", "elevation", "fov", "margin"] as const;
  const LIGHT = ["key", "fill", "rim", "shadow"] as const;

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

  $effect(() => {
    void refresh();
    void listLibrary()
      .then((list) => (carCount = list.filter((c) => c.kind === "Car").length))
      .catch(() => undefined);
  });

  /**
   * La facture du §6.3, affichée en continu.
   *
   * **Toutes les voitures**, moins celles qu'on sait protégées : changer un
   * seul degré change l'empreinte du gabarit, donc le nom des 312 images, donc
   * aucune n'est réutilisable. C'est précisément ce que l'utilisateur doit
   * savoir avant de cliquer.
   */
  const bill = $derived.by(() => {
    if (!dirty || carCount === null) return null;
    const count = Math.max(0, carCount - (stats?.failed ?? 0));
    // Une seconde et demie par voiture, mesurée sur la conversion complète.
    // Approximatif et annoncé comme tel — c'est un ordre de grandeur, pas une
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

  /**
   * « Générer toute la bibliothèque » (§5.4).
   *
   * Ce n'est pas le même besoin que la génération au fil de l'eau : à
   * l'installation on exprime une intention, ici on déclenche un travail —
   * typiquement après avoir ajouté cinquante mods. Il met simplement tout en
   * file ; la tâche de fond en bas à droite montre où ça en est et sait
   * s'arrêter.
   */
  async function generateAll() {
    const list = await listLibrary().catch(() => []);
    enqueueGridThumbs(
      list
        .filter((c) => c.kind === "Car")
        .map((c) => ({
          id: c.id_interne,
          skin: getPreferredSkin(c.id_interne)?.id ?? null,
          name: withoutBrand(c.display_name ?? c.id_interne, c.brand ?? null),
        })),
    );
  }

  function degrees(value: number): string {
    return value.toLocaleString(i18n.locale) + "°";
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
    <GridTemplateStudio {template} />

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
                 leur photo d'origine (§7). -->
            {#if stats && stats.failed > 0}
              <span class="muted">{t("settings.gridThumbsFailed", { count: String(stats.failed) })}</span>
            {/if}
          </div>
        </Field>
        {#if error}<div class="err">{errorText(error)}</div>{/if}
      </div>
    </section>
  </div>

  <div class="col">
    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("settings.gridThumbsFraming")}</span></div>
      <div class="blk-b">
        {#each FRAMING as key (key)}
          <Slider
            label={t("settings.gridThumb_" + key)}
            value={template[key]}
            min={GRID_THUMB_RANGES[key].min}
            max={GRID_THUMB_RANGES[key].max}
            step={GRID_THUMB_RANGES[key].step}
            display={unit(key, template[key])}
            hint={t("settings.gridThumbHint_" + key)}
            oninput={(v) => setGridThumbValue(key, v)}
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
            value={template[key]}
            min={GRID_THUMB_RANGES[key].min}
            max={GRID_THUMB_RANGES[key].max}
            step={GRID_THUMB_RANGES[key].step}
            display={percent(template[key])}
            hint={t("settings.gridThumbHint_" + key)}
            oninput={(v) => setGridThumbValue(key, v)}
          />
        {/each}
      </div>
    </section>

    {#if newerDefaultTemplate()}
      <!-- Un gabarit d'origine plus récent existe, et l'utilisateur a le sien :
           on le signale, on ne le remplace pas (§5.7). -->
      <p class="notice">{t("settings.gridThumbsNewerDefault")}</p>
    {/if}
  </div>
</div>

<footer>
  {#if bill}<span class="bill">{bill}</span>{/if}
  <button class="btn" type="button" onclick={resetGridTemplate}>{t("settings.gridThumbsResetTemplate")}</button>
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
  .muted {
    color: var(--muted2);
    font-size: 11.5px;
  }
  .notice {
    margin: 0;
    padding: 8px 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
  }
  .err {
    margin-top: 10px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
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
