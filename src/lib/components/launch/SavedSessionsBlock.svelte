<script lang="ts">
  // Bloc « Sessions enregistrées » de l'écran Lancement (§8.4bis) : liste du
  // type courant, chargée au clic ; Sauvegarder ouvre la popup de nommage
  // (co-localisée ici, comme le picker d'adversaire dans OpponentsBlock) —
  // l'état d'ouverture et les actions réelles (sauvegarder/charger, qui
  // touchent `setup` au niveau de l'écran entier) restent orchestrés par
  // Launch.svelte.
  import type { SessionType } from "$lib/launch";
  import type { SavedSession } from "$lib/savedSessions";
  import { t } from "$lib/i18n/index.svelte";
  import SavedSessionsDialog from "../SavedSessionsDialog.svelte";

  let {
    sessionType,
    savedList,
    dialogOpen,
    onopendialog,
    onclosedialog,
    onsave,
    onload,
  }: {
    sessionType: SessionType;
    savedList: SavedSession[];
    dialogOpen: boolean;
    onopendialog: () => void;
    onclosedialog: () => void;
    onsave: (name: string) => void;
    onload: (s: SavedSession) => void;
  } = $props();

  function fmtSavedAt(iso: string): string {
    return iso.slice(0, 16).replace("T", " ");
  }
</script>

{#if dialogOpen}
  <SavedSessionsDialog {sessionType} {onsave} onclose={onclosedialog} />
{/if}

<!-- Sessions enregistrées (§8.4bis) : liste du type courant, mise à jour par
     l'effet qui alimente `savedList` côté parent ; charge au clic,
     Sauvegarder ouvre la popup de nommage (avec écrasement d'une sauvegarde
     existante en option). -->
<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("launch.savedSessionsLabel")}</span>
    <span class="blk-n">{savedList.length}</span>
  </header>
  <div class="blk-b">
    <button class="btn saved-save-btn" type="button" onclick={onopendialog}>{t("launch.saveSession")}</button>
    <div class="saved-list">
      {#if !savedList.length}
        <div class="saved-empty">{t("launch.noSavedSessions")}</div>
      {:else}
        {#each savedList as s (s.name)}
          <button class="saved-item" type="button" onclick={() => onload(s)}>
            <div class="saved-item-b">
              <div class="saved-item-name">{s.name}</div>
              <div class="saved-item-meta mono">{fmtSavedAt(s.savedAt)}</div>
            </div>
          </button>
        {/each}
      {/if}
    </div>
  </div>
</section>

<style>
  /* Sessions enregistrées (§8.4bis) */
  .saved-save-btn {
    width: 100%;
    margin-bottom: 12px;
  }
  /* Hauteur plafonnée + défilement propre : la carte ne doit pas grandir
     sans limite si l'utilisateur accumule des sauvegardes. */
  .saved-list {
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--line);
  }
  .saved-empty {
    padding: 14px 10px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
  }
  .saved-item {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 8px 10px;
    background: var(--panel2);
    border-bottom: 1px solid var(--line);
    text-align: left;
  }
  .saved-item:last-child {
    border-bottom: none;
  }
  .saved-item:hover {
    background: var(--raised);
  }
  .saved-item-b {
    flex: 1;
    min-width: 0;
  }
  .saved-item-name {
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .saved-item-meta {
    font-size: 10px;
    color: var(--muted);
    margin-top: 2px;
  }
</style>
