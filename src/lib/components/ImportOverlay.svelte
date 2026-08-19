<script lang="ts">
  // Arbitrages bloquants de l'import (§4.2/§4.4) : nouvelle version d'un mod
  // déjà connu, import ambigu (mise à jour ou extension ?). Modales, parce
  // qu'elles attendent une réponse — le reste du retour d'import (progression,
  // rapport) vit dans la pile de notifications, `ImportToasts.svelte`.
  import {
    importState,
    resolvePendingConflict,
    resolveAmbiguous,
  } from "$lib/importState.svelte";
  import { t } from "$lib/i18n/index.svelte";
</script>

{#if importState.pendingConflicts.length}
  {@const c = importState.pendingConflicts[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>{t("importOverlay.newVersionTitle")}</h3>
      <p>
        {t("importOverlay.modalBodyOpen")}<b>{c.newName}</b>{t("importOverlay.modalBodyMid")}<span class="mono">{c.oldId}</span>{t("importOverlay.modalBodyArrow")}<span class="mono">{c.newId}</span>{t("importOverlay.modalBodyEnd")}
      </p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolvePendingConflict(c, "keep_both")}>
          {t("importOverlay.keepBoth")}
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolvePendingConflict(c, "replace")}>
          {t("importOverlay.replaceOld")}
        </button>
      </div>
      {#if importState.pendingConflicts.length > 1}
        <div class="modal-rest">{t("importOverlay.modalRest", { count: importState.pendingConflicts.length - 1 })}</div>
      {/if}
    </div>
  </div>
{/if}

{#if importState.pendingAmbiguous.length && !importState.pendingConflicts.length}
  {@const a = importState.pendingAmbiguous[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>{t("importOverlay.ambiguousTitle")}</h3>
      <p>{t("importOverlay.ambiguousBody", { name: a.name, added: a.added, overwritten: a.overwritten, total: a.total })}</p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolveAmbiguous(a, "extension")}>
          {t("importOverlay.chooseExtension")}
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolveAmbiguous(a, "update")}>
          {t("importOverlay.chooseUpdate")}
        </button>
      </div>
      {#if importState.pendingAmbiguous.length > 1}
        <div class="modal-rest">{t("importOverlay.ambiguousRest", { count: importState.pendingAmbiguous.length - 1 })}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 90;
  }
  .modal {
    width: 440px;
    max-width: 90vw;
    background: var(--panel);
    border: 1px solid var(--rosso);
    padding: 22px 24px;
  }
  .modal h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 12px;
  }
  .modal p {
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--txt2);
    margin-bottom: 18px;
  }
  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
  }
  .modal-rest {
    margin-top: 12px;
    font-size: 11px;
    color: var(--muted);
    text-align: right;
  }
</style>
