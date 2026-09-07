// La flaque de lumière du sol du studio (docs/SPEC-preview-3d-kn5.md §8.1).
//
// Extrait de `CarPreview3D` le jour où les vignettes de la grille en ont eu
// besoin aussi (SPEC-grille §5.6) : le sol du preset Vitrine est exactement
// celui de l'aperçu de la fiche, et le recopier aurait été la deuxième copie
// d'une brique qui dérive dès qu'elle en a deux.
//
// Elle vit à côté de `floorMirror.ts`, qui porte l'autre moitié du sol — le
// reflet. Les deux se composent : la flaque dit qu'il y a un sol, le reflet
// dit qu'il est laqué, et l'ombre portée dit que la voiture le touche.
import type * as ThreeModule from "three";

/**
 * Le dégradé peint du sol : une flaque de lumière, avec un assombrissement de
 * contact en son centre.
 *
 * L'intensité n'est **pas** un paramètre : elle est portée par l'opacité du
 * matériau qui reçoit cette texture, ce qui permet de la régler sans repeindre
 * un canevas de 512 px à chaque mouvement de curseur.
 */
export function poolTexture(THREE: typeof ThreeModule): ThreeModule.Texture {
  const size = 512;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const middle = size / 2;
    // Parti de la photo — le fond d'un `preview.jpg` passe de rgb(2,3,5) dans
    // les coins à rgb(12,13,15) sous la voiture — puis remonté d'un cran : ici
    // le sol est aussi le seul repère de profondeur, alors que la photo, elle,
    // montre le décor du showroom autour.
    const pool = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle);
    pool.addColorStop(0, "rgba(255,255,255,0.11)");
    pool.addColorStop(0.45, "rgba(255,255,255,0.066)");
    pool.addColorStop(0.75, "rgba(255,255,255,0.018)");
    pool.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = pool;
    ctx.fillRect(0, 0, size, size);

    // Assombrissement de contact seulement. L'ombre de la voiture, elle, est
    // **projetée** (lumière directionnelle + `ShadowMaterial`) ; ce dégradé ne
    // fait que noircir le dernier centimètre sous la caisse, là où une carte
    // d'ombre manque toujours de résolution.
    const contact = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle * 0.34);
    contact.addColorStop(0, "rgba(0,0,0,0.3)");
    contact.addColorStop(1, "rgba(0,0,0,0)");
    ctx.fillStyle = contact;
    ctx.fillRect(0, 0, size, size);
  }
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}
