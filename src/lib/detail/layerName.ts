// Nom lisible d'une couche (REFONTE§8.3).
//
// Une couche s'appelle `spa2022-release_V1-03.rar` : c'est le nom de l'archive
// que l'auteur a publiée, et c'est exactement l'objet qui a besoin d'être
// renommé. Ce nom-ci est **dérivé**, donc faillible, donc corrigeable à la main
// — la saisie de l'utilisateur, quand elle existe, l'emporte toujours
// (`display_name_user`).
//
// Trois retraits, dans cet ordre, et rien d'autre. Chaque règle en plus serait
// une occasion de faire disparaître un mot qui comptait : dans le doute, on
// garde.
import { withoutBrand } from "$lib/library/displayName";

/** Extensions d'archive rencontrées à l'import. Retirées seulement en fin de
 * nom : `v1.2_final` n'est pas une extension. */
const ARCHIVE_EXT = /\.(rar|zip|7z|tar|gz|bz2|xz)$/i;

/**
 * Le nom d'une couche tel qu'on le montre, à partir du nom de son archive et
 * de celui de son hôte.
 *
 * Le retrait du préfixe passe par [`withoutBrand`], qui fait déjà ce travail
 * pour les marques de voitures : comparaison insensible à la casse et aux
 * séparateurs, mais **coupe seulement sur une espace**. C'est ce qui empêche
 * l'hôte `ks_nordschleife` d'amputer `ks_nordschleife_touristenfahrten` en
 * plein milieu d'un mot.
 */
export function layerDisplayName(source: string, hostName: string | null): string {
  const withoutExt = source.replace(ARCHIVE_EXT, "");
  // Séparateurs techniques ramenés à l'espace : un nom de fichier n'a pas le
  // droit d'en porter, un titre si.
  const spaced = withoutExt.replace(/[_]+/g, " ").replace(/\s+/g, " ").trim();
  if (!spaced) return source;
  const short = hostName ? withoutBrand(spaced, hostName) : spaced;
  return short.trim() || spaced;
}
