// Sessions sauvegardées nommées (§8.4bis) : distinctes des presets par type
// (« dernier réglage utilisé pour ce type ») — une sauvegarde nommée capture
// un instantané complet et rappelable à la demande (surtout utile pour ne pas
// reperdre un plateau d'adversaires soigneusement ajusté).
import { invoke } from "@tauri-apps/api/core";
import { centerSpreadOf } from "./aiBand";
import { assistLevelFrom, type RaceSetup, type SessionType } from "./launch";
import { StorageKey } from "./storage";

export type Season = "" | "spring" | "summer" | "autumn" | "winter";

export interface SavedSession {
  name: string;
  savedAt: string;
  setup: RaceSetup;
  opponentCount: number;
  /** Vivier d'adversaires (§3.3), sérialisé comme les filtres de bibliothèque.
   * `undefined` sur une sauvegarde antérieure aux jetons : le chargement
   * reconstruit alors le vivier depuis `gridMode`/`categorySelection`, gardés
   * pour cette seule relecture et jamais réécrits. */
  gridFilters?: string;
  gridPinned?: string[];
  gridMode?: "same_car" | "same_category" | "free";
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
  if (Object.keys(fromRust).length > 0) return migrate(fromRust);
  // Repli sur l'ancien `localStorage` seulement si le nouveau fichier n'a
  // rien (première ouverture après la mise à jour) — et dans ce cas,
  // persiste tout de suite au nouvel endroit pour ne plus jamais redépendre
  // de `localStorage`.
  const legacy = loadLegacyAll();
  if (Object.keys(legacy).length > 0) await persist(migrate(legacy));
  return migrate(legacy);
}

/** Champs d'un instantané qui ont changé de forme depuis qu'il a été écrit.
 *
 * Passe sur **toutes** les entrées, pas sur celles du type courant : chaque
 * sauvegarde porte son propre `setup`, et n'en convertir qu'une partie
 * laisserait des booléens orphelins qui retomberaient en silence sur le défaut
 * au prochain chargement. Ici plutôt qu'au chargement d'une session : c'est le
 * seul passage obligé des trois opérations (lister, enregistrer, supprimer),
 * et `saveSession` réécrit le tout, ce qui rend la conversion durable sans
 * écriture dédiée. Idempotent — une entrée déjà convertie porte son niveau et
 * l'ancien booléen n'est plus regardé. */
function migrate(all: Record<string, SavedSession>): Record<string, SavedSession> {
  for (const s of Object.values(all)) {
    const old = s.setup as Partial<{
      abs_auto: boolean;
      traction_control_auto: boolean;
      ai_level_min: number;
      ai_level_max: number;
    }>;
    s.setup.abs = assistLevelFrom(s.setup.abs, old.abs_auto);
    s.setup.traction_control = assistLevelFrom(s.setup.traction_control, old.traction_control_auto);
    // Difficulté : deux bornes avant le modèle centre ± écart (§2.9). Converti
    // plutôt que repli sur le défaut — une session enregistrée porte souvent un
    // réglage ajusté longuement, et le voir se réinitialiser en la rechargeant
    // est pire que tout. Idempotent : une entrée déjà convertie porte son
    // centre, et les anciennes bornes ne sont plus regardées.
    if (s.setup.ai_level == null && old.ai_level_min != null && old.ai_level_max != null) {
      const band = centerSpreadOf(old.ai_level_min, old.ai_level_max);
      s.setup.ai_level = band.center;
      s.setup.ai_spread = band.spread;
    }
    s.setup.aggression_spread ??= 0;
  }
  return all;
}

function persist(all: Record<string, SavedSession>): Promise<void> {
  return invoke<void>("save_saved_sessions", { all }).catch((e) => console.error("save_saved_sessions", e));
}

/**
 * **Toutes** les sauvegardes, celles du type courant en tête (§2.11).
 *
 * Le filtre par type a été retiré parce qu'il était **invisible**. Quelqu'un
 * qui avait enregistré une session en Course et la cherchait depuis Practice ne
 * voyait pas une liste filtrée : il voyait une liste vide, et en concluait que
 * sa sauvegarde avait échoué. Charger une session bascule le type — il fait
 * partie de ce qui est enregistré —, donc la charger depuis un autre type est
 * une opération parfaitement valide qu'il n'y avait aucune raison de masquer.
 *
 * Le bénéfice pratique du filtre est conservé par le **tri** : ce qu'on cherche
 * le plus souvent est en tête, et rien n'est caché.
 */
export async function listSavedSessions(sessionType: SessionType): Promise<SavedSession[]> {
  const all = await loadAll();
  return Object.values(all).sort((a, b) => {
    const am = a.setup.session_type === sessionType ? 0 : 1;
    const bm = b.setup.session_type === sessionType ? 0 : 1;
    return am !== bm ? am - bm : b.savedAt.localeCompare(a.savedAt);
  });
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
