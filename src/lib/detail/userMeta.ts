// Pont typé vers la saisie utilisateur commune à toutes les entités
// (REFONTE§6.1 et §9) : une note libre et un nom d'affichage.
//
// Un seul couple de commandes plutôt qu'un par type : le front dit de quelle
// entité il parle, le backend résout la table. Ajouter un type plus tard coûte
// une valeur d'énumération, pas deux commandes de plus.
import { invoke } from "@tauri-apps/api/core";

/** Doit rester aligné sur `usermeta::EntityKind` côté Rust. */
export type EntityKind = "MOD" | "SUB_MOD" | "APP" | "OTHER" | "LAYER";

/**
 * Enregistre la note libre d'une entité. Une chaîne vide l'efface.
 *
 * **Jamais via `invokeSafe`** : son repli silencieux protège une *lecture*
 * (un réglage manquant vaut mieux qu'un écran figé), mais appliqué à une
 * écriture il rendrait « enregistré » une commande qui n'a rien écrit. L'appel
 * rejette, et l'appelant garde le texte saisi pour le redire.
 */
export function setEntityNote(kind: EntityKind, id: string, text: string): Promise<void> {
  return invoke("set_entity_note", { kind, id, text });
}

/**
 * Enregistre le nom d'affichage repris à la main. Une chaîne vide l'efface,
 * ce qui ramène le nom dérivé du fichier (§5bis.3).
 */
export function setEntityDisplayName(kind: EntityKind, id: string, name: string): Promise<void> {
  return invoke("set_entity_display_name", { kind, id, name });
}
