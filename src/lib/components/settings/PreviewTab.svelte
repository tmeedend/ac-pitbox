<script lang="ts">
  // Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
  //
  // Comme MusicTab, cet onglet ne passe pas par AppConfig : ses réglages vivent
  // dans `ui_prefs.json` et s'appliquent à l'instant où on les bouge — d'où
  // l'absence de bouton Enregistrer et de garde de navigation. Une fiche
  // ouverte derrière suit le curseur sans être rechargée.
  //
  // Les curseurs eux-mêmes vivent dans `Preview3dControls`, partagé avec le
  // panneau posé sur la fiche voiture : c'est là qu'on les règle en voyant le
  // résultat, ici qu'on les retrouve avec leur mode d'emploi.
  import Preview3dControls from "../detail/Preview3dControls.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { preview3dPrefs, setPreview3dEnabled } from "$lib/preview3dPrefs.svelte";

  const prefs = $derived(preview3dPrefs());
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
</style>
