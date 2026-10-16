// Which tracks the public servers run — a developer tool, never shipped.
//
// **Why.** The Online page's track category filter only knows the categories
// of the tracks one owns (SPEC-play-online.md); the catalogue of track
// categories (`taxonomy-catalog.json`, `track_categories`) fills in the
// others. Filling it for the 2 000 tracks the lobby names would be endless;
// filling it for the few dozen that hold most of the players is an evening.
// This tool says which ones, and how much of the lobby they cover.
//
// **What it reads.** The lobby list as Pit Box last received it: the backup
// `online/lobby_cache.rs` writes to `%LOCALAPPDATA%\com.pitbox.app\lobby.json`
// (the lobby itself wants a Steam id). Or a file given as argument. One list
// is one moment: players move with the hour and the day — say when it was
// taken when quoting the numbers.
//
// **What it prints.** Per track id (the folder, layouts merged): servers,
// servers with at least one player, players. Ranked by players, with the
// cumulative coverage of the first 30 and 50 ids — by players and by active
// servers, each in its own ranking.
//
//   node scripts/lobby-tracks.mjs [lobby.json] [--top N]

import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const topAt = args.indexOf("--top");
const top = topAt >= 0 ? Number(args[topAt + 1]) : 50;
const fileArg = args.find((a, i) => !a.startsWith("--") && args[i - 1] !== "--top");
const file = fileArg ?? join(process.env.LOCALAPPDATA ?? "", "com.pitbox.app", "lobby.json");

/** The track's folder id, as `online/lobby.rs::decode_track` reads it: the
 * CSP prefix (`csp/<build>/../`) dropped, then the layout after the last `-`. */
export function trackId(raw) {
  const body = raw.startsWith("csp/") ? raw.slice(4) : raw;
  const pieces = body.split("/../");
  const kunosId = pieces[pieces.length - 1] ?? "";
  const dash = kunosId.lastIndexOf("-");
  const id = dash > 0 && dash < kunosId.length - 1 ? kunosId.slice(0, dash) : kunosId;
  return id.toLowerCase();
}

const entries = JSON.parse(readFileSync(file, "utf8"));
const taken = statSync(file).mtime;

// The lobby repeats a few servers: `ip:cport` is the identity the app keys
// its rows on (`parse_lobby`), the same here.
const seen = new Set();
const tracks = new Map();
for (const e of entries) {
  if (typeof e.track !== "string" || !e.ip) continue;
  const key = `${e.ip}:${e.cport}`;
  if (seen.has(key)) continue;
  seen.add(key);
  const id = trackId(e.track);
  const clients = Number(e.clients) || 0;
  const t = tracks.get(id) ?? { id, servers: 0, active: 0, players: 0 };
  t.servers += 1;
  if (clients > 0) t.active += 1;
  t.players += clients;
  tracks.set(id, t);
}

const all = [...tracks.values()];
const total = all.reduce(
  (a, t) => ({ servers: a.servers + t.servers, active: a.active + t.active, players: a.players + t.players }),
  { servers: 0, active: 0, players: 0 },
);

/** Share of `metric` the first `n` ids hold, in the ranking by that metric. */
function coverage(metric, n) {
  const ranked = [...all].sort((a, b) => b[metric] - a[metric]);
  const sum = ranked.slice(0, n).reduce((a, t) => a + t[metric], 0);
  return total[metric] ? (100 * sum) / total[metric] : 0;
}

const pct = (x) => `${x.toFixed(1)} %`;
console.log(`Lobby list of ${taken.toISOString().slice(0, 16).replace("T", " ")} UTC (${file})`);
console.log(
  `${total.servers} servers, ${total.active} with players, ${total.players} players, on ${all.length} track ids`,
);
console.log("");
for (const n of [30, 50]) {
  console.log(
    `first ${n} ids: ${pct(coverage("players", n))} of the players, ${pct(coverage("active", n))} of the active servers`,
  );
}
console.log("");

const byPlayers = [...all].sort((a, b) => b.players - a.players || b.active - a.active || a.id.localeCompare(b.id));
let cumPlayers = 0;
let cumActive = 0;
console.log("rank | track id | servers | active | players | cum. players | cum. active");
console.log("---: | --- | ---: | ---: | ---: | ---: | ---:");
byPlayers.slice(0, top).forEach((t, i) => {
  cumPlayers += t.players;
  cumActive += t.active;
  console.log(
    `${i + 1} | ${t.id} | ${t.servers} | ${t.active} | ${t.players} | ${pct((100 * cumPlayers) / total.players)} | ${pct((100 * cumActive) / total.active)}`,
  );
});
