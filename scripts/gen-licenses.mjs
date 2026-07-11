// Génère src/lib/generated/licenses.json (écran « À propos », §12) : la liste
// des bibliothèques open source utilisées par Pit Box, avec leur licence.
// Lancé automatiquement avant `npm run dev` et `npm run build` (voir les
// scripts `predev`/`prebuild` de package.json) — jamais à écrire à la main,
// reste à jour à chaque dépendance ajoutée/retirée.
//
// Deux sources :
// - Rust  : `cargo metadata` (déjà fourni par cargo, aucun outil à installer)
//           donne le graphe résolu complet (transitif) avec la licence de
//           chaque crate telle que publiée sur crates.io.
// - npm   : dependencies + devDependencies de package.json, résolues via le
//           package.json déjà installé de chaque paquet dans node_modules
//           (version réellement installée, pas le range du manifeste).
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));

function rustLicenses() {
  const manifestPath = join(root, "src-tauri", "Cargo.toml");
  let raw;
  try {
    raw = execFileSync(
      "cargo",
      ["metadata", "--format-version=1", "--manifest-path", manifestPath],
      { encoding: "utf-8", maxBuffer: 64 * 1024 * 1024 },
    );
  } catch (e) {
    console.warn(`[gen-licenses] cargo metadata indisponible, licences Rust omises : ${e.message}`);
    return [];
  }
  const meta = JSON.parse(raw);
  const seen = new Map();
  for (const pkg of meta.packages) {
    // `source: null` = membre du workspace (Pit Box lui-même) — pas une dépendance.
    if (pkg.source == null) continue;
    const key = `${pkg.name}@${pkg.version}`;
    if (seen.has(key)) continue;
    seen.set(key, {
      name: pkg.name,
      version: pkg.version,
      license: pkg.license ?? (pkg.license_file ? `voir ${pkg.license_file}` : "non renseignée"),
      ecosystem: "rust",
    });
  }
  return [...seen.values()];
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf-8"));
}

function npmLicenses() {
  const pkg = readJson(join(root, "package.json"));
  const names = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
  const out = [];
  for (const name of names) {
    const pkgJsonPath = join(root, "node_modules", ...name.split("/"), "package.json");
    if (!existsSync(pkgJsonPath)) continue; // pas encore installé (ex. juste après clone)
    const installed = readJson(pkgJsonPath);
    let license = installed.license;
    if (!license && Array.isArray(installed.licenses) && installed.licenses.length) {
      license = installed.licenses.map((l) => l.type ?? l).join(" / ");
    }
    out.push({
      name,
      version: installed.version ?? "?",
      license: license ?? "non renseignée",
      ecosystem: "npm",
    });
  }
  return out;
}

const packages = [...rustLicenses(), ...npmLicenses()].sort((a, b) => a.name.localeCompare(b.name));

const outDir = join(root, "src", "lib", "generated");
mkdirSync(outDir, { recursive: true });
const outFile = join(outDir, "licenses.json");
writeFileSync(
  outFile,
  JSON.stringify({ generatedAt: new Date().toISOString().slice(0, 10), packages }, null, 2) + "\n",
);
console.log(`[gen-licenses] ${packages.length} paquet(s) -> ${outFile}`);
