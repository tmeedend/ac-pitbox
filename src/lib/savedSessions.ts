// Sessions sauvegardées nommées (§8.4bis) : distinctes des presets par type
// (« dernier réglage utilisé pour ce type ») — une sauvegarde nommée capture
// un instantané complet et rappelable à la demande (surtout utile pour ne pas
// reperdre un plateau d'adversaires soigneusement ajusté).
import type { GridMode, RaceSetup, SessionType } from "./launch";
import { StorageKey } from "./storage";

export type Season = "" | "spring" | "summer" | "autumn" | "winter";

export interface SavedSession {
  name: string;
  savedAt: string;
  setup: RaceSetup;
  gridMode: GridMode;
  opponentCount: number;
  season: Season;
  /** Intention météo sélectionnée (pour resurligner la bonne carte à la relecture). */
  intent: string;
}

const KEY = StorageKey.savedSessions;

/** Clé de stockage préfixée par type (§8.4bis, carte « Sessions enregistrées » :
 * une liste par type de session) — sans ça, une sauvegarde « Test » en Course
 * écraserait une sauvegarde « Test » en Practice, deux choses sans rapport
 * pour l'utilisateur. */
function keyFor(sessionType: SessionType, name: string): string {
  return `${sessionType}::${name}`;
}

function loadAll(): Record<string, SavedSession> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

function persist(all: Record<string, SavedSession>): void {
  localStorage.setItem(KEY, JSON.stringify(all));
}

/** Sauvegardes du type de session donné, les plus récentes d'abord. */
export function listSavedSessions(sessionType: SessionType): SavedSession[] {
  return Object.values(loadAll())
    .filter((s) => s.setup.session_type === sessionType)
    .sort((a, b) => b.savedAt.localeCompare(a.savedAt));
}

/** Enregistre (ou écrase si le nom existe déjà pour ce type) une session. */
export function saveSession(session: SavedSession): void {
  const all = loadAll();
  all[keyFor(session.setup.session_type, session.name)] = session;
  persist(all);
}

export function deleteSavedSession(sessionType: SessionType, name: string): void {
  const all = loadAll();
  delete all[keyFor(sessionType, name)];
  persist(all);
}
