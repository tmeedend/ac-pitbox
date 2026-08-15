<script lang="ts">
  // Bloc « Décisions d'import » de la fiche détail (§4.6).
  //
  // L'app tranche seule tout ce qui est déterminable depuis le disque — c'est
  // son travail, et lui poser la question reviendrait à le rendre à
  // l'utilisateur. Mais une décision fausse et **silencieuse** est ce qui a
  // coûté le plus cher : un pilote posé dans `<AC>\driver\` au lieu de
  // `content/driver`, trois dossiers d'emballage déversés à la racine du jeu —
  // tout ça est resté invisible jusqu'à ce qu'on aille lire le disque à la
  // main. Ce bloc est la trace lisible de ces arbitrages.
  //
  // Vide pour la grande majorité des mods : le bloc disparaît alors
  // entièrement plutôt que d'afficher « aucune décision », qui n'apprend rien
  // et occupe une place sur toutes les fiches.
  import { listImportDecisions, type ImportJournalEntry } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";

  let { modId }: { modId: string } = $props();

  let decisions = $state<ImportJournalEntry[]>([]);

  // Même garde que les autres blocs : une réponse tardive d'un mod précédent
  // ne doit pas écraser la liste du mod courant.
  $effect(() => {
    const current = modId;
    decisions = [];
    listImportDecisions(current)
      .then((ds) => {
        if (current === modId) decisions = ds;
      })
      .catch(() => {});
  });

  /** Chaque nature de décision porte sa couleur sémantique : jaune = alerte
      (quelque chose n'ira pas dans le jeu), bleu = information. */
  function toneOf(kind: string): "warn" | "info" {
    return kind === "pathRefused" || kind === "ancillaryDropped" ? "warn" : "info";
  }
</script>

{#if decisions.length}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("detail.decisionsTitle")}</span>
      <span class="blk-n">{decisions.length}</span>
    </header>
    <div class="blk-b">
      <p class="note">{t("detail.decisionsNote")}</p>
      <ul class="dec-list">
        {#each decisions as d (d.kind + d.subject)}
          <li class="dec {toneOf(d.kind)}">
            <span class="dec-what">{t(`detail.decision.${d.kind}`)}</span>
            <span class="dec-subject mono">{d.subject}</span>
            {#if d.detail}
              <span class="dec-arrow mono">→</span>
              <span class="dec-detail mono">{d.detail}</span>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  </section>
{/if}

<style>
  /* Encadré et bandeau viennent des classes globales `.blk*` (global.css). */
  .note {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  .dec-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .dec {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px;
    border: 1px solid var(--line);
    border-left-width: 2px;
    background: var(--raised);
    padding: 7px 11px;
  }
  .dec.info {
    border-left-color: var(--blue-border);
  }
  .dec.warn {
    border-left-color: var(--yellow);
  }
  .dec-what {
    font-size: 11.5px;
    color: var(--txt2);
  }
  .dec.warn .dec-what {
    color: var(--yellow);
  }
  .dec-subject,
  .dec-detail {
    font-size: 10.5px;
    color: var(--muted2);
    overflow-wrap: anywhere;
  }
  .dec-detail {
    color: var(--txt2);
  }
  .dec-arrow {
    font-size: 10.5px;
    color: var(--muted);
  }
</style>
