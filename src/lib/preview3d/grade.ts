// The colour grade of the game's previews — saturation down and a cool cast,
// the same for every car (docs/SPEC-grille.md GRILLE§5.6). The thumbnails apply
// it to their finished image (`gridthumbs/look.ts`); the 3D preview, which has
// no post-processing pass, at the end of each material's shader.
import type * as ThreeModule from "three";

/** The game's own cast at 100 % cool tint, per channel: its whites are bluish
 * (an Abarth 500 is (151, 157, 163) in its preview). */
export const GRADE_TINT = [-0.03, 0.02, 0.055] as const;

/** One set of uniforms for every graded material: the setting is the
 * preview's, not a material's, and moving its slider must not recompile. */
const uniforms = {
  pbSaturation: { value: 1 },
  pbTint: { value: [1, 1, 1] as number[] },
};

/** Sets the grade, in the settings' percent. */
export function setPreviewGrade(saturation: number, coolTint: number): void {
  const tint = coolTint / 100;
  uniforms.pbSaturation.value = saturation / 100;
  uniforms.pbTint.value = GRADE_TINT.map((c) => 1 + c * tint);
}

/**
 * Grades the material's output, once encoded — where the thumbnails grade
 * theirs, on the encoded image. At 100 % saturation and no tint, it changes
 * nothing.
 */
export function patchGradeShader(shader: ThreeModule.WebGLProgramParametersWithUniforms): void {
  shader.uniforms.pbSaturation = uniforms.pbSaturation;
  shader.uniforms.pbTint = uniforms.pbTint;
  shader.fragmentShader = shader.fragmentShader
    .replace(
      "#include <common>",
      `#include <common>
      uniform float pbSaturation;
      uniform vec3 pbTint;`,
    )
    .replace(
      "#include <colorspace_fragment>",
      `#include <colorspace_fragment>
      {
        vec3 pbGraded = gl_FragColor.rgb * pbTint;
        float pbLuma = dot( pbGraded, vec3( 0.2126, 0.7152, 0.0722 ) );
        gl_FragColor.rgb = clamp( pbLuma + ( pbGraded - pbLuma ) * pbSaturation, 0.0, 1.0 );
      }`,
    );
}
