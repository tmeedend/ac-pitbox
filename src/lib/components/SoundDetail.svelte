<script lang="ts">
  // Fiche d'un mod de son (§8).
  //
  // Même raison d'être que la fiche d'une app : les listes de fichiers y vivent,
  // pas dans un dépliant au milieu d'une liste. Et un mod de son a de quoi la
  // remplir — ce que son bank contient réellement, que personne d'autre ne sait
  // afficher parce qu'il faut décoder le conteneur pour le savoir.
  //
  // Rien n'est réinventé : `StateBadge`, `InlineEdit`, `ResourcesBlock`
  // et la clé de contact sont les composants de la fiche voiture.
  import { onMount } from "svelte";
  import { soundDetail, setSoundAuthor, type SoundDetail } from "$lib/enginesound";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import IgnitionKey from "./detail/IgnitionKey.svelte";
  import InlineEdit from "./InlineEdit.svelte";
  import StateBadge from "./StateBadge.svelte";
  import { engineState, toggleEngine, stopEngine } from "$lib/enginePlayer.svelte";

  interface Props {
    subId: string;
    onclose: () => void;
  }

  const { subId, onclose }: Props = $props();

  let detail = $state<SoundDetail | null>(null);
  let error = $state("");

  async function load() {
    try {
      detail = await soundDetail(subId);
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(load);

  // Quitter la fiche coupe le moteur : un son qui survit à l'écran qui le porte
  // ne peut plus être arrêté.
  $effect(() => {
    void subId;
    return () => stopEngine();
  });

  async function saveAuthor(value: string | null) {
    error = "";
    try {
      await setSoundAuthor(subId, value);
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function listen() {
    if (!detail) return;
    error = "";
    try {
      await toggleEngine(detail.parentId, detail.id);
    } catch (e) {
      error = errorText(e);
    }
  }

  /** Durée totale du bank, en minutes et secondes — des heures de son en
   * secondes ne se lisent pas. */
  function fmtDuration(seconds: number): string {
    const total = Math.round(seconds);
    const m = Math.floor(total / 60);
    const s = total % 60;
    return m > 0 ? `${m} min ${String(s).padStart(2, "0")} s` : `${s} s`;
  }
</script>

<div class="page">
  <header class="head">
    <button class="back" type="button" onclick={onclose}>{t("sounds.back")}</button>
    <h2 class="lbl-screen">{detail?.name ?? subId}</h2>
    {#if detail}
      <StateBadge active={detail.isActive} stock={false} />
      <div class="actions">
        <IgnitionKey state={engineState(detail.parentId, detail.id)} onclick={listen} />
      </div>
    {/if}
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if detail}
    <dl class="meta">
      <div>
        <dt class="lbl-key">{t("sounds.carLabel")}</dt>
        <dd>
          {detail.parentName ?? detail.parentId}
          <span class="mono dim">{detail.parentId}</span>
        </dd>
      </div>
      <div>
        <dt class="lbl-key">{t("sounds.sizeLabel")}</dt>
        <dd class="mono">{fmtSize(detail.sizeBytes)}</dd>
      </div>
      {#if detail.sourceArchive}
        <div>
          <dt class="lbl-key">{t("detail.sourceLabel")}</dt>
          <dd class="mono">{detail.sourceArchive}</dd>
        </div>
      {/if}
      <div>
        <dt class="lbl-key">{t("apps.importedAt")}</dt>
        <dd>{new Date(detail.importedAt).toLocaleString()}</dd>
      </div>
    </dl>

    <!-- L'auteur se saisit à la main : aucun fichier de mod ne le porte, et le
         lire dans une notice serait une devinette sur du texte libre. -->
    <section class="blk">
      <header class="blk-h"><span class="blk-t">{t("sounds.authorLabel")}</span></header>
      <div class="blk-b">
        <InlineEdit
          value={detail.author}
          original={null}
          overridden={detail.author != null}
          label={t("sounds.authorLabel")}
          placeholder={t("sounds.authorPlaceholder")}
          onsave={saveAuthor}
        />
      </div>
    </section>

    <section class="blk">
      <header class="blk-h"><span class="blk-t">{t("sounds.bankLabel")}</span></header>
      <div class="blk-b">
        {#if detail.bank}
          <dl class="meta">
            <div>
              <dt class="lbl-key">{t("sounds.bankFile")}</dt>
              <dd class="mono">{detail.bank.fileName}</dd>
            </div>
            <div>
              <dt class="lbl-key">{t("sounds.bankCodec")}</dt>
              <dd class="mono">{detail.bank.codec}</dd>
            </div>
            <div>
              <dt class="lbl-key">{t("sounds.bankSamples")}</dt>
              <dd class="mono">{detail.bank.sampleCount}</dd>
            </div>
            <div>
              <dt class="lbl-key">{t("sounds.bankRate")}</dt>
              <dd class="mono">{(detail.bank.frequency / 1000).toFixed(1)} kHz</dd>
            </div>
            <div>
              <dt class="lbl-key">{t("sounds.bankDuration")}</dt>
              <dd class="mono">{fmtDuration(detail.bank.seconds)}</dd>
            </div>
            <div>
              <dt class="lbl-key">{t("sounds.bankNames")}</dt>
              <dd>{detail.bank.named ? t("sounds.bankNamesPresent") : t("sounds.bankNamesAbsent")}</dd>
            </div>
          </dl>
        {:else}
          <p class="muted small">{t("sounds.bankUnreadable")}</p>
        {/if}
      </div>
    </section>

    <div class="body">
      <ResourcesBlock modId={detail.id} source="sound" onerror={(m) => (error = m)} />
    </div>
  {/if}
</div>

<style>
  .page {
    max-width: 860px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .head h2 {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .back {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 11.5px;
    color: var(--muted);
    cursor: pointer;
  }
  .back:hover,
  .back:focus-visible {
    color: var(--rosso-bright);
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 28px;
    margin-bottom: 14px;
  }
  .meta dd {
    font-size: 12px;
    color: var(--txt2);
    margin-top: 2px;
    overflow-wrap: anywhere;
  }
  .dim {
    color: var(--txt3);
    margin-left: 6px;
  }
  .body {
    margin-top: 14px;
  }
  .err {
    margin-bottom: 10px;
    padding: 8px 10px;
    border: 1px solid var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
</style>
