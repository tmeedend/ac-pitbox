<script lang="ts">
  // Onglet Médias — sous-vue Replays (§6.1). « Lire dans CM » passe le
  // chemin du .acreplay en argument à l'exécutable Content Manager, comme
  // l'association de fichier Windows le ferait au double-clic (même
  // mécanisme que launch()/open_content_manager, voir launch.rs). Le bouton
  // corbeille (et la touche Suppr sur la ligne focalisée) envoie le fichier à
  // la corbeille Windows — récupérable, donc sans confirmation.
  //
  // Ce qu'une ligne montre vient de trois endroits : le nom du fichier (type
  // de session, autosave ou non), le disque (taille, date de repli) et
  // l'en-tête du replay lui-même (pilote, durée, voitures en piste) — voir
  // `acreplay.rs`. Le nom seul ne distingue pas deux courses du même combo.
  import {
    listMediaReplays,
    linkMediaManually,
    openMediaFolder,
    launchReplay,
    trashMediaFile,
    onSessionEnd,
    type ReplayFile,
  } from "$lib/detail/media";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { errorText } from "$lib/errors";
  import { fmtDuration, fmtSize } from "$lib/format";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    onerror,
    oncount,
  }: {
    modId: string;
    onerror: (message: string) => void;
    /** Remonte le décompte au bandeau de l'onglet, qui l'a chargé de son côté
     * à l'ouverture de la fiche : sans ça il resterait sur l'ancien chiffre
     * après une course. */
    oncount?: (n: number) => void;
  } = $props();

  let files = $state<ReplayFile[]>([]);
  let linking = $state(false);
  let launching = $state<string | null>(null);
  let trashing = $state(false);

  async function reload() {
    const current = modId;
    const f = await listMediaReplays(current);
    if (current !== modId) return;
    files = f;
    oncount?.(f.length);
  }

  $effect(() => {
    const current = modId;
    files = [];
    listMediaReplays(current).then((f) => {
      if (current !== modId) return;
      files = f;
      oncount?.(f.length);
    });
  });

  onMount(() => {
    // La fin d'une course est le seul moment où cette liste change sans qu'on
    // ait rien fait dans l'app : la fiche pouvait rester ouverte sur une liste
    // d'avant la course tant qu'on ne changeait pas de mod.
    let stop: (() => void) | null = null;
    void onSessionEnd(() => void reload().catch(() => {})).then((off) => (stop = off));
    return () => stop?.();
  });

  function fmtDate(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? "—" : d.toLocaleString();
  }

  /** Le combo pilotée/circuit tel que l'en-tête du replay le donne — pas le
   * nom du fichier, qui colle les deux id sans séparateur. */
  function combo(f: ReplayFile): string {
    const track = f.track_layout ? `${f.track_id} · ${f.track_layout}` : f.track_id;
    return [f.car_id, track].filter(Boolean).join("  —  ") || f.file_name;
  }

  /** Ce que devient le fichier : soit AC le fera tourner avec les autres
   * autosaves de son type, soit il a été renommé et reste. */
  function rotation(f: ReplayFile): string | null {
    if (!f.autosave) return t("detail.replayKept");
    if (f.autosave_rank == null || f.autosave_limit == null) return t("detail.replayAutosave");
    return t("detail.replayAutosaveRank", { rank: f.autosave_rank, limit: f.autosave_limit });
  }

  function rotationHint(f: ReplayFile): string {
    if (!f.autosave) return t("detail.replayKeptHint");
    const type =
      f.session_type === "R"
        ? t("detail.replaySessionRace")
        : f.session_type === "Q"
          ? t("detail.replaySessionQualify")
          : t("detail.replaySessionOther");
    return t("detail.replayAutosaveHint", { limit: f.autosave_limit ?? "?", type });
  }

  async function openFolder() {
    try {
      await openMediaFolder("REPLAY");
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function playReplay(path: string) {
    if (launching) return;
    launching = path;
    try {
      await launchReplay(path);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      launching = null;
    }
  }

  // Retrait local plutôt que rechargement : le backend a déjà retiré le
  // rattachement manuel du fichier, un rechargement donnerait la même liste.
  async function trashReplay(f: ReplayFile) {
    if (trashing) return;
    trashing = true;
    try {
      await trashMediaFile(f.path);
      files = files.filter((x) => x.path !== f.path);
      oncount?.(files.length);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      trashing = false;
    }
  }

  async function linkManually() {
    if (linking) return;
    const picked = await open({
      multiple: false,
      title: t("detail.mediaLinkPickTitle"),
      filters: [{ name: "Replay", extensions: ["acreplay"] }],
    });
    if (!picked || typeof picked !== "string") return;
    linking = true;
    try {
      await linkMediaManually(modId, "REPLAY", picked);
      await reload();
    } catch (e) {
      onerror(errorText(e));
    } finally {
      linking = false;
    }
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.mediaReplaysTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <ul class="replay-list">
        {#each files as f (f.path)}
          <!-- Ligne focalisable pour que Suppr ait une cible : un clic
               n'importe où dessus la désigne, et le contour rouge dit
               laquelle. Le linter a11y ne connaît pas ce motif sur un `li`
               (rôle non interactif) alors qu'il n'y a rien à corriger ici :
               la ligne reste atteignable au clavier et ses deux actions
               restent de vrais boutons. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <li
            class="replay"
            tabindex="0"
            onkeydown={(e) => {
              if (e.key === "Delete") {
                e.preventDefault();
                trashReplay(f);
              }
            }}
          >
            <div class="replay-b">
              <div class="replay-head">
                <span class="replay-combo mono">{combo(f)}</span>
                {#if rotation(f)}
                  <span class="pill" class:kept={!f.autosave} title={rotationHint(f)}>{rotation(f)}</span>
                {/if}
              </div>
              <div class="replay-meta mono">
                <span>{fmtDate(f.recorded_at)}</span>
                {#if f.duration_s != null}<span>{fmtDuration(f.duration_s)}</span>{/if}
                {#if f.cars_number != null}<span>{t("detail.replayCars", { count: f.cars_number })}</span>{/if}
                {#if f.driver_name}<span>{f.driver_name}</span>{/if}
                <span>{fmtSize(f.size_bytes)}</span>
                {#if f.matched_counterpart}<span>{f.matched_counterpart}</span>{/if}
              </div>
              <div class="replay-name mono">{f.file_name}</div>
            </div>
            <button class="btn-ghost play" type="button" onclick={() => playReplay(f.path)} disabled={launching === f.path}>
              {launching === f.path ? t("common.working") : t("detail.playReplay")}
            </button>
            <button
              class="replay-del"
              type="button"
              title={t("detail.mediaTrash")}
              disabled={trashing}
              onclick={() => trashReplay(f)}
            >
              🗑
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">{t("detail.noReplays")}</p>
    {/if}
    <div class="actions">
      <button class="btn-ghost" type="button" onclick={openFolder}>{t("detail.openMediaFolder")}</button>
      <button class="btn-ghost" type="button" onclick={linkManually} disabled={linking}>
        {t("detail.mediaLinkManually")}
      </button>
    </div>
  </div>
</section>

<style>
  .replay-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .replay {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--line);
    background: var(--raised);
    padding: 8px 11px;
  }
  /* `:focus` et pas `:focus-visible` : la ligne se focalise aussi au clic, et
     c'est justement là qu'il faut montrer sur quoi Suppr va agir. */
  .replay:focus {
    border-color: var(--rosso-border);
    outline: none;
  }
  .replay-b {
    flex: 1;
    min-width: 0;
  }
  .replay-head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .replay-combo {
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Le gardé se distingue de l'autosave sans couleur d'alerte : ce n'est pas
     un défaut, juste un état. */
  .pill.kept {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .replay-del {
    flex: none;
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
    padding: 6px 7px;
  }
  .replay-del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .replay .play {
    flex: none;
    font-size: 11px;
    padding: 6px 10px;
  }
  .replay-name {
    font-size: 10px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 3px;
  }
  .replay-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    color: var(--muted2);
    font-size: 10px;
    margin-top: 3px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
