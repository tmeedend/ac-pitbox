<script lang="ts">
  // PDF preview of a resource (§4.5.2), rendered with pdf.js.
  //
  // Why not an <iframe> on the file: WebView2 would answer with the Edge PDF
  // viewer, a self-contained application with its own toolbar and — the part
  // that makes a mod readme tiresome to read — its own inner scrollbar inside
  // a fixed-height box. Here every page is drawn to its own canvas and the
  // canvases are simply stacked in the flow, so the document scrolls with the
  // rest of the detail page and takes the full available width.
  //
  // Three things the first version did not have, and a reader needs:
  //
  //  * **A zoom level.** A wiring diagram or a setup table shipped as a PDF is
  //    unreadable at column width, and a one-page poster is a waste of it. The
  //    percentage means what it means everywhere else — 100 % is actual size —
  //    which is only true because a PDF point is 1/72 in and a CSS pixel 1/96
  //    (`CSS_UNITS`).
  //  * **Fit modes.** "Fit width" is the old behaviour, kept as the default;
  //    "fit page" scales a page to the height of the scrolling area, so one
  //    page is one screen. Both are recomputed on resize, so they stay true.
  //  * **Lazy rendering.** The first two turn the naive "draw everything up
  //    front" into a memory hazard: an A4 page at 400 % is a 14 MPx bitmap,
  //    and the first version redrew every page of the document at every width
  //    change. Pages are now drawn when they come near the viewport and
  //    dropped once the live bitmaps exceed a budget, so the cost follows what
  //    is on screen rather than the length of the document.
  //
  // A scale change does not blank the document: the canvas already drawn is
  // stretched by CSS to the new page box (the box carries the size, the canvas
  // fills it), which makes the zoom instant, and the sharp redraw lands a
  // moment later.
  import * as pdfjs from "pdfjs-dist";
  import type { PDFDocumentLoadingTask, PDFDocumentProxy, RenderTask } from "pdfjs-dist";
  // Vite resolves this to a hashed asset it emits itself; pdf.js must be told
  // where its worker lives or it silently falls back to parsing on the main
  // thread, which freezes the window on any document of real size.
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { tick, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { zoomState } from "$lib/zoom.svelte";
  import { getUiPrefs, setUiPrefs } from "$lib/uiPrefs.svelte";

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  let {
    data,
    onerror,
  }: {
    /** Raw bytes of the PDF. */
    data: ArrayBuffer;
    onerror: (message: string) => void;
  } = $props();

  /** A PDF point is 1/72 in, a CSS pixel 1/96 in: this ratio is what makes the
   * displayed percentage mean the same thing as in any other reader. */
  const CSS_UNITS = 96 / 72;
  /** Ladder walked by the − / + buttons. Ctrl+wheel is continuous instead. */
  const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.8, 1, 1.25, 1.5, 2, 2.5, 3, 4];
  const MIN_SCALE = ZOOM_STEPS[0];
  const MAX_SCALE = ZOOM_STEPS[ZOOM_STEPS.length - 1];
  /** Bitmap ceiling for one page, in pixels. Past it the render density drops
   * rather than the size: an A4 at 400 % is 14 MPx at density 1, four times
   * that at density 2 — 228 MB of bitmap for a single page. The ceiling
   * degrades gracefully: full density up to ~200 %, then one device pixel per
   * CSS pixel, still sharper than the stretched preview it replaces. */
  const MAX_PAGE_PIXELS = 16e6;
  /** Ceiling for all live bitmaps together (~190 MB at 4 bytes each). Pages
   * near the viewport are never dropped, so this only bounds the backlog kept
   * around to make scrolling back instant. */
  const CANVAS_BUDGET = 48e6;

  type FitMode = "width" | "page" | "custom";
  // Durable, and deliberately global rather than per document: the zoom level
  // is how *this user* reads, not a property of one mod's manual. `ui_prefs`
  // and not `localStorage` — règle d'or n°6.
  const FIT_KEY = "pitbox.pdf.fit";
  const SCALE_KEY = "pitbox.pdf.scale";

  /** One page, at its natural size in CSS pixels (i.e. at 100 %). */
  type Slot = { num: number; w: number; h: number };

  let root = $state<HTMLDivElement | null>(null);
  let host = $state<HTMLDivElement | null>(null);
  let slots = $state.raw<Slot[]>([]);
  let fit = $state<FitMode>("width");
  let customScale = $state(1);
  let prefsReady = $state(false);
  let loading = $state(true);
  /** Width of the column and height of the scrolling area — the two operands
   * of the fit modes. */
  let width = $state(0);
  let viewHeight = $state(0);
  /** First page currently on screen, for the page counter. */
  let current = $state(1);

  // --- Plumbing that must NOT be reactive ----------------------------------
  //
  // Canvases, render bookkeeping and observer state change on every scroll
  // tick; making them `$state` would re-run effects for nothing.
  let doc: PDFDocumentProxy | null = null;
  let docTask: PDFDocumentLoadingTask | null = null;
  /** Bumped on every document change; a pass that finishes late checks it and
   * drops its result instead of pushing stale pages. */
  let generation = 0;
  /** Page boxes, indexed by page number − 1. Filled by `bind:this`, hence
   * `$state.raw` and not a plain `let`: the compiler refuses a bare variable
   * written from the template. Raw is the point — the slots are written one by
   * one as the DOM is built, and nothing should re-run on each of them; only
   * the wholesale reset below is meant to be seen. */
  let pageEls = $state.raw<(HTMLDivElement | null)[]>([]);
  /** Pages near enough to the viewport to be worth drawing. */
  let nearby = new Set<number>();
  /** Pages genuinely on screen, to pick the counter's value. */
  let onScreen = new Set<number>();
  let canvases = new Map<number, HTMLCanvasElement>();
  /** Scale each live canvas was drawn at — a mismatch with `scale` is what
   * marks it stale and schedules its redraw. */
  let renderedScale = new Map<number, number>();
  let renderTask: RenderTask | null = null;
  let pumping = false;
  let rerenderTimer: ReturnType<typeof setTimeout> | undefined;
  /** The two observers below, and the pages already handed to them. */
  let observers: IntersectionObserver[] = [];
  let observed = new Set<number>();

  const clamp = (s: number) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, s));

  // --- Scale ---------------------------------------------------------------
  //
  // The fit modes measure the FIRST page rather than the largest one: page
  // sizes are discovered progressively (see the loader below), so a maximum
  // would keep moving as the document loads and restart the render loop each
  // time. A document whose later pages are wider — a manual with one landscape
  // schematic — simply scrolls sideways on those.
  const reference = $derived(slots[0] ?? null);
  const fitWidth = $derived(reference && width > 0 ? width / reference.w : 1);
  const fitPage = $derived(
    reference && width > 0 && viewHeight > 0 ? Math.min(width / reference.w, (viewHeight - 8) / reference.h) : 1,
  );
  const scale = $derived(clamp(fit === "width" ? fitWidth : fit === "page" ? fitPage : customScale));

  // --- Preferences ---------------------------------------------------------
  //
  // `untrack`: `getUiPrefs` reads the preference cache synchronously before
  // its first `await`, so an unguarded call inside an effect subscribes that
  // effect to *every* preference in the app — the trap documented at length in
  // `uiPrefs.svelte.ts`.
  $effect(() => {
    untrack(() => {
      void (async () => {
        const saved = await getUiPrefs([FIT_KEY, SCALE_KEY]);
        const mode = saved[FIT_KEY];
        if (mode === "width" || mode === "page" || mode === "custom") fit = mode;
        const saved_scale = Number(saved[SCALE_KEY]);
        if (Number.isFinite(saved_scale) && saved_scale > 0) customScale = clamp(saved_scale);
        prefsReady = true;
      })();
    });
  });

  $effect(() => {
    const mode = fit;
    const value = customScale;
    if (!prefsReady) return;
    void setUiPrefs({ [FIT_KEY]: mode, [SCALE_KEY]: String(value) });
  });

  // --- Document ------------------------------------------------------------
  $effect(() => {
    const bytes = data;
    const mine = ++generation;
    reset();
    loading = true;
    void (async () => {
      // pdf.js takes ownership of the buffer it is given (it is transferred to
      // the worker and left detached), so it gets a copy — `data` has to stay
      // usable if the same resource is opened again.
      const task = pdfjs.getDocument({ data: bytes.slice(0) });
      try {
        const loaded = await task.promise;
        if (mine !== generation) {
          void task.destroy();
          return;
        }
        // The teardown hangs off the loading task, not the document: it is
        // what owns the worker, and leaving one alive per document leaks a
        // thread and the whole parsed document with it.
        docTask = task;
        doc = loaded;
        // Sizes first, pixels later: knowing every page box up front is what
        // lets the document lay out at its full height immediately, so the
        // scrollbar is honest and the observer can tell what is near.
        const list: Slot[] = [];
        for (let n = 1; n <= loaded.numPages; n++) {
          const page = await loaded.getPage(n);
          if (mine !== generation) return;
          const viewport = page.getViewport({ scale: CSS_UNITS });
          list.push({ num: n, w: viewport.width, h: viewport.height });
          page.cleanup();
          slots = [...list];
        }
      } catch (e) {
        if (mine === generation) onerror(e instanceof Error ? e.message : String(e));
        void task.destroy();
      } finally {
        if (mine === generation) loading = false;
      }
    })();
    return () => {
      generation++;
      reset();
    };
  });

  /** Back to nothing rendered and nothing loaded — same teardown whether the
   * document changed or the component is going away. */
  function reset() {
    clearTimeout(rerenderTimer);
    renderTask?.cancel();
    renderTask = null;
    for (const canvas of canvases.values()) {
      canvas.width = 0;
      canvas.height = 0;
    }
    canvases.clear();
    renderedScale.clear();
    nearby.clear();
    onScreen.clear();
    // The observers outlive the document (they hang off the host, which does
    // not move), so the boxes of the previous one have to be handed back.
    for (const observer of observers) {
      for (const box of pageEls) if (box) observer.unobserve(box);
    }
    observed.clear();
    pageEls = [];
    slots = [];
    current = 1;
    doc = null;
    void docTask?.destroy();
    docTask = null;
  }

  // --- What is on screen ---------------------------------------------------
  //
  // Two observers rather than one: the band that decides what gets drawn is
  // deliberately a viewport taller than the viewport itself (so a page is
  // ready before it is reached), while the page counter must only count what
  // is genuinely visible.
  $effect(() => {
    const el = host;
    if (!el) return;
    const numberOf = (target: Element) => Number((target as HTMLElement).dataset.n);
    const band = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const n = numberOf(entry.target);
          if (entry.isIntersecting) nearby.add(n);
          else nearby.delete(n);
        }
        void pump();
      },
      { rootMargin: "100% 0px" },
    );
    const seen = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const n = numberOf(entry.target);
        if (entry.isIntersecting) onScreen.add(n);
        else onScreen.delete(n);
      }
      if (onScreen.size) current = Math.min(...onScreen);
    });
    observers = [band, seen];
    return () => {
      band.disconnect();
      seen.disconnect();
      observers = [];
      observed.clear();
    };
  });

  // New page boxes are handed to the observers as they appear. Deliberately
  // NOT rebuilt from scratch on every `slots` change: page sizes are published
  // one page at a time while the document loads, and re-observing the whole
  // list each time would be quadratic in the page count.
  $effect(() => {
    const list = slots;
    if (!observers.length) return;
    for (const slot of list) {
      if (observed.has(slot.num)) continue;
      const box = pageEls[slot.num - 1];
      if (!box) continue;
      observed.add(slot.num);
      for (const observer of observers) observer.observe(box);
    }
  });

  // A scale change makes every canvas stale. Debounced: a Ctrl+wheel gesture
  // or a window drag would otherwise queue a redraw per event, each one
  // cancelling the last — the document would stay blurry for as long as the
  // gesture lasts.
  $effect(() => {
    void scale;
    void slots;
    clearTimeout(rerenderTimer);
    rerenderTimer = setTimeout(() => {
      renderTask?.cancel();
      void pump();
    }, 120);
    return () => clearTimeout(rerenderTimer);
  });

  // --- Rendering -----------------------------------------------------------

  /** Next page worth drawing: the one nearest the reader whose canvas is
   * missing or was drawn at another scale. */
  function nextPage(): number | null {
    let best: number | null = null;
    let bestDistance = Infinity;
    for (const n of nearby) {
      if (renderedScale.get(n) === scale) continue;
      const distance = Math.abs(n - current);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = n;
      }
    }
    return best;
  }

  /** One page at a time, nearest first: there is a single worker, and queueing
   * the whole band would draw page 12 before page 3 comes back sharp after a
   * zoom.
   *
   * A cancelled page does NOT stop the loop, and that is the whole point of
   * distinguishing it from a failure: a zoom cancels the page being drawn, and
   * the caller that cancelled cannot restart the loop either (it is still
   * running at that instant, so its own `pump()` returns straight away). Only
   * a superseded document or a genuine error stops here. */
  async function pump() {
    if (pumping || !doc) return;
    pumping = true;
    try {
      for (;;) {
        const n = nextPage();
        if (n === null) break;
        if ((await renderPage(n)) === "stop") break;
      }
    } finally {
      pumping = false;
    }
  }

  /**
   * Device pixels per CSS pixel for a page at `target` scale.
   *
   * The UI zoom (Settings — a CSS `zoom` on the document) magnifies the canvas
   * on screen without touching `devicePixelRatio`, so it belongs in the
   * density, or a 150 % interface would read a third softer. Capped so that a
   * big page at a big zoom lowers its density rather than its size.
   */
  function densityFor(slot: Slot, target: number): number {
    const dpr = Math.min((globalThis.devicePixelRatio || 1) * (zoomState.level / 100), 3);
    const pixels = slot.w * target * dpr * (slot.h * target * dpr);
    return pixels > MAX_PAGE_PIXELS ? dpr * Math.sqrt(MAX_PAGE_PIXELS / pixels) : dpr;
  }

  /** Draws one page. `"stop"` means the pump has nothing more to do — the
   * document was replaced under it, or the draw genuinely failed; a cancelled
   * draw returns `"go on"` so the loop picks the page up again at the new
   * scale. */
  async function renderPage(n: number): Promise<"go on" | "stop"> {
    const loaded = doc;
    const slot = slots[n - 1];
    const box = pageEls[n - 1];
    if (!loaded || !slot || !box) return "stop";
    const mine = generation;
    const target = scale;
    try {
      const page = await loaded.getPage(n);
      if (mine !== generation) return "stop";
      const viewport = page.getViewport({ scale: target * CSS_UNITS * densityFor(slot, target) });
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.floor(viewport.width));
      canvas.height = Math.max(1, Math.floor(viewport.height));
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("canvas 2d context unavailable");
      renderTask = page.render({ canvas, canvasContext: ctx, viewport });
      await renderTask.promise;
      renderTask = null;
      page.cleanup();
      if (mine !== generation) return "stop";
      // The canvas is laid out to the page box in CSS pixels while its bitmap
      // is `density` times denser — that is what makes the text sharp.
      const previous = canvases.get(n);
      if (previous) {
        previous.width = 0;
        previous.height = 0;
      }
      box.replaceChildren(canvas);
      canvases.set(n, canvas);
      renderedScale.set(n, target);
      evict();
      return "go on";
    } catch (e) {
      renderTask = null;
      // A cancelled render is the normal outcome of a zoom mid-draw, not a
      // failure to report.
      if (e && typeof e === "object" && (e as { name?: string }).name === "RenderingCancelledException") return "go on";
      if (mine === generation) onerror(e instanceof Error ? e.message : String(e));
      return "stop";
    }
  }

  /** Drops the canvases furthest from the reader until the live bitmaps fit
   * the budget. Setting width/height to 0 is what actually frees them — a
   * detached canvas keeps its bitmap until the collector gets to it. */
  function evict() {
    let total = 0;
    for (const canvas of canvases.values()) total += canvas.width * canvas.height;
    if (total <= CANVAS_BUDGET) return;
    const furthestFirst = [...canvases.keys()].sort((a, b) => Math.abs(b - current) - Math.abs(a - current));
    for (const n of furthestFirst) {
      if (total <= CANVAS_BUDGET) break;
      if (nearby.has(n)) continue;
      const canvas = canvases.get(n);
      if (!canvas) continue;
      total -= canvas.width * canvas.height;
      canvas.width = 0;
      canvas.height = 0;
      pageEls[n - 1]?.replaceChildren();
      canvases.delete(n);
      renderedScale.delete(n);
    }
  }

  // --- Zoom controls -------------------------------------------------------

  /** Nearest scrolling ancestor — the detail page's own scroller (`.full-wrap`
   * in Library.svelte), which is both what "fit page" fits into and what has
   * to be nudged so a zoom keeps the reader where they were. */
  function scrollerOf(el: HTMLElement): HTMLElement | null {
    let parent = el.parentElement;
    while (parent) {
      const overflow = getComputedStyle(parent).overflowY;
      if (overflow === "auto" || overflow === "scroll") return parent;
      parent = parent.parentElement;
    }
    return null;
  }

  $effect(() => {
    const el = root;
    if (!el) return;
    const scroller = scrollerOf(el);
    const measure = () => {
      viewHeight = scroller ? scroller.clientHeight : window.innerHeight / (zoomState.level / 100);
    };
    measure();
    const observer = new ResizeObserver(measure);
    if (scroller) observer.observe(scroller);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  });

  /**
   * Applies a new scale **without moving the scrollbar**.
   *
   * The first version corrected `scrollTop` in the ratio of the two scales, on
   * the theory that the reader wants to stay on the same line of the same page
   * — the document being taller, the distance already scrolled into it grows
   * with it. Tried on the real app, that theory is wrong: pressing − or +
   * makes the view slide under the cursor, which is exactly what one does NOT
   * want from a button one is about to press again. The scrollbar staying put
   * is what makes a zoom feel like a zoom rather than a jump.
   *
   * So: read `scrollTop`, let the boxes resize, put it back if anything moved
   * it. Something else *does* move it — see `overflow-anchor` in the style
   * block below, which is the browser doing the same well-meant correction on
   * its own.
   */
  async function applyScale(next: FitMode, value?: number) {
    const el = root;
    const scroller = el ? scrollerOf(el) : null;
    const before = scroller?.scrollTop;
    if (value !== undefined) customScale = clamp(value);
    fit = next;
    await tick();
    if (scroller && before !== undefined && scroller.scrollTop !== before) scroller.scrollTop = before;
  }

  /** Walks the ladder from wherever the current scale is — so a step out of
   * "fit width" lands on the next round value, not on an offset one. */
  function step(direction: 1 | -1) {
    const now = scale;
    const next =
      direction > 0
        ? (ZOOM_STEPS.find((s) => s > now + 0.001) ?? MAX_SCALE)
        : ([...ZOOM_STEPS].reverse().find((s) => s < now - 0.001) ?? MIN_SCALE);
    void applyScale("custom", next);
  }

  // Ctrl+wheel, continuous, the way every reader does it. Attached by hand
  // rather than with `onwheel` so the listener is explicitly non-passive:
  // `preventDefault` is what keeps the gesture from scrolling the sheet while
  // it zooms.
  $effect(() => {
    const el = root;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      void applyScale("custom", scale * Math.exp(-e.deltaY / 400));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });
</script>

<div class="pdf" bind:this={root} bind:clientWidth={width}>
  {#if slots.length}
    <div class="bar">
      <div class="seg">
        <button type="button" class:on={fit === "width"} onclick={() => applyScale("width")}>
          {t("detail.pdfFitWidth")}
        </button>
        <button type="button" class:on={fit === "page"} onclick={() => applyScale("page")}>
          {t("detail.pdfFitPage")}
        </button>
      </div>
      <div class="seg zoom">
        <button
          type="button"
          onclick={() => step(-1)}
          disabled={scale <= MIN_SCALE + 0.001}
          title={t("detail.pdfZoomOut")}
          aria-label={t("detail.pdfZoomOut")}>−</button
        >
        <button type="button" class="pct mono" onclick={() => applyScale("custom", 1)} title={t("detail.pdfActualSize")}>
          {t("detail.pdfZoomValue", { pct: Math.round(scale * 100) })}
        </button>
        <button
          type="button"
          onclick={() => step(1)}
          disabled={scale >= MAX_SCALE - 0.001}
          title={t("detail.pdfZoomIn")}
          aria-label={t("detail.pdfZoomIn")}>+</button
        >
      </div>
      <span class="count mono">{t("detail.pdfPageOf", { current, total: slots.length })}</span>
    </div>
  {/if}

  <div class="pages">
    <!-- `width: max-content` + `margin: 0 auto` rather than `align-items:
         center`: centring a flex column inside a horizontally scrolling box
         makes the overflow on the left unreachable — the page would be centred
         but its left edge impossible to scroll to once zoomed past the column. -->
    <div class="stack" bind:this={host}>
      {#each slots as slot (slot.num)}
        <div
          class="pg"
          data-n={slot.num}
          bind:this={pageEls[slot.num - 1]}
          style:width="{Math.round(slot.w * scale)}px"
          style:height="{Math.round(slot.h * scale)}px"
        ></div>
      {/each}
    </div>
  </div>

  {#if loading}
    <p class="loading">{t("detail.previewLoading")}</p>
  {/if}
</div>

<style>
  .pdf {
    width: 100%;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 0 0 10px;
  }
  /* Groupe segmenté, recopié de `Transversal`/`Library` — le CSS Svelte est
     scopé. C'est une copie de plus : quand le composant `Seg` du chantier
     « composants partagés » arrivera, celle-ci part avec les autres. */
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 5px 11px;
    font-size: 11px;
    border-right: 1px solid var(--line);
    cursor: pointer;
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button:hover:not(:disabled) {
    color: var(--txt2);
  }
  .seg button.on {
    background: var(--rosso);
    color: #fff;
  }
  .seg button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .zoom button {
    min-width: 26px;
    text-align: center;
  }
  /* Assez large pour « 400 % » sans que la barre ne bouge d'un cran à l'autre. */
  .pct {
    min-width: 58px;
    color: var(--txt2);
  }
  .count {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--muted2);
  }
  /* Le seul défilement propre de tout le bloc, et seulement horizontal : au
     delà de la largeur de la colonne, une page zoomée doit bien aller quelque
     part. La hauteur, elle, reste libre — c'est la fiche qui défile. */
  .pages {
    overflow-x: auto;
  }
  .stack {
    width: max-content;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    /* Sans ça, le navigateur corrige le défilement de lui-même : c'est
       l'ancrage de défilement (`overflow-anchor`), qui repère un élément
       visible et compense en `scrollTop` tout ce qui change de taille
       au-dessus de lui. Excellent pour une image qui arrive en cours de
       lecture, désastreux ici — les pages changent de taille *à la demande de
       l'utilisateur*, et la barre bougeait donc à chaque clic sur − / + malgré
       la remise en place faite dans `applyScale`. La propriété se pose sur le
       sous-arbre à exclure, pas sur le conteneur qui défile (qui appartient à
       la fiche, pas à ce composant). */
    overflow-anchor: none;
  }
  /* White backing: a PDF page is drawn with a transparent background, and the
     dark theme behind it would otherwise leave black text on black. The box
     carries the size, so a page not drawn yet — or being redrawn after a
     zoom — still holds its place instead of collapsing the document. */
  .pg {
    flex: none;
    background: #fff;
    border: 1px solid var(--line);
  }
  .pg :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .loading {
    color: var(--muted);
    font-size: 12px;
    padding: 10px 0;
  }
</style>
