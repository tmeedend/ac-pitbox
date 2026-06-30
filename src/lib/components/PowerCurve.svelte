<script lang="ts">
  // Courbe moteur à double axe (§5bis.1) façon Content Manager : couple (Nm) à
  // gauche en jaune, puissance (bhp) à droite en rouge, RPM en abscisse, zone de
  // tracé carrée avec graduations.
  interface Props {
    power: [number, number][];
    torque: [number, number][];
  }
  let { power, torque }: Props = $props();

  // Marges pour les axes ; PLOT = côté carré de la zone de tracé.
  const L = 40;
  const R = 40;
  const T = 10;
  const B = 32;
  const PLOT = 176;
  const W = L + PLOT + R;
  const H = T + PLOT + B;

  function niceMax(v: number): number {
    if (v <= 0) return 1;
    const e = Math.floor(Math.log10(v));
    const f = v / 10 ** e;
    const nf = f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10;
    return nf * 10 ** e;
  }

  const xMax = $derived(niceMax(Math.max(...power.map((p) => p[0]), ...torque.map((p) => p[0]), 1)));
  const pMax = $derived(niceMax(Math.max(...power.map((p) => p[1]), 1)));
  const tMax = $derived(niceMax(Math.max(...torque.map((p) => p[1]), 1)));

  const sx = (x: number) => L + (x / xMax) * PLOT;
  function path(points: [number, number][], yMax: number): string {
    if (points.length < 2) return "";
    const sy = (y: number) => T + PLOT - (y / yMax) * PLOT;
    return points.map((p, i) => `${i ? "L" : "M"}${sx(p[0]).toFixed(1)},${sy(p[1]).toFixed(1)}`).join(" ");
  }
  const powerPath = $derived(path(power, pMax));
  const torquePath = $derived(path(torque, tMax));

  const fracs = [0, 0.5, 1];
  const yAt = (f: number) => T + PLOT - f * PLOT;
  const xAt = (f: number) => L + f * PLOT;
  const fmtRpm = (v: number) => (v >= 1000 ? `${Math.round(v / 1000)}k` : `${Math.round(v)}`);
</script>

<svg class="curve" viewBox={`0 0 ${W} ${H}`}>
  {#each fracs as f}
    <line x1={L} y1={yAt(f)} x2={L + PLOT} y2={yAt(f)} class="grid" />
    <text x={L - 5} y={yAt(f) + 3} class="tick tor" text-anchor="end">{Math.round(f * tMax)}</text>
    <text x={L + PLOT + 5} y={yAt(f) + 3} class="tick pow" text-anchor="start">{Math.round(f * pMax)}</text>
  {/each}
  {#each fracs as f}
    <text x={xAt(f)} y={T + PLOT + 14} class="tick rpm" text-anchor="middle">{fmtRpm(f * xMax)}</text>
  {/each}

  <line x1={L} y1={T} x2={L} y2={T + PLOT} class="axis" />
  <line x1={L + PLOT} y1={T} x2={L + PLOT} y2={T + PLOT} class="axis" />
  <line x1={L} y1={T + PLOT} x2={L + PLOT} y2={T + PLOT} class="axis" />

  <path d={torquePath} class="torque" fill="none" />
  <path d={powerPath} class="power" fill="none" />

  <text x={11} y={T + PLOT / 2} class="axis-title tor" text-anchor="middle" transform={`rotate(-90 11 ${T + PLOT / 2})`}>Nm</text>
  <text x={W - 11} y={T + PLOT / 2} class="axis-title pow" text-anchor="middle" transform={`rotate(90 ${W - 11} ${T + PLOT / 2})`}>bhp</text>
  <text x={L + PLOT / 2} y={H - 5} class="axis-title rpm" text-anchor="middle">RPM</text>
</svg>

<style>
  .curve {
    width: 100%;
    height: auto;
    display: block;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 0.5;
  }
  .axis {
    stroke: var(--muted2);
    stroke-width: 0.7;
  }
  .power {
    stroke: var(--rosso-bright);
    stroke-width: 2;
  }
  .torque {
    stroke: var(--yellow);
    stroke-width: 2;
  }
  .tick {
    font-size: 8px;
    font-family: var(--mono);
  }
  .tick.tor {
    fill: var(--yellow);
  }
  .tick.pow {
    fill: var(--rosso-bright);
  }
  .tick.rpm {
    fill: var(--muted);
  }
  .axis-title {
    font-size: 9px;
    font-family: var(--mono);
  }
  .axis-title.tor {
    fill: var(--yellow);
  }
  .axis-title.pow {
    fill: var(--rosso-bright);
  }
  .axis-title.rpm {
    fill: var(--muted);
  }
</style>
