<script lang="ts">
  // Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
  //
  // **L'aperçu est ici**, en haut de l'onglet, et c'est ce qui justifie que les
  // treize curseurs y soient aussi : on règle en voyant le résultat. La fiche
  // voiture n'en porte plus qu'un raccourci — son panneau compact ne tenait que
  // cinq curseurs sur treize, et le reste s'y serait entassé.
  //
  // La voiture montrée est **celle de la session en cours** (barre latérale), à
  // défaut la première de la bibliothèque : n'importe laquelle ferait l'affaire
  // pour juger d'un sol ou d'une exposition, autant que ce soit celle que
  // l'utilisateur a en tête.
  //
  // Cinq blocs, et ils ne se ressemblent pas : le rendu, le cadrage,
  // l'éclairage et le sol s'appliquent tous à l'image suivante, alors que le
  // cache touche au disque et évince pour de bon. D'où l'ordre : ce qui ne
  // coûte rien d'abord, ce qui efface des fichiers en dernier.
  //
  // Cet onglet ne passe pas par AppConfig : ses réglages vivent dans
  // `ui_prefs.json` et s'appliquent à l'instant où on les bouge, donc pas de
  // garde de navigation à poser. Le bouton Enregistrer n'est pas décoratif pour
  // autant : l'écriture disque est **différée** le temps que le curseur
  // s'arrête (sinon un glissé réécrit `ui_prefs.json` cinquante fois). Il force
  // cette écriture et attend qu'elle ait eu lieu — c'est ce que dit la pastille
  // « Enregistré ».
  import CarPreview3D from "../detail/CarPreview3D.svelte";
  import Preview3dControls from "../detail/Preview3dControls.svelte";
  import Field from "../Field.svelte";
  import Slider from "../Slider.svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { listLibrary } from "$lib/library";
  import { nav } from "$lib/nav.svelte";
  import { clearPreviewCache, previewCacheSize } from "$lib/preview";
  import {
    DRIVER_MODES,
    INTRO_EFFECTS,
    PREVIEW3D_RANGES,
    PREVIEW_QUALITIES,
    type DriverMode,
    savePreview3dPrefs,
    preview3dDirty,
    preview3dPrefs,
    resetPreview3dView,
    revertPreview3dPrefs,
    setPreview3dDriver,
    setPreview3dEnabled,
    setPreview3dIntro,
    setPreview3dQuality,
    setPreview3dValue,
  } from "$lib/preview3dPrefs.svelte";

  const prefs = $derived(preview3dPrefs());

  let saving = $state(false);
  let saved = $state(false);

  async function save() {
    saving = true;
    try {
      await savePreview3dPrefs();
      saved = true;
    } finally {
      saving = false;
    }
    // Enregistrer force l'écriture, donc l'application du plafond, donc
    // l'éviction : le chiffre affiché doit repartir de la réalité du disque.
    await refreshCacheSize();
  }

  // La pastille ne survit pas au réglage suivant : « Enregistré » doit parler
  // de l'état courant, pas du dernier clic.
  $effect(() => {
    if (preview3dDirty()) saved = false;
  });

  // --- Voiture montrée ----------------------------------------------------

  let sampleCar = $state<string | null>(nav.sessionCar?.id ?? null);
  const sampleSkin = nav.sessionCar?.skin ?? null;

  // Aucune voiture de session : on prend la première de la bibliothèque. Une
  // seule fois au montage — l'effet ne lit aucune valeur réactive.
  $effect(() => {
    if (sampleCar) return;
    void listLibrary()
      .then((mods) => {
        const car = mods.find((m) => m.kind === "Car");
        if (car) sampleCar = car.id_interne;
      })
      .catch((e) => console.error("list_library", e));
  });

  // --- Cache -------------------------------------------------------------

  /** Le plafond n'est appliqué qu'à l'écriture des préférences, elle-même
   * différée de 400 ms. On relit un peu après : sans ça, baisser le plafond
   * évince des entrées pendant que l'écran continue d'afficher l'occupation
   * d'avant — exactement l'impression que le réglage n'a rien fait. */
  const CACHE_REFRESH_DELAY_MS = 700;

  let cacheBytes = $state<number | null>(null);
  let cacheError = $state<string | null>(null);
  let clearing = $state(false);
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;

  async function refreshCacheSize() {
    try {
      cacheBytes = await previewCacheSize();
      cacheError = null;
    } catch (e) {
      cacheError = String(e);
    }
  }

  function scheduleCacheRefresh() {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => void refreshCacheSize(), CACHE_REFRESH_DELAY_MS);
  }

  async function clearCache() {
    clearing = true;
    try {
      await clearPreviewCache();
      cacheError = null;
    } catch (e) {
      cacheError = String(e);
    } finally {
      clearing = false;
    }
    await refreshCacheSize();
  }

  $effect(() => {
    void refreshCacheSize();
    return () => {
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  });

  /** Degrés signés : le signe dit de quel côté le volant est tourné, et un
   * zéro n'a pas à en porter un. */
  function degrees(value: number): string {
    const sign = value > 0 ? "+" : "";
    return sign + value.toLocaleString(i18n.locale) + "°";
  }

  /** Gigaoctets, une décimale, dans la langue de l'app — « 1.5 » et « 1,5 » ne
   * s'écrivent pas de la même façon selon la locale. */
  function gigabytes(bytes: number): string {
    const value = bytes / (1024 * 1024 * 1024);
    const number = value.toLocaleString(i18n.locale, {
      minimumFractionDigits: 1,
      maximumFractionDigits: 1,
    });
    return number + " " + t("settings.preview3dCacheUnit");
  }
</script>

<div class="cards">
  <div class="col">
  {#if prefs.enabled && sampleCar}
    <div class="stage">
      <!-- `driverAlways` : il n'y a pas de clé de contact sur cet écran, et
           régler un pilote qu'on ne voit jamais n'aurait pas de sens. -->
      <CarPreview3D carId={sampleCar} skinId={sampleSkin} driverAlways />
      <!-- Même bouton que sur la fiche voiture : replace la caméra selon les
           réglages et relance le plateau. Ici il sert aussi à **revoir l'effet
           d'entrée**, qui ne se joue par définition qu'à l'entrée. -->
      <button
        class="replace"
        type="button"
        onclick={resetPreview3dView}
        title={t("detail.preview3dReplace")}
        aria-label={t("detail.preview3dReplace")}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" />
          <path d="M13.5 2v3.2h-3.2" />
        </svg>
      </button>
    </div>
  {/if}
  <section class="blk">
  <div class="blk-h"><span class="blk-t">{t("settings.preview3dGroupRender")}</span></div>
  <div class="blk-b">
    <Field hint={t("settings.preview3dEnabledHint")}>
      <label class="check">
        <input
          type="checkbox"
          checked={prefs.enabled}
          onchange={(e) => setPreview3dEnabled(e.currentTarget.checked)}
        />
        <span>{t("settings.preview3dEnabled")}</span>
      </label>
    </Field>

    <Field label={t("settings.preview3dDriver")} hint={t("settings.preview3dDriverHint")}>
      <select
        class="input"
        value={prefs.driver}
        onchange={(e) => setPreview3dDriver(e.currentTarget.value as DriverMode)}
      >
        {#each DRIVER_MODES as mode (mode)}
          <option value={mode}>{t("settings.preview3dDriverOption." + mode)}</option>
        {/each}
      </select>
    </Field>

    <Field>
      <Slider
        label={t("settings.preview3dSteer")}
        value={prefs.steer}
        min={PREVIEW3D_RANGES.steer.min}
        max={PREVIEW3D_RANGES.steer.max}
        step={PREVIEW3D_RANGES.steer.step}
        display={degrees(prefs.steer)}
        hint={t("settings.preview3dSteerHint")}
        oninput={(v) => setPreview3dValue("steer", v)}
      />
    </Field>

    <Field label={t("settings.preview3dQuality")}>
      <div class="radios">
        {#each PREVIEW_QUALITIES as level (level)}
          <label class="radio-opt">
            <input
              type="radio"
              name="preview3d_quality"
              value={level}
              checked={prefs.quality === level}
              onchange={() => setPreview3dQuality(level)}
            />
            <span>
              <span class="radio-title">{t("settings.preview3dQualityOption." + level)}</span>
              <span class="radio-hint">{t("settings.preview3dQualityOption." + level + "Hint")}</span>
            </span>
          </label>
        {/each}
      </div>
    </Field>

    <Field label={t("settings.preview3dIntro")}>
      <div class="radios">
        {#each INTRO_EFFECTS as effect (effect)}
          <label class="radio-opt">
            <input
              type="radio"
              name="preview3d_intro"
              value={effect}
              checked={prefs.intro === effect}
              onchange={() => setPreview3dIntro(effect)}
            />
            <span>
              <span class="radio-title">{t("settings.preview3dIntroOption." + effect)}</span>
              <span class="radio-hint">{t("settings.preview3dIntroOption." + effect + "Hint")}</span>
            </span>
          </label>
        {/each}
      </div>
    </Field>
  </div>
</section>

<section class="blk">
  <div class="blk-h"><span class="blk-t">{t("settings.preview3dGroupLight")}</span></div>
  <div class="blk-b"><Preview3dControls group="light" /></div>
</section>

<section class="blk">
  <div class="blk-h"><span class="blk-t">{t("settings.preview3dGroupFloor")}</span></div>
  <div class="blk-b"><Preview3dControls group="floor" /></div>
</section>
  </div>

  <div class="col">
<section class="blk">
  <div class="blk-h"><span class="blk-t">{t("settings.preview3dGroupFraming")}</span></div>
  <div class="blk-b"><Preview3dControls group="framing" /></div>
</section>

<section class="blk">
  <div class="blk-h">
    <span class="blk-t">{t("settings.preview3dGroupCache")}</span>
    {#if cacheBytes !== null}
      <span class="blk-n">{t("settings.preview3dCacheUsed", { size: gigabytes(cacheBytes) })}</span>
    {/if}
  </div>
  <div class="blk-b">
    <Slider
      label={t("settings.preview3dCache")}
      value={prefs.cacheMb}
      min={PREVIEW3D_RANGES.cacheMb.min}
      max={PREVIEW3D_RANGES.cacheMb.max}
      step={PREVIEW3D_RANGES.cacheMb.step}
      display={gigabytes(prefs.cacheMb * 1024 * 1024)}
      hint={t("settings.preview3dCacheHint")}
      oninput={(v) => {
        setPreview3dValue("cacheMb", v);
        scheduleCacheRefresh();
      }}
    />
    <div class="cache-row">
      <button class="btn" type="button" onclick={clearCache} disabled={clearing || cacheBytes === 0}>
        {t("settings.preview3dCacheClear")}
      </button>
    </div>
    {#if cacheError}<div class="err">{errorText(cacheError)}</div>{/if}
  </div>
  </section>
  </div>
</div>

<footer>
  {#if preview3dDirty()}
    <span class="pill pill-warn">{t("settings.unsavedTitle")}</span>
  {:else if saved}
    <span class="pill pill-ok">{t("settings.saved")}</span>
  {/if}
  <!-- Annuler revient sur les valeurs enregistrées, **y compris à l'écran** :
       sans lui, essayer un réglage était sans retour possible dès qu'on avait
       oublié sa valeur d'avant (retour utilisateur). -->
  <button
    class="btn"
    type="button"
    onclick={revertPreview3dPrefs}
    disabled={saving || !preview3dDirty()}
  >
    {t("settings.discard")}
  </button>
  <button
    class="btn btn-primary"
    type="button"
    onclick={save}
    disabled={saving || !preview3dDirty()}
  >
    {saving ? t("settings.saving") : t("settings.save")}
  </button>
</footer>

<style>
  /* L'aperçu occupe **une colonne**, pas toute la largeur : en pleine largeur
     il devenait immense sur un écran 4K, et il ne ressemblait plus à ce qu'on
     voit sur une fiche voiture — où il tient dans un cadre. En colonne, les
     réglages viennent se ranger à sa droite et on en voit plusieurs d'un coup,
     ce qui est tout l'intérêt de les avoir réunis ici.
     Ratio 16:9, comme la zone héros d'une fiche : c'est le même composant, il
     doit donner la même image. */
  .replace {
    position: absolute;
    right: 10px;
    bottom: 10px;
    z-index: 4;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    /* Mêmes valeurs que `.hero-btn` de la fiche voiture : assez opaque pour se
       détacher d'une carrosserie claire comme d'un fond noir. */
    background: rgba(6, 6, 9, 0.82);
    border: 1px solid var(--muted2);
    color: var(--txt);
    cursor: pointer;
  }
  .replace:hover {
    border-color: var(--rosso);
    color: var(--rosso-bright);
  }
  .replace svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .stage {
    position: relative;
    aspect-ratio: 16 / 9;
    margin-bottom: 14px;
    border: 1px solid var(--line);
    background: var(--card);
    overflow: hidden;
  }
  /* **Deux colonnes assignées, pas un flux.** `columns` laissait le navigateur
     répartir les cartes, et l'ordre obtenu ne pouvait pas être choisi : ce qui
     se règle le plus souvent — le cadrage — tombait où il tombait. Ici la
     colonne de gauche porte l'aperçu et ce qu'on regarde en même temps que lui
     (rendu, éclairage, sol), la droite ce qu'on manipule le plus (cadrage)
     puis le cache, seul bloc qui touche au disque et qui reste donc en
     dernier. Une seule colonne quand la fenêtre est trop étroite pour deux
     lisibles. */
  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    align-items: start;
  }
  @media (max-width: 780px) {
    .cards {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .col {
    min-width: 0;
  }
  /* Les cartes se suivent dans leur colonne ; la première n'a rien au-dessus
     d'elle, la grille posant déjà l'écart entre colonnes. */
  .cards .blk {
    margin-top: 0;
  }
  .cards .blk + .blk {
    margin-top: 14px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--txt2);
    cursor: pointer;
  }
  /* Côte à côte, et non les uns sous les autres : trois options empilées
     mangeaient une hauteur d'écran pour trois mots. Chacune garde une largeur
     confortable pour son explication, et le tout se replie quand la fenêtre
     est étroite. */
  .radios {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 24px;
  }
  .radio-opt {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    flex: 1 1 210px;
    max-width: 320px;
    cursor: pointer;
  }
  .radio-opt input {
    margin-top: 2px;
    accent-color: var(--rosso);
    flex: none;
  }
  .radio-title {
    display: block;
    font-size: 12.5px;
    color: var(--txt);
  }
  .radio-hint {
    display: block;
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.5;
    margin-top: 2px;
  }
  .cache-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
  }
  .err {
    margin-top: 10px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
  /* Même barre que les autres onglets de Réglages (Musique, Général…) :
     pastille d'état puis bouton, alignés à droite. */
  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
  }
</style>
