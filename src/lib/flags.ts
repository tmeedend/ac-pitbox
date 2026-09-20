// Quel nom de la table du jeu correspond à ce que le mod a écrit.
//
// La partie **pure** de `$lib/flags` : aucune E/S, aucun cache, donc testable
// (`flags.test.ts`). Le cache et l'URL de l'image vivent à côté, dans
// `flags.svelte.ts`.

/**
 * Les orthographes qu'Assetto Corsa ne connaît pas, et le nom de sa table
 * auquel chacune renvoie. Clés et valeurs en minuscules.
 *
 * **Les quatre premières sont mesurées**, sur les 575 `ui_*.json` d'une
 * bibliothèque réelle : 17 orthographes de pays en tout, dont cinq
 * introuvables dans la table du jeu — `U.S.A.` (35 mods), `Great Britain`
 * (22), `USA` (21), `United States of America` (2), et une valeur cassée
 * traitée par `countryKey`. À elles seules, elles couvrent les 82 mods qui
 * n'avaient pas de drapeau.
 *
 * **Les quatre dernières ne le sont pas**, et c'est assumé : l'app est
 * publique, et une bibliothèque qu'on n'a pas sous la main écrira d'autres
 * noms. Elles n'entrent ici qu'à deux conditions — désigner **un seul** pays
 * sans discussion possible, et ne pas avoir déjà leur propre entrée dans la
 * table du jeu.
 *
 * **Ce qui n'y entre surtout pas : les nations britanniques.** AC donne son
 * propre drapeau à l'Écosse (SCT, quatre circuits dans le relevé), à
 * l'Angleterre, au pays de Galles et à l'Irlande du Nord. Les renvoyer sur le
 * Royaume-Uni remplacerait un drapeau juste par un autre drapeau juste, et
 * perdrait ce que l'auteur du mod a pris la peine d'écrire.
 */
export const COUNTRY_ALIASES: Record<string, string> = {
  // Mesurées.
  usa: "united states",
  "u.s.a.": "united states",
  "united states of america": "united states",
  "great britain": "united kingdom",
  // Certaines, non mesurées — aucune n'a d'entrée à elle dans la table du jeu.
  uk: "united kingdom",
  "u.k.": "united kingdom",
  holland: "netherlands",
  deutschland: "germany",
};

/**
 * La clé de la table du jeu pour ce que le mod a écrit, ou `null`.
 *
 * `isKnown` dit si une clé existe dans la table — passée en paramètre parce
 * que la table, elle, vient du disque : c'est ce qui garde cette fonction
 * pure.
 *
 * Trois passes, dans cet ordre :
 *
 *  1. le nom tel quel, insensible à la casse et aux espaces de bord — le jeu
 *     stocke un nom entier normalisé, mais un `ui_car.json` de mod écrit ce
 *     qu'il veut ;
 *  2. la table d'alias ci-dessus ;
 *  3. **la valeur cassée**, et l'auteur du mod en est la cause. Relevé sur
 *     `le_lancone` : son `ui_track.json` contient
 *     `"country": "France\", \"Corsica"`, soit la chaîne `France", "Corsica`
 *     une fois le JSON lu — l'auteur a voulu écrire deux entrées et en a
 *     produit une seule, avec des guillemets dedans. Le fichier du mod est en
 *     lecture seule (règle d'or n°1) ; ce qu'on peut faire, c'est le LIRE : un
 *     nom de pays ne contient jamais de guillemet, donc ce qui précède le
 *     premier est le nom.
 *
 * **Et surtout pas une coupe à la virgule**, qui paraîtrait faire la même
 * chose : la table du jeu en contient (`Tanzania, {United Republic of}`,
 * `Micronesia, {Federated States of}`), et les couper les rendrait
 * introuvables. Le guillemet, lui, ne peut venir que d'un fichier abîmé.
 */
export function countryKey(name: string, isKnown: (key: string) => boolean): string | null {
  const direct = match(name, isKnown);
  if (direct) return direct;
  const quote = name.indexOf('"');
  // `> 0` : un nom qui COMMENCE par un guillemet ne laisse rien à lire.
  return quote > 0 ? match(name.slice(0, quote), isKnown) : null;
}

function match(name: string, isKnown: (key: string) => boolean): string | null {
  const key = name.trim().toLowerCase();
  if (isKnown(key)) return key;
  const alias = COUNTRY_ALIASES[key];
  return alias && isKnown(alias) ? alias : null;
}
