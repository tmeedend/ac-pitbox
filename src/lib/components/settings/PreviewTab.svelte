<script lang="ts">
  // Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
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
  // Les curseurs eux-mêmes vivent dans `Preview3dControls`, partagé avec le
  // panneau posé sur la fiche voiture : c'est là qu'on les règle en voyant le
  // résultat, ici qu'on les retrouve avec leur mode d'emploi.
  import Preview3dControls from "../detail/Preview3dControls.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    flushPreview3dPrefs,
    preview3dDirty,
    preview3dPrefs,
    setPreview3dEnabled,
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
  }

  // La pastille ne survit pas au réglage suivant : « Enregistré » doit parler
  // de l'état courant, pas du dernier clic.
  $effect(() => {
    if (preview3dDirty()) saved = false;
  });
</script>

<section class="lang-section">
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

<Preview3dControls />

<footer>
  {#if saved}<span class="pill pill-ok">{t("settings.saved")}</span>{/if}
  <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
    {saving ? t("settings.saving") : t("settings.save")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
  .lang-section {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  .lang-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 340px;
  }
  .lang-section label.check {
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
