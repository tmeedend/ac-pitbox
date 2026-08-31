<script lang="ts">
  // Corps du rapport d'import (§4.2bis), sans son cadre : rendu à l'identique
  // dans le toast de fin (ImportToasts) et sur l'écran Import, qui garde le
  // dernier rapport consultable. Extrait justement pour ça — deux copies du
  // même balisage auraient divergé dès la première retouche.
  import { nav, requestSection } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { importState, openPendingDialog, refreshPendingCount } from "$lib/importState.svelte";
  import { activateOther } from "$lib/others";
  import { errorText } from "$lib/errors";
  import type { ArchiveResult, OtherImported, SubImported } from "$lib/library";

  interface Props {
    report: ArchiveResult[];
    /** Appelé juste avant d'ouvrir une fiche : le toast s'en sert pour se
     * REPLIER, sans quoi il recouvrirait la fiche qu'on vient d'ouvrir. Il se
     * fermait, avant — et on ouvre souvent plusieurs mods du même lot l'un
     * après l'autre, sans aucun chemin pour revenir au rapport. */
    onnavigate?: () => void;
  }
  const { report, onnavigate }: Props = $props();

  // Libellé + classe CSS de la pastille d'issue d'un mod importé.
  function outcomeChip(o: string): { cls: string; label: string } {
    if (o === "UPDATE_REPLACE") return { cls: "upd", label: t("importOverlay.outcomeUpdate") };
    if (o === "DUPLICATE") return { cls: "dup", label: t("importOverlay.outcomeDuplicate") };
    if (o === "EXTENSION") return { cls: "ext", label: t("importOverlay.outcomeExtension") };
    if (o === "UNMANAGED") return { cls: "unm", label: t("importOverlay.outcomeUnmanaged") };
    if (o === "PARKED") return { cls: "ext", label: t("importOverlay.outcomeParked") };
    if (o === "HOST_MISSING" || o === "HOST_UNKNOWN")
      return { cls: "unm", label: t("importOverlay.outcomeHostMissing") };
    return { cls: "new", label: t("importOverlay.outcomeNew") };
  }

  /** Ouvre la fiche d'un contenu. Pour une couche, `id` est déjà celui du
   * contenu de base (§4.4) : c'est donc lui qui s'ouvre. */
  async function openContent(id: string, kind: string): Promise<void> {
    onnavigate?.();
    if (await requestSection(kind === "Track" ? "tracks" : "cars")) nav.openMod = id;
  }

  async function openSection(section: string): Promise<void> {
    onnavigate?.();
    await requestSection(section);
  }

  // --- Composants optionnels (§4.6bis) ------------------------------------
  //
  // Livrés par l'auteur dans une archive à part **et** modifiant le jeu de
  // base : l'app les a rangés sans les activer, parce qu'aucun des deux
  // défauts n'est sûr. La question est posée ici, en fin de lot — jamais
  // pendant, un import de cinquante mods ne doit pas s'interrompre.
  //
  // Ne rien décider est une réponse valable : le composant reste en
  // bibliothèque, activable depuis l'écran « Autres mods » quand on veut.
  const optionals = $derived<OtherImported[]>(
    report.flatMap((a) => (a.others ?? []).filter((o) => o.optional)),
  );

  /** État local par composant — le rapport vit en mémoire, ces choix aussi. */
  let settled = $state<Record<string, "installed" | "skipped">>({});
  let installing = $state<string | null>(null);
  let optionalError = $state<string | null>(null);

  async function install(o: OtherImported) {
    installing = o.id;
    optionalError = null;
    try {
      await activateOther(o.id);
      settled = { ...settled, [o.id]: "installed" };
    } catch (e) {
      optionalError = errorText(e);
    } finally {
      installing = null;
    }
  }

  // --- Dossiers proposés (§4.6ter) ----------------------------------------
  //
  // Le rapport n'en garde qu'un bandeau : la décision se prend dans une modale
  // (`PendingDialog`), parce qu'elle demande un titre, une description libre,
  // une notice, un avertissement et jusqu'à quatre réponses — de quoi rendre
  // la pile de notifications, large de 380 px, illisible.
  //
  // Le compte vit dans `importState`, partagé avec la modale : deux compteurs
  // locaux avaient divergé dès le premier arbitrage.
  $effect(() => {
    void report;
    void refreshPendingCount();
  });

  /** Skins et sons regroupés par contenu parent : un pack de quarante livrées
   * ferait déborder le rapport à raison d'une ligne chacune, alors qu'il n'y a
   * qu'une seule fiche à ouvrir au bout. */
  function subsByParent(
    subs: SubImported[],
  ): { id: string; kind: string; skins: number; sounds: number; orphan: boolean }[] {
    const groups = new Map<string, { id: string; kind: string; skins: number; sounds: number; orphan: boolean }>();
    for (const s of subs) {
      const g = groups.get(s.parent_id) ?? {
        id: s.parent_id,
        kind: s.sub_type === "TRACK_SKIN" ? "Track" : "Car",
        skins: 0,
        sounds: 0,
        // Hôte absent (§4.3bis) : rangé sous l'id visé, mais rien n'est posé
        // dans le jeu. C'était le cas le plus silencieux de l'import — le
        // backend le savait, aucun écran ne le disait.
        orphan: false,
      };
      if (s.sub_type === "SOUND") g.sounds++;
      else g.skins++;
      if (!s.parent_known) g.orphan = true;
      groups.set(s.parent_id, g);
    }
    return [...groups.values()];
  }
</script>

{#if importState.pendingCount}
  <!-- Une ligne, pas une section : la question se pose dans la modale, qui
       s'ouvre toute seule en fin de lot. Cette ligne sert à y revenir. -->
  <button class="pend-bar" type="button" onclick={openPendingDialog}>
    <span class="pend-bar-l">{t("importOverlay.pendingBarLine", { count: importState.pendingCount })}</span>
    <span class="pend-bar-a">{t("importOverlay.pendingOpen")}</span>
  </button>
{/if}

{#if optionals.length}
  <!-- En tête du rapport : c'est la seule chose qui attend une réponse. -->
  <section class="opt">
    <div class="opt-h">{t("importOverlay.optionalTitle")}</div>
    <p class="opt-note">{t("importOverlay.optionalNote")}</p>
    {#each optionals as o (o.id)}
      <div class="opt-row">
        <span class="opt-nm mono">{o.id}</span>
        <span class="opt-n">{t("importOverlay.optionalReplaces", { count: o.game_files_replaced ?? 0 })}</span>
        {#if settled[o.id] === "installed"}
          <span class="opt-done">{t("importOverlay.optionalInstalled")}</span>
        {:else if settled[o.id] === "skipped"}
          <span class="opt-skip">{t("importOverlay.optionalSkipped")}</span>
        {:else}
          <button class="btn" type="button" disabled={installing === o.id} onclick={() => install(o)}>
            {t("importOverlay.optionalInstall")}
          </button>
          <button
            class="btn ghost"
            type="button"
            onclick={() => (settled = { ...settled, [o.id]: "skipped" })}
          >
            {t("importOverlay.optionalSkip")}
          </button>
        {/if}
      </div>
    {/each}
    {#if optionalError}<p class="opt-err">{optionalError}</p>{/if}
  </section>
{/if}

{#each report as a}
  {#if a.error}
    <div class="r-line err">⚠ {a.archive} — {a.error}</div>
  {/if}
  {#each a.mods as m}
    {@const chip = outcomeChip(m.outcome)}
    <!-- Un mod resté AMBIGU n'a rien écrit (§4.4) : il n'y a pas de fiche à
         ouvrir tant que l'utilisateur n'a pas tranché. Même chose pour un
         fragment (§4.3bis) : écarté, il n'a rien écrit ; gardé en attente, la
         fiche qu'il faudrait ouvrir est celle d'un mod qui n'existe pas encore. -->
    {@const openable = !["AMBIGUOUS", "PARKED", "HOST_MISSING", "HOST_UNKNOWN"].includes(m.outcome)}
    <div class="r-line">
      <span class="r-out {chip.cls}">{chip.label}</span>
      {#if openable}
        <button class="r-open" type="button" onclick={() => openContent(m.id_interne, m.kind)}>
          {m.display_name ?? m.id_interne}
        </button>
      {:else}
        {m.display_name ?? m.id_interne}
      {/if}
      {#if m.outcome === "DUPLICATE"}
        <span class="r-conflict">{t("importOverlay.duplicateNote")}</span>
      {:else if m.outcome === "EXTENSION" && m.source_name}
        <!-- Fragment rattaché (§4.3bis) : la décision a été prise sans rien
             demander, elle doit donc se lire ici — quoi, et sur quoi. -->
        <span class="r-conflict">{t("importOverlay.extensionFromNote", { source: m.source_name, added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
      {:else if m.outcome === "EXTENSION"}
        <span class="r-conflict">{t("importOverlay.extensionNote", { added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
      {:else if m.outcome === "UNMANAGED"}
        <span class="r-conflict">{t("importOverlay.unmanagedNote")}</span>
      {:else if m.outcome === "PARKED"}
        <span class="r-conflict">{t("importOverlay.parkedNote", { host: m.host_id ?? "" })}</span>
      {:else if m.outcome === "HOST_MISSING" || m.outcome === "HOST_UNKNOWN"}
        <span class="r-conflict">{t("importOverlay.hostMissingNote")}</span>
      {:else if m.fragment}
        <!-- Importé comme mod alors qu'il n'a pas de géométrie : le seul cas où
             l'entrée créée risque de ne rien donner en jeu. -->
        <span class="r-conflict">{t("importOverlay.fragmentImportedNote")}</span>
      {/if}
    </div>
  {/each}
  {#each subsByParent(a.subs ?? []) as g}
    <div class="r-line shared">
      {t("importOverlay.subsAttached", {
        parts: `${g.skins ? t("importOverlay.skinCount", { count: g.skins }) : ""}${g.skins && g.sounds ? " · " : ""}${g.sounds ? t("importOverlay.soundCount", { count: g.sounds }) : ""}`,
      })}
      {#if g.orphan}
        <!-- Pas de fiche à ouvrir : l'hôte n'est pas dans la bibliothèque. -->
        <span class="mono">{g.id}</span>
        <span class="r-conflict">{t("importOverlay.subHostMissingNote")}</span>
      {:else}
        <button class="r-open" type="button" onclick={() => openContent(g.id, g.kind)}>{g.id}</button>
      {/if}
    </div>
  {/each}
  {#if (a.apps ?? []).length}
    <div class="r-line shared">
      <button class="r-open" type="button" onclick={() => openSection("apps")}>
        {t("importOverlay.appsImported", { count: a.apps.length })}
      </button>
    </div>
  {/if}
  {#if (a.others ?? []).length}
    <div class="r-line shared">
      <button class="r-open" type="button" onclick={() => openSection("others")}>
        {t("importOverlay.othersImported", { count: a.others.length })}
      </button>
    </div>
  {/if}
  {@const resExtracted =
    a.mods.reduce((acc, m) => acc + (m.resources_extracted ?? 0), 0) +
    (a.subs ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
    (a.apps ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
    (a.others ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0)}
  {#if resExtracted > 0}
    <div class="r-line shared">{t("importOverlay.resourcesExtracted", { count: resExtracted })}</div>
  {/if}
  {@const extras = a.extras ?? 0}
  {#if extras > 0}
    <div class="r-line shared">{t("importOverlay.extrasAttached", { count: extras })}</div>
  {/if}
{/each}

<style>
  /* Dossiers proposés (§4.6ter) : un bandeau, pas une section. Bleu comme
     partout ailleurs pour l'information — ce n'est ni une alerte ni une
     erreur, c'est une question qui attend. */
  .pend-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 8px 10px;
    margin-bottom: 10px;
    background: var(--raised);
    border: 1px solid var(--blue-border);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .pend-bar:hover,
  .pend-bar:focus-visible {
    border-color: var(--blue);
  }
  .pend-bar-l {
    font-size: 11.5px;
    color: var(--txt2);
  }
  .pend-bar-a {
    font-size: 11px;
    color: var(--blue);
    white-space: nowrap;
  }

  /* Composants optionnels (§4.6bis). Jaune = alerte : ce n'est ni une erreur
     ni une action destructive, c'est la seule chose du rapport qui attend une
     réponse. Encadré plutôt que fondu dans les lignes, sans quoi la question
     se lirait comme un compte rendu de plus. */
  .opt {
    border: 1px solid var(--yellow);
    background: var(--raised);
    padding: 9px 11px;
    margin-bottom: 10px;
  }
  .opt-h {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--yellow);
    margin-bottom: 4px;
  }
  .opt-note {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .opt-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 3px 0;
  }
  .opt-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .opt-n {
    font-size: 10.5px;
    color: var(--yellow);
    white-space: nowrap;
  }
  .opt-done,
  .opt-skip {
    font-size: 10.5px;
    white-space: nowrap;
  }
  .opt-done {
    color: var(--green);
  }
  .opt-skip {
    color: var(--muted2);
  }
  .opt-err {
    margin-top: 6px;
    font-size: 11px;
    color: var(--rosso-bright);
  }
  .r-line {
    padding: 2px 0;
    color: var(--txt2);
  }
  .r-line.err {
    color: var(--rosso-bright);
  }
  .r-line.shared {
    color: var(--muted);
    font-size: 11px;
    padding-top: 4px;
  }
  .r-out {
    font-size: 9px;
    letter-spacing: 0.5px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    margin-right: 6px;
  }
  .r-out.new {
    color: var(--green);
    border-color: var(--green-border);
  }
  .r-out.upd {
    color: var(--yellow);
    border-color: #4a4426;
  }
  .r-out.dup {
    color: var(--muted);
    border-color: var(--line);
  }
  .r-out.ext {
    color: var(--green);
    border-color: var(--green-border);
  }
  /* Rien n'a été écrit : le gris des mods non gérés, pas le vert de ce qui a
     été rangé quelque part. */
  .r-out.unm {
    color: var(--muted);
    border-color: var(--line);
  }
  .r-conflict {
    color: var(--yellow);
    margin-left: 6px;
    font-size: 11px;
  }
  /* Nom cliquable : ouvre la fiche du contenu. Bouton et non `<span onclick>` —
     le rapport doit rester atteignable au clavier et à la manette. */
  .r-open {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--txt);
    text-align: left;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .r-open:hover,
  .r-open:focus-visible {
    color: var(--rosso-bright);
    text-decoration-color: currentColor;
  }
  .r-line.shared .r-open {
    color: var(--muted);
    font-size: 11px;
  }
  .r-line.shared .r-open:hover,
  .r-line.shared .r-open:focus-visible {
    color: var(--rosso-bright);
  }
</style>
