// Rendu localisé des entrées d'historique (§3.2). Le backend stocke désormais
// les détails sous forme de payload structuré JSON (`{ key, ...params }`) ; on
// le traduit ici via `history.<key>`. Les anciennes lignes (texte brut FR
// d'avant la passe i18n) ne sont pas du JSON valide → affichées telles quelles.
import { t } from "$lib/i18n/index.svelte";

/** Libellé localisé de l'événement (badge). Repli sur le code brut si absent. */
export function historyEventLabel(event: string): string {
  const key = `history.event.${event}`;
  const label = t(key);
  return label === key ? event : label;
}

/** Texte localisé du détail d'une entrée d'historique. */
export function historyDetails(details: string): string {
  try {
    const p = JSON.parse(details) as { key?: string } & Record<string, unknown>;
    if (p && typeof p === "object" && typeof p.key === "string") {
      return t(`history.${p.key}`, p as Record<string, string | number>);
    }
  } catch {
    /* ligne héritée : texte brut, affiché tel quel */
  }
  return details;
}
