// Silhouettes of the category families (TAXO§3.2), embedded in the app.
//
// The user PICKS a family's icon from this set; he never supplies one. A family
// whose icon is unknown - one he made up, or a name mistyped in the rules file
// - gets the neutral glyph, never an empty tile.
//
// Inner markup of a `viewBox="0 0 120 52"` SVG. The body takes the fill of the
// `<svg>`; the parts marked `cut` (windows, tyres) are holes painted in the
// colour of the tile behind, which is what lets one path read as a car at
// 34 px. The component gives `cut` its colour: these strings carry none, so
// the silhouettes follow the design tokens instead of freezing two greys.

const wheels = (x1: number, x2: number, r: number) =>
  `<circle class="cut" cx="${x1}" cy="40" r="${r}"/><circle cx="${x1}" cy="40" r="${r - 3}"/>` +
  `<circle class="cut" cx="${x2}" cy="40" r="${r}"/><circle cx="${x2}" cy="40" r="${r - 3}"/>`;

const STREET_BODY =
  "M10 40c-2 0-3-1-3-3l1-6c.4-3 3-5 6-6l7-1 8-7c2-2 5-3 8-3h22c3 0 6 1 8 3l8 7 10 1c4 .6 7 3 7 6l1 6c0 2-1 3-3 3z";
const STREET_GLASS = "M40 19h-8l-7 6h15zm5 0h16l6 6H45z";

export const FAMILY_ICONS: Record<string, string> = {
  street: `<path d="${STREET_BODY}"/><path class="cut" d="${STREET_GLASS}"/>${wheels(32, 88, 9)}`,
  sports:
    `<path d="M8 41c-2 0-3-1-3-3l.5-5c.5-3 3-5 7-6l10-2 11-7c3-2 6-3 10-3h17c4 0 8 1 11 4l9 7 12 2c5 1 8 3 8 6l.5 4c0 2-1 3-3 3z"/>` +
    `<path class="cut" d="M42 21h-7l-8 5h15zm5 0h14l7 5H47z"/>${wheels(30, 90, 9)}`,
  classic:
    `<path d="M12 41c-3 0-5-2-5-5v-4c0-4 3-7 8-8l6-1 6-9c2-3 5-4 9-4h26c4 0 7 1 9 4l6 9 6 1c5 1 8 4 8 8v4c0 3-2 5-5 5z"/>` +
    `<path class="cut" d="M42 15h-8l-5 8h13zm5 0h13l5 8H47z"/>${wheels(32, 88, 10)}`,
  race:
    `<path d="M6 41c-2 0-3-1-3-3l.5-5c.4-3 3-5 6-6l9-2 10-7c3-2 6-3 9-3h20c4 0 7 1 10 3l9 7 11 2c4 .7 7 3 7 6l.5 5c0 2-1 3-3 3z"/>` +
    `<path class="cut" d="M42 20h-7l-7 5h14zm5 0h14l7 5H47z"/><path d="M96 13h20v5h-20zm16 5h5v10h-5z"/>${wheels(30, 88, 9)}`,
  proto:
    `<path d="M4 41c-2 0-3-1-3-3v-4c0-3 2-5 5-6l16-3 12-6c3-1 6-2 9-2h24c5 0 9 2 12 5l9 7 22 4c4 .7 7 3 7 5v3z"/>` +
    `<path class="cut" d="M48 20h-9l-8 4h17zm5 0h12l8 4H53z"/><path d="M100 14h19v4h-19z"/>${wheels(28, 92, 8)}`,
  openwheel:
    `<path d="M50 34h26v6H50zm-8-8h40c3 0 5 2 5 5v3H42z"/><path d="M44 25c0-4 3-7 7-7s7 3 7 7z"/>` +
    `<path d="M6 34h22v6H6zm4-6h14v6H10z"/><path d="M96 20h22v4H96zm0 14h22v4H96z"/>` +
    `<circle class="cut" cx="26" cy="40" r="10"/><circle cx="26" cy="40" r="7"/>` +
    `<circle class="cut" cx="92" cy="40" r="10"/><circle cx="92" cy="40" r="7"/>`,
  drift:
    `<path d="M6 41c-2 0-3-1-3-3l.5-5c.4-3 3-5 7-6l9-2 11-7c3-2 6-3 9-3h19c4 0 7 1 10 3l9 7 11 2c4 .7 7 3 7 6l.5 5c0 2-1 3-3 3z"/>` +
    `<path class="cut" d="M42 20h-7l-8 5h15zm5 0h13l7 5H47z"/><path d="M92 12h22v5H92zm8 5h6v9h-6z"/>` +
    `<path class="smoke" d="M6 46c8-2 16-2 24 0M8 50c8-2 14-2 20 0"/>${wheels(30, 88, 9)}`,
  rally:
    `<path d="M10 40c-2 0-3-1-3-3l.5-6c.4-3 3-5 6-6l8-1 9-8c2-2 5-3 8-3h20c3 0 6 1 8 3l8 8 10 1c4 .6 7 3 7 6l.5 6c0 2-1 3-3 3z"/>` +
    `<path class="cut" d="M42 17h-8l-7 7h15zm5 0h14l7 7H47z"/><path d="M78 11h16v5H78z"/>` +
    `<rect x="18" y="44" width="10" height="7" rx="1"/><rect x="88" y="44" width="10" height="7" rx="1"/>${wheels(32, 86, 10)}`,
};

/** The neutral glyph: `Unclassified`, and any family without a known icon. */
export const NEUTRAL_ICON = `<path d="${STREET_BODY}" opacity=".5"/><path class="cut" d="${STREET_GLASS}"/>${wheels(32, 88, 9)}`;

export function familyIcon(icon: string | null | undefined): string {
  return (icon && FAMILY_ICONS[icon]) || NEUTRAL_ICON;
}
