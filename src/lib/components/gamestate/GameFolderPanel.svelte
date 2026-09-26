<script lang="ts">
  // The detail panel (DOSSIER§8.2): "where does this come from, and what does
  // it mean". It follows the selection, never the pointer, and it is never
  // empty - without a selection it shows the root.
  //
  // **Read only** (DOSSIER§R1): what it sees and cannot fix, it says, and it
  // says where to fix it. Its only buttons navigate.
  import { t } from "$lib/i18n/index.svelte";
  import { hasSheet } from "$lib/gamestate/gameFolder.svelte";
  import type { Counts, Detail, ModHit, NodeId, Owner, StateValue } from "$lib/gamestate/gamestate";

  interface Props {
    detail: Detail | null;
    /** A library element chosen in the search that is not in the game. */
    mod: ModHit | null;
    onopen: (owner: Owner) => void;
    onfilter: (owner: Owner) => void;
    onexplorer: (node: NodeId) => void;
    onmaintenance: () => void;
  }
  let { detail, mod, onopen, onfilter, onexplorer, onmaintenance }: Props = $props();

  const d = $derived(detail);
  const leaf = $derived(d ? d.kind !== "dir" : false);
  /** The element the path is about: the one providing it, else the first to
   * claim it. */
  const about = $derived<Owner | null>(d ? (d.owner ?? d.claims[0]?.owner ?? null) : (mod?.owner ?? null));

  const date = (iso: string | null) => (iso ? new Date(iso).toLocaleDateString() : "");
  const size = (n: number | null) => (n === null ? "?" : n.toLocaleString());

  /** The state said in one sentence (DOSSIER§8.2). */
  const sentence = $derived.by(() => {
    if (!d) return "";
    const name = about?.name ?? "";
    if (!leaf) {
      if (d.folder) return t("gamefolder.sayModFolder", { name: d.folder.owner.name });
      if (d.population === "origin") return t("gamefolder.sayOrigin");
      if (d.population === "unmanaged") return t("gamefolder.sayUnmanaged");
      return "";
    }
    if (d.drift) {
      const k = d.drift.kind;
      if (k === "modified") {
        const base = t("gamefolder.sayModified", {
          before: size(d.drift.expectedSize),
          after: size(d.drift.actualSize),
        });
        return d.cmZone ? `${base} ${t("gamefolder.sayModifiedCm")}` : base;
      }
      return t(`gamefolder.say.${k}`);
    }
    switch (d.state) {
      case "posed":
        return d.mechanism?.type === "link"
          ? t("gamefolder.sayPosedLink", { name })
          : t("gamefolder.sayPosed", { name });
      case "replacesGame":
        return t("gamefolder.sayReplaces");
      case "waiting":
        return t("gamefolder.sayWaiting", { name });
      default:
        if (d.population === "origin") return t("gamefolder.sayOrigin");
        if (d.population === "unmanaged") return t("gamefolder.sayUnmanaged");
        return t("gamefolder.sayNobody");
    }
  });

  const TONE: Record<string, string> = {
    posed: "pill-ok",
    replacesGame: "pill-err",
    waiting: "pill-warn",
    drift: "pill-warn",
  };

  const DECOMPOSITION: { value: StateValue; key: keyof Counts }[] = [
    { value: "drift", key: "drift" },
    { value: "replacesGame", key: "replacesGame" },
    { value: "waiting", key: "waiting" },
    { value: "cmZone", key: "cmZone" },
    { value: "posed", key: "posed" },
    { value: "shared", key: "shared" },
    { value: "nobody", key: "nobody" },
  ];

  const REASON: Record<string, string> = {
    older: "gamefolder.reasonOlder",
    sameDate: "gamefolder.reasonSameDate",
    waiting: "gamefolder.reasonWaiting",
  };
</script>

{#snippet ownerLink(o: Owner)}
  {#if hasSheet(o)}
    <button type="button" class="lnk" onclick={() => onopen(o)}>{o.name}</button>
  {:else}
    <span>{o.name}</span>
  {/if}
{/snippet}

<aside class="panel" aria-live="polite">
  {#if d}
    <header class="dh">
      <span class="n mono">{d.name || t("gamefolder.rootName")}</span>
      <span class="p mono">{d.relPath || "\\"}</span>
      {#if leaf}
        <span class="pills">
          {#if d.drift}
            <span class="pill pill-warn"><span aria-hidden="true">≠</span>{t(`gamefolder.drift.${d.drift.kind}`)}</span>
          {:else if d.state !== "nobody"}
            <span class="pill {TONE[d.state]}">{t(`gamefolder.state.${d.state}`)}</span>
          {/if}
          {#if d.cmZone}<span class="pill pill-cm">{t("gamefolder.state.cmZone")}</span>{/if}
        </span>
      {/if}
      {#if sentence}<p class="say">{sentence}</p>{/if}
    </header>

    {#if d.mechanism}
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("gamefolder.blkMechanism")}</span></header>
        <div class="blk-b body">
          {#if d.mechanism.type === "library"}
            <span>{t("gamefolder.mechLibrary")}</span>
            <span class="v mono">{d.mechanism.copy}</span>
          {:else if d.mechanism.type === "link"}
            <span>{t(d.mechanism.broken ? "gamefolder.mechLinkBroken" : "gamefolder.mechLink")}</span>
            <span class="v mono">{d.mechanism.target}</span>
          {:else if d.mechanism.type === "replaced"}
            <span>{t("gamefolder.mechReplaced")}</span>
            <span class="v mono">{d.mechanism.backup}</span>
          {:else if d.mechanism.type === "missing"}
            <span>{t("gamefolder.mechMissing")}</span>
          {:else}
            <span>{t("gamefolder.mechReal")}</span>
          {/if}
        </div>
      </section>
    {/if}

    {#if leaf && (d.owner || d.claims.length)}
      <section class="blk">
        <header class="blk-h">
          <span class="blk-t">{t("gamefolder.blkProvenance")}</span>
          {#if d.claims.length > 1}<span class="blk-n">{d.claims.length}</span>{/if}
        </header>
        <div class="blk-b body">
          {#if d.claims.length}
            {#each d.claims as c (`${c.owner.kind}:${c.owner.id}`)}
              <div class="claim">
                {@render ownerLink(c.owner)}
                <span class="why">
                  {#if c.provided}
                    {t(c.forced ? "gamefolder.reasonForced" : "gamefolder.reasonProvides")}
                  {:else if c.reason}
                    {t(REASON[c.reason], { date: date(c.copyDate) })}
                  {/if}
                </span>
              </div>
            {/each}
          {:else if d.owner}
            <div class="claim">{@render ownerLink(d.owner)}</div>
          {/if}
        </div>
      </section>
    {/if}

    {#if d.revert.length}
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("gamefolder.blkRevert")}</span></header>
        <div class="blk-b body">
          <p>{t("gamefolder.revertSay", { names: d.revert.map((o) => o.name).join(", ") })}</p>
          {#if hasSheet(d.revert[0])}
            <button type="button" class="lnk" onclick={() => onopen(d.revert[0])}>{t("gamefolder.openSheetOf", { name: d.revert[0].name })}</button>
          {/if}
        </div>
      </section>
    {/if}

    {#if d.drift}
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("gamefolder.blkDrift")}</span></header>
        <div class="blk-b body">
          <span class="v">{t(`gamefolder.drift.${d.drift.kind}`)}</span>
          <p class="muted">{t(`gamefolder.cause.${d.drift.kind}`)}</p>
          <button type="button" class="lnk" onclick={onmaintenance}>{t("gamefolder.goMaintenance")}</button>
        </div>
      </section>
    {/if}

    {#if !leaf}
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("gamefolder.blkFolder")}</span></header>
        <div class="blk-b body">
          <div class="dec">
            {#each DECOMPOSITION as row (row.value)}
              {#if d.counts[row.key]}
                <span>{t(`gamefolder.state.${row.value}`)}</span>
                <span class="c mono">{d.counts[row.key].toLocaleString()}</span>
              {/if}
            {/each}
          </div>
          {#if d.folder}
            <div class="kv">
              {#if d.folder.version}
                <span class="lbl-key">{t("gamefolder.version")}</span><span class="v mono">{d.folder.version}</span>
              {/if}
              {#if d.folder.layers.length}
                <span class="lbl-key">{t("gamefolder.layers")}</span><span class="v">{d.folder.layers.join(", ")}</span>
              {/if}
            </div>
          {/if}
          {#if d.contributors.length}
            <span class="lbl-key sub">{t("gamefolder.contributors")}</span>
            {#each d.contributors as c (`${c.owner.kind}:${c.owner.id}`)}
              <div class="claim">
                {@render ownerLink(c.owner)}
                <span class="why mono">{c.files.toLocaleString()}</span>
              </div>
            {/each}
            {#if d.moreContributors}
              <span class="muted">{t("gamefolder.andOthers", { count: d.moreContributors })}</span>
            {/if}
          {/if}
        </div>
      </section>
    {/if}

    <footer class="foot">
      {#if about && hasSheet(about)}
        <button type="button" class="btn" onclick={() => onopen(about)}>{t("gamefolder.openSheet")}</button>
      {/if}
      {#if about}
        <button type="button" class="btn" onclick={() => onfilter(about)}>{t("gamefolder.filterOn")}</button>
      {/if}
      <button type="button" class="btn" onclick={() => onexplorer(d.id)}>{t("gamefolder.showInExplorer")}</button>
    </footer>
  {:else if mod}
    <header class="dh">
      <span class="n">{mod.owner.name}</span>
      <span class="p mono">{mod.owner.id}</span>
      <span class="pills"><span class="pill">{t(`gamefolder.presence.${mod.presence}`)}</span></span>
      <p class="say">{t("gamefolder.sayNotInGame")}</p>
    </header>
    <footer class="foot">
      {#if hasSheet(mod.owner)}
        <button type="button" class="btn" onclick={() => onopen(mod.owner)}>{t("gamefolder.openSheet")}</button>
      {/if}
    </footer>
  {/if}
</aside>

<style>
  .panel {
    height: 100%;
    overflow-y: auto;
    padding: 16px;
    background: var(--panel);
    border-left: 1px solid var(--line);
  }
  .dh {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 14px;
  }
  .n {
    font-size: 14px;
    color: var(--txt);
    word-break: break-all;
  }
  .p {
    font-size: 11px;
    color: var(--muted);
    word-break: break-all;
    user-select: all;
  }
  .pills {
    display: inline-flex;
    gap: 4px;
  }
  .pill-cm {
    color: var(--blue);
    border-color: var(--blue-border);
    background: var(--blue-dim);
  }
  .say {
    font-size: 12.5px;
    color: var(--txt2);
    line-height: 1.55;
  }
  .blk {
    margin-bottom: 12px;
  }
  .blk-h {
    padding: 9px 12px;
  }
  .body {
    padding: 10px 12px;
    font-size: 12px;
    color: var(--txt2);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .v {
    font-size: 11px;
    word-break: break-all;
  }
  .muted {
    color: var(--muted);
    font-size: 11.5px;
  }
  .lnk {
    color: var(--blue);
    background: none;
    padding: 0;
    text-align: left;
    font: inherit;
  }
  .lnk:hover {
    text-decoration: underline;
  }
  .claim {
    display: flex;
    justify-content: space-between;
    gap: 10px;
  }
  .why {
    color: var(--muted);
    font-size: 11px;
    text-align: right;
  }
  .dec {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px 10px;
  }
  .c {
    color: var(--txt2);
    font-variant-numeric: tabular-nums;
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin-top: 6px;
    align-items: baseline;
  }
  .lbl-key {
    text-transform: uppercase;
  }
  .sub {
    margin-top: 8px;
  }
  .foot {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }
</style>
