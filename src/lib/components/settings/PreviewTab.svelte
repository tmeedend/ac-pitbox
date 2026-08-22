<script lang="ts">
  // Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
  //
  // Trois blocs, et ils ne se ressemblent pas : le **rendu** (qualité, effet
  // d'entrée) et le **cadrage** s'appliquent à l'image suivante, alors que le
  // **cache** touche au disque et évince pour de bon. D'où l'ordre : ce qui ne
  // coûte rien d'abord, ce qui efface des fichiers en dernier.
  //
  // Cet onglet ne passe pas par AppConfig : ses réglages vivent dans
  // `ui_prefs.json` et s'appliquent à l'instant où on les bouge — une fiche
  // ouverte derrière suit le curseur sans être rechargée, donc pas de garde de
  // navigation à poser.
  //
  // Le bouton Enregistrer n'est pas décoratif pour autant : l'écriture disque
  // est **différée** le temps que le curseur s'arrête (sinon un glissé
  // réécrit `ui_prefs.json` cinquante fois). Il force cette écriture et attend
  // qu'elle ait eu lieu — c'est ce que dit la pastille « Enregistré ».
  //
  // Les curseurs de cadrage vivent dans `Preview3dControls`, partagé avec le
  // panneau posé sur la fiche voiture : c'est là qu'on les règle en voyant le
  // résultat, ici qu'on les retrouve avec leur mode d'emploi. La qualité,
  // l'effet d'entrée et le cache n'y sont **pas** : on ne les juge pas en les
  // bougeant, et le panneau compact est déjà serré.
  import Preview3dControls from "../detail/Preview3dControls.svelte";
  import Slider from "../Slider.svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { clearPreviewCache, previewCacheSize } from "$lib/preview";
  import {
    INTRO_EFFECTS,
    PREVIEW3D_RANGES,
    PREVIEW_QUALITIES,
    flushPreview3dPrefs,
    preview3dDirty,
    preview3dPrefs,
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
      await flushPreview3dPrefs();
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

  // Une lecture au montage, et le nettoyage du minuteur au démontage : l'effet
  // ne lit aucune valeur réactive, il ne se rejouera donc pas.
  $effect(() => {
    void refreshCacheSize();
    return () => {
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  });

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

<section class="block">
  <label class="check">
    <input
      type="checkbox"
      checked={prefs.enabled}
      onchange={(e) => setPreview3dEnabled(e.currentTarget.checked)}
    />
    <span>{t("settings.preview3dEnabled")}</span>
  </label>
  <p class="hint">{t("settings.preview3dEnabledHint")}</p>
</section>

<h3 class="lbl">{t("settings.preview3dGroupRender")}</h3>

<section class="block">
  <span class="lbl-key">{t("settings.preview3dQuality")}</span>
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
</section>

<section class="block">
  <span class="lbl-key">{t("settings.preview3dIntro")}</span>
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
</section>

<h3 class="lbl">{t("settings.preview3dGroupFraming")}</h3>

<Preview3dControls />

<h3 class="lbl">{t("settings.preview3dGroupCache")}</h3>

<section class="block">
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
    {#if cacheBytes !== null}
      <span class="used mono">{t("settings.preview3dCacheUsed", { size: gigabytes(cacheBytes) })}</span>
    {/if}
    <button class="btn" type="button" onclick={clearCache} disabled={clearing || cacheBytes === 0}>
      {t("settings.preview3dCacheClear")}
    </button>
  </div>
  {#if cacheError}<div class="err">{errorText(cacheError)}</div>{/if}
</section>

<footer>
  {#if saved}<span class="pill pill-ok">{t("settings.saved")}</span>{/if}
  <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
    {saving ? t("settings.saving") : t("settings.save")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
  .block {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  .block label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 340px;
  }
  .block label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    max-width: none;
    cursor: pointer;
  }
  .hint {
    margin-top: 8px;
    font-size: 11px;
    color: var(--faint);
  }
  h3 {
    margin: 0 0 12px;
  }
  /* Même groupe de boutons radio que le mode de déploiement (ConfigFields) :
     titre par-dessus, explication en dessous, alignés sur le bouton. */
  .radio-opt {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    margin-top: 10px;
    max-width: 520px;
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
    font-size: 11px;
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
  .used {
    font-size: 11.5px;
    color: var(--txt2);
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
