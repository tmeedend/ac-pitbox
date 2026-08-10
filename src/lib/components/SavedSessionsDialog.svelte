<script lang="ts">
  import { onMount } from "svelte";
  import type { SessionType } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import { listSavedSessions, deleteSavedSession, type SavedSession } from "$lib/savedSessions";

  interface Props {
    sessionType: SessionType;
    onsave: (name: string) => void;
    onclose: () => void;
  }
  let { sessionType, onsave, onclose }: Props = $props();

  // Filtrée par type (§8.4bis, carte « Sessions enregistrées ») : la liste de
  // chargement inline a déjà celles du type courant, cette popup ne sert plus
  // qu'à nommer une sauvegarde — mais choisir un nom existant ici, pour
  // l'écraser, reste plus rapide que de le retaper. Chargée une seule fois à
  // l'ouverture (composant recréé à chaque fois, voir l'appelant) puis
  // rafraîchie manuellement après suppression.
  let sessions = $state<SavedSession[]>([]);
  let name = $state("");
  let confirmName = $state<string | null>(null);

  onMount(() => {
    listSavedSessions(sessionType).then((list) => (sessions = list));
  });

  function pick(s: SavedSession) {
    name = s.name;
    confirmName = null;
  }

  async function remove(s: SavedSession, e: Event) {
    e.stopPropagation();
    await deleteSavedSession(sessionType, s.name);
    sessions = await listSavedSessions(sessionType);
    if (confirmName === s.name) confirmName = null;
  }

  function submitSave() {
    const trimmed = name.trim();
    if (!trimmed) return;
    const exists = sessions.some((s) => s.name === trimmed);
    if (exists && confirmName !== trimmed) {
      confirmName = trimmed;
      return;
    }
    onsave(trimmed);
  }

  function fmtDate(iso: string): string {
    return iso.slice(0, 16).replace("T", " ");
  }
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <h2>{t("launch.saveSessionTitle")}</h2>
      <button class="btn btn-ghost" type="button" onclick={onclose}>✕</button>
    </header>

    <div class="save-row">
      <input
        class="input"
        placeholder={t("launch.sessionNamePlaceholder")}
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && submitSave()}
      />
      <button class="btn btn-primary" type="button" onclick={submitSave} disabled={!name.trim()}>
        {t("settings.save")}
      </button>
    </div>
    {#if confirmName}
      <div class="confirm-overwrite">
        <span>{t("launch.overwriteConfirm", { name: confirmName })}</span>
        <button class="btn btn-primary" type="button" onclick={() => onsave(confirmName ?? "")}>
          {t("launch.overwriteConfirmBtn")}
        </button>
        <button class="btn" type="button" onclick={() => (confirmName = null)}>{t("common.cancel")}</button>
      </div>
    {/if}

    <div class="list">
      {#if !sessions.length}
        <div class="empty">{t("launch.noSavedSessions")}</div>
      {:else}
        {#each sessions as s (s.name)}
          <button class="item" type="button" onclick={() => pick(s)}>
            <div class="item-b">
              <div class="item-name">{s.name}</div>
              <div class="item-meta mono">{fmtDate(s.savedAt)}</div>
            </div>
            <span
              class="item-x"
              role="button"
              tabindex="-1"
              title={t("common.remove")}
              onclick={(e) => remove(s, e)}
              onkeydown={(e) => e.key === "Enter" && remove(s, e)}
            >✕</span>
          </button>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 460px;
    max-width: 92vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .save-row {
    display: flex;
    gap: 8px;
    padding: 14px 16px 0;
    flex: none;
  }
  .save-row .input {
    flex: 1;
  }
  .confirm-overwrite {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 10px 16px 0;
    padding: 10px 12px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
    flex: none;
  }
  .confirm-overwrite span {
    flex: 1;
  }
  .list {
    overflow-y: auto;
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .empty {
    color: var(--faint);
    font-size: 12px;
    padding: 12px 0;
    text-align: center;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    text-align: left;
  }
  .item:hover {
    background: var(--raised);
  }
  .item-b {
    flex: 1;
    min-width: 0;
  }
  .item-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-meta {
    font-size: 10px;
    color: var(--faint);
    margin-top: 2px;
  }
  .item-x {
    flex: none;
    color: var(--muted2);
    font-size: 12px;
    padding: 2px 4px;
  }
  .item-x:hover {
    color: var(--rosso-bright);
  }
</style>
