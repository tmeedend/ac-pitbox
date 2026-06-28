<script lang="ts">
  interface Props {
    power: [number, number][];
    torque: [number, number][];
  }
  let { power, torque }: Props = $props();

  const W = 288;
  const H = 130;
  const PAD = 6;

  function pathFor(points: [number, number][], xMin: number, xMax: number): string {
    if (points.length < 2) return "";
    const yMax = Math.max(...points.map((p) => p[1])) || 1;
    const sx = (x: number) => PAD + ((x - xMin) / (xMax - xMin || 1)) * (W - 2 * PAD);
    const sy = (y: number) => H - PAD - (y / yMax) * (H - 2 * PAD);
    return points.map((p, i) => `${i === 0 ? "M" : "L"}${sx(p[0]).toFixed(1)},${sy(p[1]).toFixed(1)}`).join(" ");
  }

  const xMin = $derived(Math.min(...power.map((p) => p[0]), ...torque.map((p) => p[0]), 0));
  const xMax = $derived(Math.max(...power.map((p) => p[0]), ...torque.map((p) => p[0]), 1));
  const powerPath = $derived(pathFor(power, xMin, xMax));
  const torquePath = $derived(pathFor(torque, xMin, xMax));
  const peakPower = $derived(power.length ? Math.max(...power.map((p) => p[1])) : 0);
  const peakTorque = $derived(torque.length ? Math.max(...torque.map((p) => p[1])) : 0);
</script>

<div class="chart">
  <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none">
    <path d={torquePath} class="torque" fill="none" />
    <path d={powerPath} class="power" fill="none" />
  </svg>
  <div class="legend">
    <span class="l power">Puissance ~{Math.round(peakPower)}</span>
    <span class="l torque">Couple ~{Math.round(peakTorque)}</span>
  </div>
</div>

<style>
  .chart {
    border: 1px solid var(--line);
    background: var(--bg);
    padding: 6px;
  }
  svg {
    width: 100%;
    height: 110px;
    display: block;
  }
  .power {
    stroke: var(--rosso-bright);
    stroke-width: 1.4;
  }
  .torque {
    stroke: var(--blue);
    stroke-width: 1.4;
  }
  .legend {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    font-size: 10px;
    font-family: var(--mono);
  }
  .l.power {
    color: var(--rosso-bright);
  }
  .l.torque {
    color: var(--blue);
  }
</style>
