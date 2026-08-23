<script lang="ts">
  // Bloc « Historique » de la fiche détail (§3.2 / §6.2).
  //
  // Versions et historique ne font plus deux rubriques : une version EST un
  // événement du mod, la séparer obligeait à faire l'aller-retour entre deux
  // listes pour répondre à « d'où vient la version installée ». Une seule
  // frise, donc, où chaque entrée porte sa date, et où la version en place
  // est mise en avant.
  //
  // Purement présentationnel à deux exceptions près — activer une autre
  // version, et en supprimer une (§10) : deux actions réelles, déléguées
  // au parent (qui possède `busy`, la relecture de la fiche, la confirmation
  // et la bannière d'erreur).
  import type { ModDetail } from "$lib/library";
  import { historyEventLabel, historyDetails } from "$lib/history";
  import { fmtSize } from "$lib/format";
  import { t } from "$lib/i18n/index.svelte";

  let {
    detail,
    busy,
    onactivateversion,
    ondeleteversion,
  }: {
    detail: ModDetail;
    /** Une action est déjà en cours côté parent : les boutons se désactivent. */
    busy: boolean;
    onactivateversion: (versionId: string) => void;
    /** Supprimer une version rangée (§10). Jamais proposé sur la version
     * en place : ses fichiers sont ceux que le jeu utilise. */
    ondeleteversion: (versionId: string) => void;
  } = $props();

  /** Date+heure locales ; repli sur l'ISO tronqué si la chaîne est illisible. */
  function fmtDate(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso.slice(0, 16).replace("T", " ") : d.toLocaleString();
  }

  interface Entry {
    key: string;
    when: string;
    /** Renseigné pour une version ; absent pour un événement isolé. */
    versionId?: string;
    label: string;
    installed: boolean;
    /** Ce qui s'est passé : « import initial », « mise à jour »… */
    event: string;
    /** Taille sur disque de cette version (§9.4), `null` pour un événement
     * isolé ou une version importée avant que la taille ne soit calculée. */
    size: number | null;
    /** Archive dont provient la version, ou détail de l'événement. */
    detail: string;
  }

  const entries = $derived.by(() => {
    // Contenu de base Kunos (§4/§12bis.1) : pas de vraie notion de version ni
    // d'import à raconter ici — une seule ligne informative, pas la frise
    // habituelle avec « (sans n°) », badge « installée » et une date qui ne
    // correspond à rien de réel (juste l'indexation locale).
    if (detail.is_stock) {
      return [
        {
          key: "stock",
          when: "",
          label: "",
          installed: false,
          event: detail.is_unmanaged ? t("detail.unmanagedContentLabel") : t("detail.baseContentLabel"),
          detail: "",
          size: null,
        },
      ];
    }

    // Une version et son événement d'import partagent la même seconde : sans
    // ce dédoublonnage, la frise afficherait deux fois la même chose.
    const versionStamps = new Set(detail.versions.map((v) => v.imported_at));

    const fromVersions: Entry[] = detail.versions.map((v) => ({
      key: v.id,
      when: v.imported_at,
      versionId: v.id,
      label: v.version_label ?? t("detail.noVersionNumber"),
      installed: v.id === detail.active_version_id,
      event:
        historyEventLabel(
          detail.history.find((h) => h.timestamp === v.imported_at)?.event ?? "IMPORT",
        ),
      detail: v.source_archive ?? "",
      size: v.size_bytes,
    }));

    // Activer/désactiver n'est pas un événement de cycle de vie : ces lignes
    // pollueraient la frise sans rien apprendre (§3.2).
    const fromHistory: Entry[] = detail.history
      .filter(
        (h) =>
          h.event !== "ACTIVATE" && h.event !== "DEACTIVATE" && !versionStamps.has(h.timestamp),
      )
      .map((h, i) => ({
        key: `h${i}-${h.timestamp}`,
        when: h.timestamp,
        label: "",
        installed: false,
        event: historyEventLabel(h.event),
        detail: historyDetails(h.details),
        size: null,
      }));

    // Version installée en tête (c'est ce qu'on vient vérifier en premier),
    // le reste du plus récent au plus ancien.
    return [...fromVersions, ...fromHistory].sort((a, b) => {
      if (a.installed !== b.installed) return a.installed ? -1 : 1;
      return b.when.localeCompare(a.when);
    });
  });
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.historyLabel")}</span>
    <!-- La place que le mod prend sur le disque, toutes versions confondues —
         la même valeur que la colonne « Taille » de la bibliothèque, et le
         même formatage (`fmtSize`). Ici plutôt que dans une carte à elle :
         c'est une donnée qu'on consulte, pas qu'on suit. -->
    {#if detail.size_bytes != null}
      <span class="blk-n">{fmtSize(detail.size_bytes)}</span>
    {/if}
  </header>
  <div class="blk-b">
    <ul class="timeline">
      {#each entries as e (e.key)}
        <li class="entry" class:installed={e.installed}>
          <span class="dot" aria-hidden="true"></span>
          <div class="body">
            <div class="head">
              {#if e.label}<span class="ver mono">{e.label}</span>{/if}
              <!-- Décomposée par version, et pas seulement cumulée : depuis
                   qu'une version se supprime (§10), c'est le chiffre qui dit
                   laquelle vaut la peine d'être retirée. -->
              {#if e.size != null}<span class="size mono">{fmtSize(e.size)}</span>{/if}
              {#if e.installed}
                <span class="badge">{t("detail.installedBadge")}</span>
              {:else if e.versionId}
                <button
                  class="activate"
                  type="button"
                  disabled={busy}
                  onclick={() => onactivateversion(e.versionId!)}
                >
                  {t("common.activate")}
                </button>
                <button
                  class="trash"
                  type="button"
                  disabled={busy}
                  title={t("detail.deleteVersion")}
                  aria-label={t("detail.deleteVersion")}
                  onclick={() => ondeleteversion(e.versionId!)}
                >
                  🗑
                </button>
              {/if}
            </div>
            <div class="event">{e.event}</div>
            {#if e.detail}<div class="detail mono">{e.detail}</div>{/if}
            {#if e.when}<div class="when mono">{fmtDate(e.when)}</div>{/if}
          </div>
        </li>
      {/each}
    </ul>
  </div>
</section>

<style>
  /* Habillage propre au bloc. L'encadré et le bandeau viennent des classes
     globales `.blk*` (voir global.css). */
  .timeline {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .entry {
    display: flex;
    gap: 10px;
  }
  /* Pastille de frise : discrète par défaut, rouge pour la version en place. */
  .dot {
    flex: none;
    width: 9px;
    height: 9px;
    margin-top: 4px;
    border-radius: 50%;
    background: var(--faint);
  }
  .entry.installed .dot {
    background: var(--rosso);
  }
  .body {
    min-width: 0;
    flex: 1;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-bottom: 3px;
  }
  .ver {
    color: var(--txt);
    font-size: 14px;
    font-weight: 600;
  }
  .size {
    color: var(--muted2);
    font-size: 10.5px;
  }
  .badge {
    background: var(--rosso);
    color: #fff;
    font-family: var(--mono);
    font-size: 9px;
    letter-spacing: 1px;
    text-transform: uppercase;
    padding: 2px 7px;
  }
  .activate {
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 10px;
    padding: 2px 9px;
  }
  .activate:hover:not(:disabled) {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .activate:disabled {
    opacity: 0.5;
  }
  /* Rouge seulement au survol : la frise se parcourt pour lire l'historique,
     une colonne de pastilles rouges y crierait à chaque ligne. */
  .trash {
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted2);
    font-size: 11px;
    line-height: 1;
    padding: 3px 6px;
  }
  .trash:hover:not(:disabled) {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .trash:disabled {
    opacity: 0.5;
  }
  .event {
    color: var(--txt2);
    font-size: 12px;
  }
  .detail {
    color: var(--muted);
    font-size: 11px;
    margin-top: 2px;
    overflow-wrap: anywhere;
  }
  .when {
    color: var(--muted2);
    font-size: 10.5px;
    margin-top: 3px;
  }
</style>
