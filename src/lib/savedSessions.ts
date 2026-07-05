// Sessions sauvegardées nommées (§8.4bis) : distinctes des presets par type
// (« dernier réglage utilisé pour ce type ») — une sauvegarde nommée capture
// un instantané complet et rappelable à la demande (surtout utile pour ne pas
// reperdre un plateau d'adversaires soigneusement ajusté).
import type { GridMode, RaceSetup } from "./launch";

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

const KEY = "pitbox.savedSessions";

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

/** Les plus récentes d'abord. */
export function listSavedSessions(): SavedSession[] {
  return Object.values(loadAll()).sort((a, b) => b.savedAt.localeCompare(a.savedAt));
}

/** Enregistre (ou écrase si le nom existe déjà) une session. */
export function saveSession(session: SavedSession): void {
  const all = loadAll();
  all[session.name] = session;
  persist(all);
}

export function deleteSavedSession(name: string): void {
  const all = loadAll();
  delete all[name];
  persist(all);
}
