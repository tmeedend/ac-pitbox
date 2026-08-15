<script lang="ts">
  // Corps du rapport d'import (§4.2bis), sans son cadre : rendu à l'identique
  // dans le toast de fin (ImportOverlay) et sur l'écran Import, qui garde le
  // dernier rapport consultable. Extrait justement pour ça — deux copies du
  // même balisage auraient divergé dès la première retouche.
  import { nav, requestSection } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { activateOther } from "$lib/others";
  import { errorText } from "$lib/errors";
  import type { ArchiveResult, OtherImported, SubImported } from "$lib/library";

  interface Props {
    report: ArchiveResult[];
    /** Appelé juste avant d'ouvrir une fiche : le toast s'en sert pour se
     * fermer, sans quoi il recouvrirait la fiche qu'on vient d'ouvrir. */
    onnavigate?: () => void;
  }
  const { report, onnavigate }: Props = $props();

  // Libellé + classe CSS de la pastille d'issue d'un mod importé.
  function outcomeChip(o: string): { cls: string; label: string } {
    if (o === "UPDATE_REPLACE") return { cls: "upd", label: t("importOverlay.outcomeUpdate") };
    if (o === "DUPLICATE") return { cls: "dup", label: t("importOverlay.outcomeDuplicate") };
    if (o === "EXTENSION") return { cls: "ext", label: t("importOverlay.outcomeExtension") };
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

  /** Skins et sons regroupés par contenu parent : un pack de quarante livrées
   * ferait déborder le rapport à raison d'une ligne chacune, alors qu'il n'y a
   * qu'une seule fiche à ouvrir au bout. */
  function subsByParent(subs: SubImported[]): { id: string; kind: string; skins: number; sounds: number }[] {
    const groups = new Map<string, { id: string; kind: string; skins: number; sounds: number }>();
    for (const s of subs) {
      const g = groups.get(s.parent_id) ?? {
        id: s.parent_id,
        kind: s.sub_type === "TRACK_SKIN" ? "Track" : "Car",
        skins: 0,
        sounds: 0,
      };
      if (s.sub_type === "SOUND") g.sounds++;
      else g.skins++;
      groups.set(s.parent_id, g);
    }
    return [...groups.values()];
  }
</script>

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
         ouvrir tant que l'utilisateur n'a pas tranché. -->
    {@const openable = m.outcome !== "AMBIGUOUS"}
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
      {:else if m.outcome === "EXTENSION"}
        <span class="r-conflict">{t("importOverlay.extensionNote", { added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
      {/if}
    </div>
  {/each}
  {#each subsByParent(a.subs ?? []) as g}
    <div class="r-line shared">
      {t("importOverlay.subsAttached", {
        parts: `${g.skins ? t("importOverlay.skinCount", { count: g.skins }) : ""}${g.skins && g.sounds ? " · " : ""}${g.sounds ? t("importOverlay.soundCount", { count: g.sounds }) : ""}`,
      })}
      <button class="r-open" type="button" onclick={() => openContent(g.id, g.kind)}>{g.id}</button>
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
