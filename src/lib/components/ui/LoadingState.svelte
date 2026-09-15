<script lang="ts">
  // Indicateur de chargement générique (liste en cours de peuplement). Chaque
  // écran de liste avait fini par dupliquer la même icône + le même texte —
  // extrait ici une fois pour toutes.
  import { t } from "$lib/i18n/index.svelte";
</script>

<div class="loading">
  <span class="spinner"></span>
  <p>{t("common.loading")}</p>
</div>

<style>
  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
  .spinner {
    width: 22px;
    height: 22px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: loading-state-spin 0.8s linear infinite;
  }
  @keyframes loading-state-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
