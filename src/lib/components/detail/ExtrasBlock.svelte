<script lang="ts">
  // Bloc « Ajouts au jeu » de la fiche détail (§4.5.5) : ce que le mod
  // installe hors de son dossier `content/<type>/<id>` — configs CSP, shaders,
  // pilote, fonts. C'est la réponse à « qu'est-ce que ce mod met chez moi en
  // plus de son dossier ? », que rien ne montrait jusqu'ici.
  //
  // Regroupé par dossier de destination : 69 lignes plates sont illisibles,
  // alors que quatre destinations disent tout de suite ce que le mod touche.
  import { listModExtras, type ExtraFile } from "$lib/library";
  import { listAppExtras } from "$lib/apps";
  import { listPackExtras } from "$lib/packs";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    source = "mod",
  }: {
    modId: string;
    /** Une app pose ses ajouts au jeu exactement comme une voiture (§4.5.3) :
     * même arbre, même arbitrage, même affichage. Seul le chemin de résolution
     * côté backend diffère. Un **pack** aussi (§4.4) — ce sont même les seuls
     * ajouts au jeu que rien n'affichait avant sa fiche : `listModExtras` ne
     * regarde que `extras/<type>/<id>`, jamais `extras/packs/<nom>`. */
    source?: "mod" | "app" | "pack";
  } = $props();

  let files = $state<ExtraFile[]>([]);

  // Même garde que ResourcesBlock : une réponse tardive d'un mod précédent ne
  // doit pas écraser la liste du mod courant.
  $effect(() => {
    const current = modId;
    files = [];
    const load =
      source === "app" ? listAppExtras : source === "pack" ? listPackExtras : listModExtras;
    load(current).then((fs) => {
      if (current === modId) files = fs;
    });
  });

  /** Taille lisible (base 1024), même présentation que le bloc Ressources. */
  function fmtFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} o`;
    const units = ["Ko", "Mo", "Go"];
    let v = bytes;
    let i = -1;
    do {
      v /= 1024;
      i++;
    } while (v >= 1024 && i < units.length - 1);
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
  }

  interface Group {
    dir: string;
    files: ExtraFile[];
    size: number;
    /** Fichiers du groupe qu'un autre mod fournit — fichiers partagés (§4.5.4). */
    shared: number;
    /** Fichiers du groupe qui remplacent un fichier du jeu (§4.5.4). */
    replaced: number;
    /** Fichiers dont le chemin n'en est pas un pour AC : jamais posés (§4.5.3). */
    offPath: number;
    /** Fichiers en zone auto-gérée par un outil externe, typiquement CM (§4.5.3). */
    managed: number;
    /** Fichiers qu'un fichier étranger occupe déjà dans le jeu (§4.5.4). */
    foreign: number;
  }

  // Groupe = dossier parent. Un fichier posé à la racine d'AC (rare) tombe
  // dans un groupe au libellé vide plutôt que d'être perdu.
  let groups = $derived.by<Group[]>(() => {
    const by = new Map<string, ExtraFile[]>();
    for (const f of files) {
      const i = f.rel_path.lastIndexOf("/");
      const dir = i < 0 ? "" : f.rel_path.slice(0, i + 1);
      const list = by.get(dir);
      if (list) list.push(f);
      else by.set(dir, [f]);
    }
    return [...by.entries()]
      .map(([dir, fs]) => ({
        dir,
        files: fs,
        size: fs.reduce((a, f) => a + f.size_bytes, 0),
        shared: fs.filter((f) => f.provided_by !== null).length,
        replaced: fs.filter((f) => f.replaces_game_file).length,
        offPath: fs.filter((f) => f.off_game_path).length,
        managed: fs.filter((f) => f.externally_managed).length,
        foreign: fs.filter((f) => f.held_by_foreign_file).length,
      }))
      .sort((a, b) => a.dir.localeCompare(b.dir));
  });

  let openDirs = $state<Record<string, boolean>>({});
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.extrasTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <p class="note">{t("detail.extrasNote")}</p>
      {#if groups.some((g) => g.managed)}
        <p class="warn">{t("detail.extrasManagedNote")}</p>
      {/if}
      <ul class="grp-list">
        {#each groups as g (g.dir)}
          <li>
            <button class="grp-row" type="button" onclick={() => (openDirs[g.dir] = !openDirs[g.dir])}>
              <span class="grp-caret mono">{openDirs[g.dir] ? "−" : "+"}</span>
              <span class="grp-dir mono">{g.dir || "/"}</span>
              {#if g.offPath}
                <span class="grp-offpath">{t("detail.extrasOffPath", { count: g.offPath })}</span>
              {/if}
              {#if g.foreign}
                <span class="grp-foreign">{t("detail.extrasForeign", { count: g.foreign })}</span>
              {/if}
              {#if g.managed}
                <span class="grp-managed">{t("detail.extrasManaged", { count: g.managed })}</span>
              {/if}
              {#if g.replaced}
                <span class="grp-replaced">{t("detail.extrasReplaced", { count: g.replaced })}</span>
              {/if}
              {#if g.shared}
                <span class="grp-shared">{t("detail.extrasShared", { count: g.shared })}</span>
              {/if}
              <span class="grp-n">{t("detail.extrasFileCount", { count: g.files.length })}</span>
              <span class="grp-size mono">{fmtFileSize(g.size)}</span>
            </button>
            {#if openDirs[g.dir]}
              <ul class="file-list">
                {#each g.files as f (f.rel_path)}
                  <li class="file-row">
                    <span class="file-nm">{f.rel_path.slice(g.dir.length)}</span>
                    {#if f.off_game_path}
                      <span class="file-offpath">{t("detail.extrasOffPathFile")}</span>
                    {/if}
                    {#if f.held_by_foreign_file}
                      <span class="file-foreign" title={t("detail.extrasForeignHint")}
                        >{t("detail.extrasForeignFile")}</span
                      >
                    {/if}
                    {#if f.externally_managed}
                      <span class="file-managed" title={t("detail.extrasManagedHint")}
                        >{t("detail.extrasManagedFile")}</span
                      >
                    {/if}
                    {#if f.replaces_game_file}
                      <span class="file-replaced">{t("detail.extrasReplacesGameFile")}</span>
                    {/if}
                    {#if f.provided_by}
                      <span class="file-by" title={f.provided_by}>{t("detail.extrasProvidedBy", { mod: f.provided_by })}</span>
                    {/if}
                    <span class="file-size mono">{fmtFileSize(f.size_bytes)}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">{t("detail.noExtras")}</p>
    {/if}
  </div>
</section>

<style>
  /* Encadré et bandeau viennent des classes globales `.blk*` (global.css). */
  .note {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  /* Jaune = alerte, comme les autres signalements de ce bloc : rien n'est
     cassé, mais l'utilisateur doit savoir que ces fichiers ne lui appartiennent
     pas tout à fait. */
  .warn {
    color: var(--yellow);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
  }
  .grp-list,
  .file-list {
    list-style: none;
  }
  .grp-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .grp-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--line);
    background: var(--raised);
    padding: 8px 11px;
    text-align: left;
    cursor: pointer;
  }
  .grp-row:hover {
    border-color: var(--rosso-border);
  }
  .grp-caret {
    color: var(--muted);
    font-size: 12px;
    width: 10px;
  }
  .grp-dir {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grp-replaced,
  .file-replaced {
    color: var(--rosso-bright);
    font-size: 10.5px;
    white-space: nowrap;
  }
  .grp-shared {
    color: var(--yellow);
    font-size: 10.5px;
    white-space: nowrap;
  }
  /* Bleu = information : la zone auto-gérée n'est pas une anomalie, c'est un
     fait sur le chemin. Jaune pour l'occupant étranger, qui lui a une
     conséquence — le fichier du mod n'arrive pas dans le jeu. */
  .grp-managed,
  .file-managed {
    color: var(--blue);
    font-size: 10.5px;
    white-space: nowrap;
  }
  .grp-foreign,
  .file-foreign {
    color: var(--yellow);
    font-size: 10.5px;
    white-space: nowrap;
  }
  /* Jaune = alerte : ni une erreur, ni une action destructive — un fichier
     conservé mais que le jeu ne recevra pas. */
  .grp-offpath,
  .file-offpath {
    color: var(--yellow);
    font-size: 10.5px;
    white-space: nowrap;
  }
  .grp-n {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }
  .grp-size {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
    min-width: 56px;
    text-align: right;
  }
  .file-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 0 2px 32px;
  }
  .file-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11.5px;
  }
  .file-nm {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-by {
    color: var(--yellow);
    font-size: 10.5px;
    white-space: nowrap;
  }
  .file-size {
    color: var(--muted);
    font-size: 10.5px;
    white-space: nowrap;
    min-width: 56px;
    text-align: right;
  }
</style>
