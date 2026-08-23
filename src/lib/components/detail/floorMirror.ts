// Reflet de la voiture au sol (docs/SPEC-preview-3d-kn5.md §8.1).
//
// **Pourquoi un vrai miroir et pas un matériau brillant** — la question a coûté
// trois détours, elle mérite d'être tranchée ici une fois pour toutes : un
// matériau à carte d'environnement ne reflète que l'environnement **figé** du
// studio, jamais les objets de la scène. La voiture n'y est pas et n'y sera
// jamais. Mesuré au banc : le sol devant la voiture est identique au pixel près
// avec et sans voiture. Aucun réglage de matériau ne pouvait donc marcher ; il
// fallait une seconde passe depuis une caméra symétrique, c'est-à-dire
// `Reflector`.
//
// Ce fichier ne porte que le **shader**, parce que c'est lui qui contient les
// décisions : le `Reflector` de three.js rend un miroir parfait, net et
// infini, ce qui donne un sol mouillé de jeu vidéo. Trois choses le
// transforment en sol de salon — un flou, une extinction, une intensité — et
// les trois sont réglables par l'utilisateur.

/**
 * Shader du miroir, dérivé de `ReflectorShader` de three.js.
 *
 * Reprend sa projection (`textureMatrix` → `vUv`) et y ajoute :
 *
 * - **un flou** en 25 prises pondérées. C'est ce qui coûte, et c'est ce qui
 *   décide de l'aspect : net, un sol paraît mouillé ; flouté, il paraît laqué.
 * - **une extinction radiale**, pour que le reflet meure près de la voiture
 *   plutôt que de couvrir un sol infini. Le seuil bas est fixé à une fraction
 *   du seuil haut : le reflet reste plein sous la caisse et s'éteint ensuite.
 * - **une intensité**, portée par l'alpha — le matériau est transparent, donc
 *   le reflet se compose par-dessus la flaque et l'ombre au lieu de les
 *   remplacer.
 *
 * `vLocal` transporte les UV du plan, seule façon de savoir *où sur le sol* on
 * se trouve : `vUv` est une coordonnée projetée à l'écran, elle ne dit rien de
 * la distance à la voiture.
 */
export const floorMirrorShader = {
  uniforms: {
    color: { value: null },
    tDiffuse: { value: null },
    textureMatrix: { value: null },
    intensity: { value: 0.85 },
    blur: { value: 0.001 },
    reach: { value: 0.75 },
  },
  vertexShader: /* glsl */ `
    uniform mat4 textureMatrix;
    varying vec4 vUv;
    varying vec2 vLocal;

    void main() {
      vLocal = uv;
      vUv = textureMatrix * vec4( position, 1.0 );
      gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
    }`,
  fragmentShader: /* glsl */ `
    uniform vec3 color;
    uniform sampler2D tDiffuse;
    uniform float intensity;
    uniform float blur;
    uniform float reach;
    varying vec4 vUv;
    varying vec2 vLocal;

    void main() {
      vec2 uv = vUv.xy / vUv.w;

      vec3 sum = vec3( 0.0 );
      float total = 0.0;
      for ( int i = -2; i <= 2; i ++ ) {
        for ( int j = -2; j <= 2; j ++ ) {
          vec2 offset = vec2( float( i ), float( j ) ) * blur;
          float weight = max( 1.0 - length( vec2( float( i ), float( j ) ) ) / 3.2, 0.0 );
          sum += texture2D( tDiffuse, uv + offset ).rgb * weight;
          total += weight;
        }
      }
      vec3 reflection = sum / max( total, 0.0001 );

      float distance = length( vLocal - 0.5 ) * 2.0;
      float fade = 1.0 - smoothstep( reach * 0.35, reach, distance );

      gl_FragColor = vec4( reflection * color, intensity * fade );

      #include <tonemapping_fragment>
      #include <colorspace_fragment>
    }`,
};

/** Ce que le shader attend, dans ses unités à lui. */
export interface FloorMirrorSettings {
  /** 0 à 100, tel que réglé par l'utilisateur. */
  reflection: number;
  /** Dixièmes, tels que réglés par l'utilisateur (5 = 0,5). */
  reflectionBlur: number;
  /** 20 à 150, tel que réglé par l'utilisateur. */
  reflectionReach: number;
}

/** Report des réglages sur les uniformes. Une fonction plutôt que trois lignes
 * recopiées : le miroir se règle depuis la construction **et** depuis l'effet
 * qui suit les préférences, et les deux doivent convertir pareil. */
export function applyFloorMirror(
  uniforms: Record<string, { value: unknown }>,
  settings: FloorMirrorSettings,
): void {
  uniforms.intensity.value = settings.reflection / 100;
  // Le pas du curseur est le dixième ; le shader travaille en fraction de la
  // largeur de la cible, d'où la division. Calibré au banc contre la valeur
  // retenue par l'utilisateur (0,5).
  uniforms.blur.value = (settings.reflectionBlur / 10) * 0.002;
  uniforms.reach.value = settings.reflectionReach / 100;
}
