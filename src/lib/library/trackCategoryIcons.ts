// Pictograms of the track categories, embedded in the app: the counterpart of
// the car families' silhouettes (`familyIcons.ts`), on the same 120×52 grid and
// at the same weight, so that the two indexes read as one set.
//
// The user PICKS a category's icon from this set; a category he made up gets
// the circuit until he does, and an unknown icon falls back to the neutral
// glyph — never an empty tile.
//
// Inner markup of a `viewBox="0 0 120 52"` SVG. Three classes, styled by the
// component that draws them, so the strings carry no colour and follow the
// design tokens:
//   - no class: solid, in the icon's colour;
//   - `ln`: a stroke in the icon's colour (roads, kerbs: a ribbon is a line);
//   - `cut`, `cutln`: a hole, filled or stroked in the colour of the tile
//     behind (lane markings, windows, start lines);
//   - `smoke`: a light stroke at half strength (dust, tyre smoke).

/** A ribbon of road: the stroke of the set, about 4.5 px on a 34 px tile. */
const road = (d: string, w = 7) => `<path class="ln" stroke-width="${w}" d="${d}"/>`;
/** Round holes on one row: lights, tyre centres. */
const holes = (xs: number[], y: number, r: number) =>
  xs.map((x) => `<circle class="cut" cx="${x}" cy="${y}" r="${r}"/>`).join("");
/** A marking cut into a road or a shape. */
const mark = (d: string, w = 2, dash = "") =>
  `<path class="cutln" stroke-width="${w}"${dash ? ` stroke-dasharray="${dash}"` : ""} d="${d}"/>`;

const CIRCUIT =
  road("M22 39C11 39 8 27 16 21L39 11C47 7.5 54 12 52 19.5C50 26.5 57 30 65 26.5L91 14C103 8.5 114 16 110 28C107 36 100 40 90 40Z") +
  mark("M64 36v8", 2.5);

export const TRACK_CATEGORY_ICONS: Record<string, string> = {
  circuit: CIRCUIT,

  // A city: blocks behind, the street in front with its centre line.
  street:
    `<path d="M8 38V20h12v-8h12v26h4V6h16v32h4V16h14v22h4V10h12v8h10v20z"/>` +
    `<path class="cut" d="M24 16h4v4h-4zm16-6h4v4h-4zm6 0h4v4h-4zm-6 8h4v4h-4zm6 0h4v4h-4zm16 2h4v4h-4zm16-6h4v4h-4zm6 0h4v4h-4zm-6 8h4v4h-4z"/>` +
    `<path d="M4 41h112v8H4z"/>` +
    mark("M10 45h100", 1.6, "7 5"),

  // A banked oval and its start line.
  oval: road("M60 9C88 9 110 16 110 26S88 43 60 43 10 36 10 26 32 9 60 9Z") + mark("M60 38v10", 2.5),

  // A car seen from above, sliding, its smoke behind it.
  drift:
    `<g transform="rotate(-24 76 24)"><rect x="56" y="14" width="44" height="21" rx="6"/>` +
    `<path class="cut" d="M83 17h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2h-7zm-20 2h6v11h-6z"/></g>` +
    `<circle cx="34" cy="38" r="8" opacity=".45"/><circle cx="20" cy="31" r="6" opacity=".35"/>` +
    `<circle cx="44" cy="46" r="4.5" opacity=".4"/><circle cx="10" cy="40" r="4" opacity=".25"/>` +
    `<path class="smoke" d="M8 48C30 50 48 44 58 34"/>`,

  // A mountain pass: its switchbacks cut into the slope.
  touge:
    `<path d="M3 47 33 15l12 11 20-21 52 42z"/>` +
    mark("M18 47C42 45 58 41 47 37S53 31 64 28 59 19 65 12", 3.2),

  // A stage in the forest: two edges of a gravel road between the pines.
  rally:
    `<path d="M14 46 22 30h-4l7-12h-3l7-12 7 12h-3l7 12h-4l8 16zM92 46l7-14h-3l6-11h-3l6-11 6 11h-3l6 11h-3l7 14z"/>` +
    `<path class="ln" stroke-width="4.5" d="M45 49C56 36 70 28 61 13M77 49C81 36 79 24 67 13"/>` +
    `<path class="smoke" d="M52 49c4-3 9-3 13 0"/>`,

  // A short loop, half asphalt and half gravel.
  rallycross:
    road("M28 37C14 37 13 17 28 17H68C78 17 80 9 91 9 106 9 110 26 99 33") +
    road("M99 33C93 37 86 37 79 37H28").replace('class="ln"', 'class="ln" stroke-linecap="butt" stroke-dasharray="4 3"'),

  // A steep road to a summit flag.
  hillclimb:
    `<path d="M3 48 101 14h15v34z"/>` +
    mark("M14 46 99 17", 2.2) +
    `<path class="ln" stroke-width="2.4" d="M108 14V2"/>` +
    `<path d="M109 2h10v7h-10z"/><path class="cut" d="M109 2h3.3v3.5H109zm6.6 0h3.3v3.5h-3.3zM112.3 5.5h3.3V9h-3.3z"/>`,

  // The christmas tree over the strip.
  drag:
    `<rect x="47" y="3" width="26" height="25" rx="3"/>` +
    [9, 16, 23].map((y) => holes([54, 66], y, 2.6)).join("") +
    `<path d="M58 28h4v9h-4zM4 38h112v11H4z"/>` +
    mark("M8 43.5h104", 1.6, "8 5") +
    mark("M30 38v11", 2.2),

  // A motorway running away under its gantry.
  freeroam:
    `<path d="M4 49 50 15h20l46 34z"/>` +
    mark("M44 49 57 15M76 49 63 15", 1.8, "6 4") +
    `<path d="M14 4h92v8H14z"/><path class="ln" stroke-width="2.4" d="M18 12v15M102 12v15"/>` +
    `<path class="cut" d="M30 6.5h18v3H30zm42 0h18v3H72z"/>`,

  // A tight kart track and a stack of tyres.
  karting:
    road(
      "M24 40C14 40 12 30 20 26L34 20C40 17 40 11 48 11S58 18 64 18 74 11 82 11 96 16 96 24 88 32 80 32 70 36 70 40Z",
      5,
    ) +
    `<circle cx="104" cy="43" r="5.5"/><circle cx="115" cy="43" r="5.5"/><circle cx="109.5" cy="33" r="5.5"/>` +
    holes([104, 115], 43, 2) +
    holes([109.5], 33, 2),

  // From A to B: a winding road between two pins.
  a2b:
    road("M22 40C40 42 42 26 58 26S78 14 96 16", 4.5) +
    `<path d="M22 47C15 39 12 35 12 30a10 10 0 0 1 20 0c0 5-3 9-10 17z"/><circle class="cut" cx="22" cy="30" r="4"/>` +
    `<path d="M98 25C91 17 88 13 88 8a10 10 0 0 1 20 0c0 5-3 9-10 17z"/><circle class="cut" cx="98" cy="8" r="4"/>`,

  // A test area: cones on the tarmac.
  test:
    `<path d="M4 44h112v5H4z"/>` +
    `<path d="M30 42 38 14h4l8 28zM70 42l8-28h4l8 28z"/>` +
    `<path class="cut" d="M35.4 30h9.2l1.1 4H34.3zm40 0h9.2l1.1 4H74.3z"/>` +
    `<path d="M52 42l5-17h2.6l5 17z" opacity=".6"/>`,
};

/** The neutral glyph: `Unclassified`, and any category without a known icon. */
export const NEUTRAL_TRACK_ICON = `<g opacity=".5">${CIRCUIT}</g>`;

/** The icon a category gets when it names none: the circuit. */
export const DEFAULT_TRACK_ICON = "circuit";

export function trackCategoryIcon(icon: string | null | undefined): string {
  return (icon && TRACK_CATEGORY_ICONS[icon]) || NEUTRAL_TRACK_ICON;
}
