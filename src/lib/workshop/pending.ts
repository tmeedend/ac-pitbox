// Pont typé vers les dossiers proposés par l'auteur (§4.6ter) : ce qu'une
// archive livre à côté du mod sans que le disque dise quoi en faire.
//
// La liste se lit **en base**, jamais dans le rapport en mémoire : ne rien
// décider est une réponse valable, donc ce qui attend doit survivre à une
// fermeture de l'app.
import { invoke } from "@tauri-apps/api/core";

/** Formes reconnues — miroir des constantes de `pending.rs`. Elles ne décident
 * de rien : elles choisissent ce qu'on montre et ce qu'on pré-remplit. */
export type PendingShape = "jsgme" | "gameTree" | "skinVariant" | "documents" | "unknown";

/** Sorts possibles. `discard` est le seul qui supprime — voir §4.5.3. */
export type PendingAction = "game" | "layer" | "resources" | "other" | "discard";

export interface PendingFolder {
  id: string;
  archive: string;
  /** Chemin dans l'archive : c'est ce qui identifie le dossier pour
   * l'utilisateur, bien mieux que l'id interne. */
  rel_path: string;
  owner_id: string | null;
  /** "cars" | "tracks" | "apps". */
  owner_kind: string | null;
  shape: PendingShape;
  /** Titre écrit par l'auteur (première ligne d'un `description.jsgme`). */
  title: string | null;
  description: string | null;
  /** Nom du document d'explication trouvé dans le dossier. */
  readme: string | null;
  /** Contenu dont ce dossier recouvrirait les livrées. */
  skin_target: string | null;
  /** Fichiers du jeu de base qu'il remplacerait (§4.6bis) : le seul chiffre qui
   * dit « ceci ne concerne pas que ce mod ». */
  replaced: number;
  file_count: number;
  size_bytes: number;
  /** Pre-filled answer; `""` when the app has no opinion (§4.6bis). */
  suggestion: PendingAction | "";
  /** Actions qui ont un sens pour ce dossier-ci, la proposition en tête. */
  actions: PendingAction[];
}

export function listPendingFolders(): Promise<PendingFolder[]> {
  return invoke<PendingFolder[]>("list_pending_folders");
}

export function resolvePendingFolder(id: string, action: PendingAction): Promise<void> {
  return invoke<void>("resolve_pending_folder", { id, action });
}

/** Contenu texte de la notice d'un dossier proposé, pour la lire sans quitter
 * l'écran où la décision se prend — le va-et-vient vers l'explorateur est
 * précisément ce qui fait cliquer au hasard (§4.6bis). */
export function readPendingDocument(id: string, name: string): Promise<string> {
  return invoke<string>("read_pending_document", { id, name });
}

/** Several pending folders asked as one question. */
export interface PendingGroup {
  /** Stable render key: the grouping key, or the folder id for a lone one. */
  key: string;
  folders: PendingFolder[];
}

/** Last segment of an archive path — the name the author gave the folder. */
export function folderName(relPath: string): string {
  const parts = relPath.split("/").filter((p) => p.length > 0);
  return parts[parts.length - 1] ?? relPath;
}

/**
 * What makes two questions the same question: the folder the author named the
 * same way, recognised as the same shape, offered the same answers. Twenty VRC
 * cars each delivering `Wallpapers/` ask one thing twenty times.
 *
 * `null` = never grouped. A folder that replaces base-game files stays alone:
 * that answer changes every session, not one mod (§4.6bis), and it is read
 * against its own count of replaced files.
 */
export function groupKey(f: PendingFolder): string | null {
  if (f.replaced > 0) return null;
  return [folderName(f.rel_path).toLowerCase(), f.title ?? "", f.shape, f.actions.join(","), f.suggestion].join("|");
}

/** Pending folders as questions, in the order of their first folder. Groups
 * whose key is in `split` are asked folder by folder, as the user asked. */
export function groupPending(folders: PendingFolder[], split: ReadonlySet<string>): PendingGroup[] {
  const groups: PendingGroup[] = [];
  const byKey = new Map<string, PendingGroup>();
  for (const f of folders) {
    const key = groupKey(f);
    if (key === null || split.has(key)) {
      groups.push({ key: `id:${f.id}`, folders: [f] });
      continue;
    }
    const known = byKey.get(key);
    if (known) {
      known.folders.push(f);
    } else {
      const g = { key, folders: [f] };
      byKey.set(key, g);
      groups.push(g);
    }
  }
  return groups;
}
