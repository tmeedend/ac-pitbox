// Sessions sauvegardées nommées (§8.4bis) : distinctes des presets par type
// (« dernier réglage utilisé pour ce type ») — une sauvegarde nommée capture
// un instantané complet et rappelable à la demande (surtout utile pour ne pas
// reperdre un plateau d'adversaires soigneusement ajusté).
import { invoke } from "@tauri-apps/api/core";
import type { GridMode, RaceSetup, SessionType } from "./launch";
import { StorageKey } from "./storage";

export type Season = "" | "spring" | "summer" | "autumn" | "winter";

export interface SavedSession {
  name: string;
  savedAt: string;
  setup: RaceSetup;
  gridMode: GridMode;
  opponentCount: number;
  /** §8.6 : `SAME_CATEGORY` ou une catégorie fixée à la main. `undefined` sur
   * une sauvegarde antérieure à ce champ — même repli qu'à la relecture d'un
   * preset, `SAME_CATEGORY` reproduit exactement l'ancien comportement
   * implicite (toujours suivre la voiture pilotée). */
  categorySelection?: string;
  season: Season;
  /** Intention météo sélectionnée (pour resurligner la bonne carte à la relecture). */
  intent: string;
  /** Skins de circuit actifs au moment de la sauvegarde (§8, plusieurs
   * possibles). Voiture, skin piloté, circuit et tracé sont déjà dans `setup` ;
   * les skins de circuit, eux, ne sont pas un réglage de session mais un état
   * de déploiement — d'où ce champ séparé. `undefined` sur une sauvegarde
   * antérieure à ce champ : le chargement n'y touche alors pas du tout, plutôt
   * que de prendre une liste vide pour « aucun skin actif » et de désactiver
   * ce que l'utilisateur avait mis en place. */
  trackSkins?: string[];
}

/** Clé de stockage préfixée par type (§8.4bis, carte « Sessions enregistrées » :
 * une liste par type de session) — sans ça, une sauvegarde « Test » en Course
 * écraserait une sauvegarde « Test » en Practice, deux choses sans rapport
 * pour l'utilisateur. */
function keyFor(sessionType: SessionType, name: string): string {
  return `${sessionType}::${name}`;
}

/** Ancien mécanisme (avant fix) : lu une seule fois pour migrer les
 * sauvegardes déjà faites, jamais réécrit. `localStorage` n'est pas garanti
 * synchrone sur disque côté WebView2 — une sauvegarde nommée juste avant de
 * fermer l'app pouvait ne jamais atteindre le disque (même bug réel que le
 * duo de session/les presets, voir `nav.svelte.ts`/`session_state.rs`). */
function loadLegacyAll(): Record<string, SavedSession> {
  try {
    return JSON.parse(localStorage.getItem(StorageKey.savedSessions) ?? "{}");
  } catch {
    return {};
  }
}

/** Persistance durable (§8.4bis) : fichier écrit côté Rust
 * (`saved_sessions.json`, `std::fs::write` synchrone), pas `localStorage` —
 * voir `loadLegacyAll` pour le pourquoi du changement. */
async function loadAll(): Promise<Record<string, SavedSession>> {
  const fromRust = await invoke<Record<string, SavedSession>>("get_saved_sessions").catch(() => ({}));
  if (Object.keys(fromRust).length > 0) return fromRust;
  // Repli sur l'ancien `localStorage` seulement si le nouveau fichier n'a
  // rien (première ouverture après la mise à jour) — et dans ce cas,
  // persiste tout de suite au nouvel endroit pour ne plus jamais redépendre
  // de `localStorage`.
  const legacy = loadLegacyAll();
  if (Object.keys(legacy).length > 0) await persist(legacy);
  return legacy;
}

function persist(all: Record<string, SavedSession>): Promise<void> {
  return invoke<void>("save_saved_sessions", { all }).catch((e) => console.error("save_saved_sessions", e));
}

/** Sauvegardes du type de session donné, les plus récentes d'abord. */
export async function listSavedSessions(sessionType: SessionType): Promise<SavedSession[]> {
  const all = await loadAll();
  return Object.values(all)
    .filter((s) => s.setup.session_type === sessionType)
    .sort((a, b) => b.savedAt.localeCompare(a.savedAt));
}

/** Enregistre (ou écrase si le nom existe déjà pour ce type) une session. */
export async function saveSession(session: SavedSession): Promise<void> {
  const all = await loadAll();
  all[keyFor(session.setup.session_type, session.name)] = session;
  await persist(all);
}

export async function deleteSavedSession(sessionType: SessionType, name: string): Promise<void> {
  const all = await loadAll();
  delete all[keyFor(sessionType, name)];
  await persist(all);
}

/** Date de sauvegarde, dans le fuseau de l'utilisateur.
 *
 * `savedAt` est un ISO **UTC** (`new Date().toISOString()`), et le tronquer à
 * la main (`iso.slice(0, 16)`) affichait donc l'heure UTC : une sauvegarde
 * faite à 14 h en France s'affichait « 12:00 ». Le stockage reste en UTC —
 * c'est ce qui rend le tri par `localeCompare` correct — seul l'affichage
 * repasse en heure locale. Format volontairement fixe (ISO court) plutôt que
 * `toLocaleString` : la même colonne monospace pour les six locales. */
export function formatSavedAt(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso.slice(0, 16).replace("T", " ");
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
