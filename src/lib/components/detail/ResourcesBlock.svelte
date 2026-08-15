<script lang="ts">
  // Bloc « Ressources » de la fiche détail (§4.5.2) : fichiers annexes rangés à
  // part du contenu de jeu (PDF, changelog, templates de skin…).
  //
  // Lus **en direct sur disque** à chaque ouverture, jamais mémorisés en base :
  // un fichier déposé à la main dans le dossier apparaît sans réimport.
  //
  // Un format lisible (texte, markdown, image, PDF) s'ouvre en prévisualisation
  // sous la liste plutôt que dans une application externe. Le rendu est posé
  // dans le flux de la page, sans hauteur imposée ni défilement propre : c'est
  // la page entière qui défile, un document se lit d'un seul geste.
  import { listModResources, openModResource, modResourceSrc, readModResource, type ResourceFile } from "$lib/library";
  import { previewKind, decodeText, type PreviewKind } from "$lib/resourcePreview";
  import { renderMarkdown } from "$lib/markdown";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ResourcePdf from "./ResourcePdf.svelte";

  let {
    modId,
    onerror,
  }: {
    modId: string;
    onerror: (message: string) => void;
  } = $props();

  let files = $state<ResourceFile[]>([]);
  /** Ressource ouverte en prévisualisation, `null` quand la liste seule est affichée. */
  let selected = $state<string | null>(null);
  let loading = $state(false);
  /** Message d'échec propre à la prévisualisation : il s'affiche à la place du
      document, sans faire remonter une bannière d'erreur sur toute la fiche. */
  let failure = $state<string | null>(null);
  let text = $state<string | null>(null);
  let html = $state<string | null>(null);
  let imgSrc = $state<string | null>(null);
  let pdfData = $state<ArrayBuffer | null>(null);

  const selectedKind = $derived<PreviewKind | null>(selected ? previewKind(selected) : null);

  // La garde sur `modId` évite qu'une réponse tardive d'un mod précédent
  // n'écrase la liste du mod courant.
  $effect(() => {
    const current = modId;
    files = [];
    selected = null;
    listModResources(current).then((rs) => {
      if (current === modId) files = rs;
    });
  });

  function clearPreview() {
    text = null;
    html = null;
    imgSrc = null;
    pdfData = null;
    failure = null;
  }

  // Chargement du contenu sélectionné. Même garde que la liste : seule la
  // dernière sélection a le droit d'écrire le résultat, sinon un clic rapide
  // sur deux fichiers peut afficher le contenu du premier sous le nom du second.
  $effect(() => {
    const rel = selected;
    const mod = modId;
    clearPreview();
    if (!rel) return;
    const kind = previewKind(rel);
    if (!kind) return;
    const stale = () => rel !== selected || mod !== modId;
    loading = true;
    (async () => {
      try {
        if (kind === "image") {
          const src = await modResourceSrc(mod, rel);
          if (!stale()) imgSrc = src;
          return;
        }
        const bytes = await readModResource(mod, rel);
        if (stale()) return;
        if (kind === "pdf") pdfData = bytes;
        else if (kind === "markdown") html = renderMarkdown(decodeText(bytes));
        else text = decodeText(bytes);
      } catch (e) {
        if (!stale()) failure = errorText(e);
      } finally {
        if (!stale()) loading = false;
      }
    })();
  });

  /** Taille lisible (Ko/Mo/Go, base 1024) — unités françaises, propres à ce bloc. */
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

  /** Un clic prévisualise ce qui est lisible, et bascule (referme) la sélection
      courante ; le reste part dans l'application par défaut de Windows. */
  function activate(f: ResourceFile) {
    if (previewKind(f.rel_path)) {
      selected = selected === f.rel_path ? null : f.rel_path;
    } else {
      openExternally(f);
    }
  }

  async function openExternally(f: ResourceFile) {
    try {
      // Le chemin relatif est résolu et validé côté backend (anti-traversée).
      await openModResource(modId, f.rel_path);
    } catch (e) {
      onerror(errorText(e));
    }
  }

  /** Les liens d'un readme partent dans le navigateur du système : suivis dans
      la WebView, ils remplaceraient l'application par la page distante. */
  function interceptLink(e: MouseEvent) {
    const a = (e.target as HTMLElement | null)?.closest("a");
    if (!a) return;
    e.preventDefault();
    openUrl(a.getAttribute("href") ?? "").catch(() => {});
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.resourcesTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <p class="note">{t("detail.resourcesNote")}</p>
      <ul class="res-list">
        {#each files as f (f.rel_path)}
          {@const canPreview = previewKind(f.rel_path) !== null}
          <li>
            <div class="res-row" class:on={selected === f.rel_path}>
              <button
                class="res-main"
                type="button"
                onclick={() => activate(f)}
                title={canPreview ? t("detail.resourcePreviewTooltip") : t("detail.resourceOpenTooltip")}
              >
                <span class="res-nm">{f.rel_path}</span>
                <span class="res-size mono">{fmtFileSize(f.size_bytes)}</span>
              </button>
              {#if canPreview}
                <button
                  class="res-ext"
                  type="button"
                  onclick={() => openExternally(f)}
                  title={t("detail.resourceOpenTooltip")}
                  aria-label={t("detail.resourceOpenTooltip")}>↗</button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>

      {#if selected}
        <div class="preview">
          <header class="pv-h">
            <span class="pv-nm mono">{selected}</span>
            <button class="pv-close" type="button" onclick={() => (selected = null)} title={t("common.close")}>×</button>
          </header>
          {#if failure}
            <p class="pv-err">{failure}</p>
          {:else if loading && selectedKind !== "pdf"}
            <p class="pv-info">{t("detail.previewLoading")}</p>
          {:else if imgSrc}
            <img class="pv-img" src={imgSrc} alt={selected} />
          {:else if pdfData}
            <ResourcePdf data={pdfData} onerror={(m) => (failure = m)} />
          {:else if html !== null}
            <!-- Markdown rendu par `renderMarkdown`, qui échappe la source
                 avant de produire le moindre tag (voir markdown.ts). -->
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="pv-md" onclick={interceptLink}>{@html html}</div>
          {:else if text !== null}
            <pre class="pv-txt">{text}</pre>
          {/if}
        </div>
      {/if}
    {:else}
      <p class="empty">{t("detail.noResources")}</p>
    {/if}
  </div>
</section>

<style>
  /* Habillage propre au bloc. Encadré et bandeau viennent des classes
     globales `.blk*` (voir global.css). */
  .note {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
  }
  .res-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .res-row {
    display: flex;
    align-items: stretch;
    border: 1px solid var(--line);
    background: var(--raised);
  }
  .res-row:hover,
  .res-row.on {
    border-color: var(--rosso-border);
  }
  .res-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    background: none;
    border: none;
    padding: 8px 11px;
    text-align: left;
    cursor: pointer;
  }
  .res-nm {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res-row:hover .res-nm,
  .res-row.on .res-nm {
    color: var(--rosso-bright);
  }
  .res-size {
    flex: none;
    font-size: 10.5px;
    color: var(--muted2);
  }
  .res-ext {
    flex: none;
    padding: 0 10px;
    background: none;
    border: none;
    border-left: 1px solid var(--line);
    color: var(--muted2);
    font-size: 12px;
    cursor: pointer;
  }
  .res-ext:hover {
    color: var(--rosso-bright);
  }

  /* Prévisualisation : aucune hauteur imposée et aucun `overflow` — le
     document s'étend, et c'est la page de la fiche qui défile. Une boîte à
     défilement propre obligerait à viser une zone pour lire un readme. */
  .preview {
    margin-top: 14px;
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  .pv-h {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px 7px 11px;
    border-bottom: 1px solid var(--line);
    background: var(--raised);
    /* Le nom du fichier reste visible en défilant un long document. */
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .pv-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pv-close {
    flex: none;
    width: 22px;
    height: 22px;
    background: none;
    border: none;
    color: var(--muted2);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
  }
  .pv-close:hover {
    color: var(--rosso-bright);
  }
  .pv-info,
  .pv-err {
    padding: 14px;
    font-size: 12px;
  }
  .pv-info {
    color: var(--muted);
  }
  .pv-err {
    color: var(--rosso-bright);
  }
  .pv-img {
    display: block;
    max-width: 100%;
    margin: 0 auto;
  }
  .pv-txt {
    margin: 0;
    padding: 14px 16px;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.6;
    color: var(--txt2);
    /* Un readme est écrit pour une console : les retours à la ligne de
       l'auteur comptent, mais une ligne trop longue doit se replier plutôt
       qu'imposer un défilement horizontal. */
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .pv-md {
    padding: 14px 18px 20px;
    font-size: 13px;
    line-height: 1.65;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  /* Le markdown est injecté en `{@html}` : son style ne peut pas passer par
     le scoping habituel de Svelte, d'où `:global()` limité à `.pv-md`. */
  .pv-md :global(h1),
  .pv-md :global(h2),
  .pv-md :global(h3),
  .pv-md :global(h4),
  .pv-md :global(h5),
  .pv-md :global(h6) {
    color: var(--txt);
    font-weight: 600;
    line-height: 1.3;
    margin: 18px 0 8px;
  }
  .pv-md :global(h1) {
    font-size: 18px;
  }
  .pv-md :global(h2) {
    font-size: 15.5px;
  }
  .pv-md :global(h3) {
    font-size: 13.5px;
  }
  .pv-md :global(h4),
  .pv-md :global(h5),
  .pv-md :global(h6) {
    font-size: 13px;
  }
  .pv-md :global(:first-child) {
    margin-top: 0;
  }
  .pv-md :global(p) {
    margin: 0 0 10px;
  }
  .pv-md :global(ul),
  .pv-md :global(ol) {
    margin: 0 0 10px;
    padding-left: 22px;
  }
  .pv-md :global(li) {
    margin-bottom: 3px;
  }
  .pv-md :global(blockquote) {
    margin: 0 0 10px;
    padding-left: 12px;
    border-left: 2px solid var(--line);
    color: var(--muted);
  }
  .pv-md :global(hr) {
    border: none;
    border-top: 1px solid var(--line);
    margin: 16px 0;
  }
  .pv-md :global(code) {
    font-family: var(--mono);
    font-size: 11.5px;
    background: var(--raised);
    padding: 1px 4px;
  }
  .pv-md :global(pre) {
    background: var(--raised);
    border: 1px solid var(--line);
    padding: 10px 12px;
    margin: 0 0 10px;
    overflow-x: auto;
  }
  .pv-md :global(pre code) {
    background: none;
    padding: 0;
  }
  .pv-md :global(a) {
    color: var(--blue);
    cursor: pointer;
  }
  .pv-md :global(strong) {
    color: var(--txt);
  }
</style>
