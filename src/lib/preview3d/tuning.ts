// The numbers of the car preview (docs/SPEC-preview-3d-kn5.md PREVIEW§15):
// turntable speed, lens and framing, quality levels, every rendering knob,
// the intro and the driver's fade. Each one carries the measurement or the
// report that decided it — the reason they are gathered in one file, out of
// `CarPreview3D.svelte`: one place to change a value, reopen a car sheet and
// look.
//
// None of them enters the model conversion: changing one invalidates no cache
// entry and shows on the next frame.

// Plateau tournant, comme un socle de salon : c'est la raison d'être de tout
// ce chantier — voir la voiture tourner, pas seulement pouvoir la tourner.
//
// Le PREVIEW§8.4 de la spec demande l'inverse (« ne pas rendre en continu à 60 fps
// sur un panneau statique ») et les deux sont inconciliables. La contrepartie
// est donc payée là où elle se voit : la rotation s'arrête dès que la fiche
// quitte l'écran, que la fenêtre passe en arrière-plan, ou que l'utilisateur
// attrape le modèle — un panneau qu'on ne regarde pas ne consomme rien.

/** Un tour en ~28 s à 100 % : assez lent pour être calme, assez vif pour
 * qu'on voie les reflets glisser sur la carrosserie. */
export const SPIN_SPEED = 0.22;

// Lens and framing, measured against Kunos' `preview.jpg` (PREVIEW§15 point 7).
//
// A 20° field of view rather than the 35° first used: at 35° the nose of a
// car looms and its tail falls away, a distortion the game's own previews do
// not have. The distance that follows is what puts the car at the size Kunos
// frames it — the two go together, since a longer lens has to step back.
export const FRAMING_FOV = 20;
/** Camera distance at zoom 100 %, in multiples of the model's radius. */
export const FRAMING_DISTANCE = 4.9;

// Qualité de rendu (PREVIEW§15). Ne touche **que** l'affichage : aucun de ces
// réglages n'entre dans la conversion, donc en changer n'invalide aucune
// entrée de cache et s'applique à l'image suivante.
//
// **Un seul levier, le suréchantillonnage**, et c'est le résultat d'un essai
// mené jusqu'au bout plutôt qu'un choix de départ. Une passe SMAA a été
// ajoutée, déplacée d'un espace colorimétrique à l'autre, puis retirée :
// comparée à l'écran sur le cas le plus défavorable qui soit — un jonc
// chromé quasi horizontal d'un pixel de haut sur fond noir — elle n'a jamais
// produit de différence visible, alors qu'elle imposait un `EffectComposer`,
// donc **deux** cibles RGBA16F multi-échantillonnées (il clone la sienne)
// plus ses deux tampons internes : près d'un gigaoctet de mémoire graphique
// sur une fiche large. Le suréchantillonnage, lui, se voit (PREVIEW§15 point 8).
//
// Ce qu'il faut retenir si l'idée revient : il ne suffit pas d'ajouter la
// passe, il faut prouver qu'elle se voit — et sur ce panneau, elle ne se
// voyait pas.
//
// Retirer la chaîne rend au passage le MSAA du contexte (`antialias: true`)
// à tous les niveaux, et avec lui `alphaToCoverage` sur les découpes en
// alpha : le montage post-traitement les avait justement contournés.
// ------------------------------------------------------------------------
// **Et le facteur ne peut valoir que la densité de l'écran, ou son double.**
//
// Ce n'est pas un choix de confort, c'est une contrainte mesurée, et elle a
// coûté trois niveaux de réglage qui dégradaient l'image au lieu de
// l'améliorer. Le canevas est dessiné plus grand que le panneau, puis c'est
// le **compositeur du navigateur** qui le réduit à la taille écran — avec
// une seule prise bilinéaire, sans mipmap. Le résultat ne dépend donc pas du
// facteur mais du **rapport de réduction**, et il n'est bon qu'à 2 :
//
//   réduction 2 → le centre du pixel de sortie tombe sur le coin entre
//                 quatre texels : la bilinéaire les lit à poids égaux, c'est
//                 un filtre boîte 2×2 exact, et il est gratuit ;
//   réduction 3 → il tombe sur le **centre** d'un texel : la bilinéaire
//                 dégénère en plus proche voisin et ne lit qu'un texel sur
//                 neuf. Le pire cas de tous ;
//   réduction 4 → coin à nouveau, mais 4 texels lus sur 16 ;
//   non entier  → les prises dérivent, les poids se déséquilibrent, des
//                 texels sont sautés.
//
// Mesuré hors application (rendu de lignes claires quasi horizontales à
// chaque facteur, réduction bilinéaire sur GPU, comparaison à un filtre
// boîte depuis un rendu 16×), écart quadratique moyen sur 255 :
//
//   réduction  1,00  1,33  1,50  1,67  2,00  2,50  2,67  3,00  4,00
//   RMS        7,69  9,98  8,87 11,08  4,97 12,17 13,47 21,34 15,43
//
// Sur un écran à 1,5 — le cas de l'utilisateur — les anciens niveaux
// donnaient 1,00 / 1,67 / 2,67 : les deux niveaux « qualité » étaient
// **mesurablement pires que de ne rien faire**, et Ultra le pire des deux.
// C'est exactement ce que l'utilisateur voyait, et ça explique après coup le
// « aucune différence entre Standard et Ultra » du début du chantier ainsi
// que le « 5× ne se distingue pas de 4× ».
//
// Il ne reste donc que deux valeurs utiles, d'où deux niveaux et non trois.
// Aller au-delà demanderait de faire la réduction soi-même (cible hors écran
// à 4×, passe de filtre boîte) — donc un tone mapping à refaire à la main,
// `alphaToCoverage` perdu, et 133 Mio de cible : le montage qui a déjà été
// construit puis retiré une fois. Le gain irait de 4 à 16 échantillons par
// pixel écran ; à reprendre le jour où quelqu'un prouve qu'il se voit.
export const QUALITY = {
  /** Un pixel de tampon pour un pixel d'écran : rien de plus. */
  standard: { oversampling: 1 },
  /** Le double, la seule autre valeur que le compositeur sache réduire. */
  high: { oversampling: 2 },
} as const;

/**
 * **Every rendering knob, gathered here on purpose.** Change a value, reopen
 * a car sheet, look.
 *
 * Nothing below enters the model conversion, so no value here invalidates a
 * cache entry: a change shows up on the next frame, never after a reconversion.
 * Each entry says what it does and what is worth trying.
 *
 * Two things deliberately stay outside this object:
 *
 * - the **oversampling factor**, in `QUALITY` just above. It is the one value
 *   that cannot be freely picked — see the long comment there, and the table
 *   of measurements that goes with it;
 * - the **anisotropic filtering**, left at whatever the card's maximum is
 *   (16 in practice). There is no reason to want less, and no way to want
 *   more.
 */
export const TUNING = {
  /**
   * Budget of the drawing buffer, in pixels.
   *
   * On the **area**, not on the factor: the window decides the size of the
   * panel, and the level must not multiply it without limit. An allocation
   * that fails does not degrade the image — it loses the WebGL context and
   * leaves the panel black. 16 Mpx of multisampled RGBA fits in ~256 MiB.
   * Lower it if a very large window ever turns the preview black.
   */
  drawingPixels: 16_000_000,

  /**
   * Budget of the mirror's own target, in pixels.
   *
   * Lower than the drawing buffer's on purpose: the reflection is blurred
   * then faded out, so it gains nothing from a resolution far past the
   * screen. 8 Mpx without MSAA fits in ~96 MiB. **This is the first knob to
   * turn if the floor shimmers or if the panel drops frames** — halving it
   * halves the cost of the mirror pass, which is a second render of the whole
   * scene.
   */
  mirrorPixels: 8_000_000,

  /**
   * MSAA samples on the mirror pass. Zero on purpose.
   *
   * The memory goes into resolution instead: MSAA samples triangle coverage
   * but still shades once per texel, so it does nothing for a sub-pixel
   * specular highlight, while the target now follows a supersampled drawing
   * buffer. Set it to 4 to trade the resolution back for coverage quality —
   * it costs four times the memory of the mirror target.
   */
  mirrorSamples: 0,

  /**
   * Shadow map resolution — **and the softness of the shadow**, which is the
   * counter-intuitive part.
   *
   * `PCFSoftShadowMap` has a fixed filter kernel counted in *texels*, so
   * fewer texels means a wider blur. 512 gives the soft edge of a studio
   * light; raising it makes the shadow harder, not better. It also crawls as
   * the car turns, so lowering it further trades a soft edge for a mushy one.
   */
  shadowMapSize: 512,

  /**
   * Shadow map depth bias. Without it the map self-shadows in fine stripes on
   * surfaces nearly parallel to the light — the bonnet, the roof. More
   * negative pushes the stripes away but detaches the shadow from the car.
   */
  shadowBias: -0.0015,

  /**
   * **The two knobs against shimmer in motion**, and they are the only two
   * that measured as working. Both attack the same cause and they add up.
   *
   * The cause: the car turns while the studio stays put, so the reflection of
   * a ceiling strip sweeps across the bodywork. On a near-mirror surface that
   * reflection is a band **narrower than a pixel**, and a pixel-wide band
   * crossing a pixel grid flickers. It is perfectly still on a screenshot,
   * which is what makes it so hard to chase — and why neither supersampling
   * nor any post-pass ever touched it.
   *
   * Measured on a bench outside the app (a near-mirror knot turning half a
   * degree between two frames, eight pairs, counting the pixels that jump by
   * more than 40 out of 255 — a few pixels flipping hard is what reads as
   * sparkle, not many pixels drifting a little):
   *
   * | `environmentBlur` | `roughnessFloor` | pixels jumping >40 | luminance |
   * | --- | --- | --- | --- |
   * | 0,04 | 0 | 1,50 % (référence) | 36,1 |
   * | 0,08 | 0 | 1,34 % (−11 %) | 38,0 |
   * | 0,04 | 0,15 | 1,21 % (−19 %) | 39,1 |
   * | **0,08** | **0,15** | **0,97 % (−35 %)** | 40,5 |
   * | 0,15 | 0,15 | 0,92 % (−38 %) | 41,0 |
   * | 0,30 | 0,15 | 0,92 % (−38 %) | 41,1 |
   *
   * Two things to read off that table. The effect **saturates around
   * 0,08–0,15**: past that, more blur costs contrast and buys nothing. And the
   * last column is the price — the surfaces come out about 12 % brighter and
   * flatter, chrome least like a mirror.
   *
   * **The shipped pair is 0,08 / 0,15**, the marked row: the last one that
   * still buys something. Since the price is a matter of taste and not of
   * correctness, the values were put to the user and are his — same as the
   * framing defaults (point 14). `environmentBlur: 0.04` with
   * `roughnessFloor: 0` restores exactly the look the app had before.
   *
   * ⚠️ **The quality level has nothing to do with any of this.** Measured on
   * the same bench, modelling the compositor: 1,49 % of violent pixels at
   * Standard, 1,47 % at Élevée, 1,48 % at the old Ultra — flat. Oversampling
   * decides the *static* quality of edges and nothing else. A sharper image
   * simply makes the same flicker easier to read, which is what made it look
   * worse at the higher level. Lowering the level to hide it would blur the
   * whole image to mask a defect that has its own remedy.
   */

  /**
   * Blur of the studio environment map — the `sigma` of `PMREMGenerator`.
   *
   * Softens every reflection at once, and costs nothing per frame: it is baked
   * into the environment map when the scene is built. 0,04 was the value the
   * app shipped with before the flicker was measured.
   */
  environmentBlur: 0.08,

  /**
   * Floor under the roughness of every material, 0 to disable.
   *
   * A perfect mirror (roughness near zero) has a highlight of *zero* width,
   * which no amount of sampling can resolve — AC's chrome and glass land
   * there. A floor gives those highlights a width. Applied per pixel, inside
   * the shader, because a plain `material.roughness` would only *scale* a
   * roughness map instead of lifting its dark parts.
   *
   * ⚠️ **Geometric specular antialiasing was tried here and removed.** The
   * textbook remedy (Kaplanyan/Frostbite: fold the screen derivative of the
   * normal into the roughness) measured at **exactly nothing** — 1,50 % of
   * violent pixels against 1,51 %, unchanged even at four times the standard
   * strength. It keys on the normal varying fast across one pixel, which
   * happens when geometry is undersampled; these cars are densely tessellated
   * and fill the frame, so their normals barely move from pixel to pixel. The
   * sparkle is in the sharpness of the reflection, not in the geometry. Do not
   * re-add it without measuring first.
   */
  roughnessFloor: 0.15,
};

// Effet d'entrée du plateau (PREVIEW§15). Deux gestes, et rien d'autre qu'un
// facteur appliqué à la vitesse déjà calculée : aucune image de plus, aucun
// coût GPU.
/** Montée en douceur jusqu'à la vitesse réglée. */
export const INTRO_RAMP_MS = 1200;
/** Départ lancé : la voiture part à `1 + BOOST` fois la vitesse réglée et
 * décroît vers elle. Durée choisie pour que le dernier dixième de l'écart
 * soit déjà imperceptible quand on coupe. */
export const INTRO_LAUNCH_MS = 2600;
export const INTRO_LAUNCH_BOOST = 4;
export const INTRO_LAUNCH_TAU_MS = 900;

/** Durée du fondu du pilote, en secondes. Assez court pour qu'on ne
 * l'attende pas, assez long pour qu'on le voie : c'est une arrivée, pas une
 * apparition. */
export const DRIVER_FADE_SECONDS = 0.45;
