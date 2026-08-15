<script lang="ts">
  // PDF preview of a resource (§4.5.2), rendered with pdf.js.
  //
  // Why not an <iframe> on the file: WebView2 would answer with the Edge PDF
  // viewer, a self-contained application with its own toolbar and — the part
  // that makes a mod readme tiresome to read — its own inner scrollbar inside
  // a fixed-height box. Here every page is drawn to its own canvas and the
  // canvases are simply stacked in the flow, so the document scrolls with the
  // rest of the detail page and takes the full available width.
  import * as pdfjs from "pdfjs-dist";
  import type { PDFDocumentLoadingTask } from "pdfjs-dist";
  // Vite resolves this to a hashed asset it emits itself; pdf.js must be told
  // where its worker lives or it silently falls back to parsing on the main
  // thread, which freezes the window on any document of real size.
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { t } from "$lib/i18n/index.svelte";

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  let {
    data,
    onerror,
  }: {
    /** Raw bytes of the PDF. */
    data: ArrayBuffer;
    onerror: (message: string) => void;
  } = $props();

  /** Rendered pages, in order. Canvases are created off-DOM and attached below. */
  let canvases = $state<HTMLCanvasElement[]>([]);
  let host = $state<HTMLDivElement | null>(null);
  let width = $state(0);
  let busy = $state(true);

  // Device pixel ratio, capped: a 4x-scaled canvas of an A4 page is ~35 MPx
  // and buys nothing visible, but it does buy an out-of-memory on a long
  // document.
  const dpr = Math.min(globalThis.devicePixelRatio || 1, 2);

  /**
   * Renders the whole document at the current width.
   *
   * pdf.js needs a pixel size up front, so the render is redone when the
   * column is resized — debounced by the caller. `token` guards against an
   * older pass finishing last and pushing stale canvases.
   */
  let token = 0;
  async function render(bytes: ArrayBuffer, cssWidth: number) {
    if (cssWidth < 1) return;
    const mine = ++token;
    busy = true;
    // The teardown hangs off the loading task, not the document: it is what
    // owns the worker, and leaving one alive per re-render leaks a thread and
    // the whole parsed document with it.
    let task: PDFDocumentLoadingTask | null = null;
    try {
      // pdf.js takes ownership of the buffer it is given (it is transferred to
      // the worker and left detached), so it gets a copy — `data` has to stay
      // usable for the next re-render.
      task = pdfjs.getDocument({ data: bytes.slice(0) });
      const doc = await task.promise;
      const rendered: HTMLCanvasElement[] = [];
      for (let n = 1; n <= doc.numPages; n++) {
        const page = await doc.getPage(n);
        const base = page.getViewport({ scale: 1 });
        const viewport = page.getViewport({ scale: (cssWidth / base.width) * dpr });
        const canvas = document.createElement("canvas");
        canvas.width = Math.floor(viewport.width);
        canvas.height = Math.floor(viewport.height);
        // The canvas is laid out in CSS pixels while its bitmap is dpr times
        // denser — that is what makes the text sharp on a scaled display.
        canvas.style.width = "100%";
        canvas.style.height = "auto";
        const ctx = canvas.getContext("2d");
        if (!ctx) throw new Error("canvas 2d context unavailable");
        await page.render({ canvas, canvasContext: ctx, viewport }).promise;
        page.cleanup();
        if (mine !== token) return;
        rendered.push(canvas);
        // Published page by page: a long document shows its first pages
        // immediately instead of staying blank until the last one is drawn.
        canvases = [...rendered];
      }
      canvases = rendered;
    } catch (e) {
      if (mine === token) onerror(e instanceof Error ? e.message : String(e));
    } finally {
      if (mine === token) busy = false;
      task?.destroy();
    }
  }

  // Re-render on document change or on a real width change. Rounding to whole
  // pixels keeps sub-pixel layout jitter from restarting the render loop.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const bytes = data;
    const w = Math.round(width);
    clearTimeout(timer);
    timer = setTimeout(() => render(bytes, w), 80);
    return () => clearTimeout(timer);
  });

  // Canvases are DOM nodes built imperatively, so they are attached by hand
  // rather than through markup.
  $effect(() => {
    const el = host;
    const pages = canvases;
    if (!el) return;
    el.replaceChildren(...pages);
  });
</script>

<div class="pdf" bind:clientWidth={width}>
  <div class="pages" bind:this={host}></div>
  {#if busy}
    <p class="loading">{t("detail.previewLoading")}</p>
  {/if}
</div>

<style>
  .pdf {
    width: 100%;
  }
  .pages {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* White backing: a PDF page is drawn with a transparent background, and the
     dark theme behind it would otherwise leave black text on black. */
  .pages :global(canvas) {
    display: block;
    background: #fff;
    border: 1px solid var(--line);
  }
  .loading {
    color: var(--muted);
    font-size: 12px;
    padding: 10px 0;
  }
</style>
