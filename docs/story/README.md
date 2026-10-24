# Story Mode & Create Your Own

- **`SPEC-story-mode.md`** — the engine and editor spec, generic: what Pit Box
  must do for **any** story. Written in English.
- **`authoring/`** — the author guide: giving a story its look (phones, apps,
  skins, scenes), step by step on a copy of the sample.

**What is not here, on purpose.** This repository is public, and a story stays
private until its release. So nothing that belongs to a story itself enters
it: no characters, plot, dialogue, events, places, sponsors, and no settings an
author picked for a given story. That covers the full design document and the
screen mockups, which are filled with a story's content.

They stay on the machine, in `docs/story/private/`, which `.gitignore`
excludes: the complete design spec and one HTML mockup per screen of the
editor and the player view. The mockups use the design canvas runtime and image
links that only resolve there: read them as markup. They bring the **UX**,
never the UI (`CLAUDE.md`, « Le design system fait foi, pas la maquette »).

**When the design document changes**, the public spec is updated by hand from
it, and every new passage goes through the same sorting: what the engine must
do comes in, what one story chose stays out. Examples stay neutral (« a
character », « Race 01 »), never taken from a story in progress.
