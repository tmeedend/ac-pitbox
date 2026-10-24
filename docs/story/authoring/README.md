# Giving a story its look — the author guide

How to make the phones of a story, dress their apps, and set the places the
story happens in. Everything here works on a copy of the sample story
(Create Your Own › Copy the sample), whose files are shown as examples; open
it beside this guide.

- **[Phones and skins](frames-and-skins.md)** — a story's phones, from the
  most basic to the latest; their apps; the skins that give an app its look;
  what a Beat changes in the phone.
- **[Scenes](scenes.md)** — the places behind the phone and the dialogues,
  their variants by season, time of day, weather and mood, their sounds.

Nothing is required. A story with no picture at all plays: every phone, app
and scene falls back on Pit Box's own look. Add one image, see it, add the
next.

## The starter kit

The **Starter kit** block, at the top of the Frames screen, saves it in a
folder you pick (**Save the starter kit…**) - outside the story's folder,
next to it say: the kit is for drawing, not part of what you share. The
sheets land in a `starter-kit` folder there, one per template, as SVG and
PNG; **Open the folder** shows them. A sheet shows each slot of
that template where it sits, at its reference size, with the stretch margins
we suggest drawn dashed, and what to keep clear hatched. Draw over a sheet in
any tool, cut each slot to its own file, and import them in the Frames
screen.

- Phone sheets are drawn for a screen **1080 px wide** (2340 high); scene
  sheets for a scene **1920 × 1080**.
- **Stretch margins** (9-slice): the four borders, in the image's pixels,
  that stay as drawn while the middle stretches - the round corners of a
  bubble, the ends of a header. A slot with no margins is stretched whole.
- **PNG or WebP** for images, **TTF, OTF or WOFF2** for fonts, **OGG, MP3 or
  WAV** for sounds, **WebM or MP4** for a scene's video loop.

## Files and licences

What a story carries goes with it when it is shared. Every file you add needs
a licence that lets it be redistributed with the story: your own work, CC0,
CC BY (credited), or similar. A font usually comes with its licence file:
keep it beside the font, in the skin's folder.

Each file the player sees or hears - a picture, a video, a sound, a font -
has a line in a `credits.yaml`: `media/credits.yaml` for the files of
`media/`, and one in each skin's folder for the skin's own files, so a skin
copied to another story keeps its credits. A line says which files (by path
inside that folder, `*` for any part of a name), where they come from, who
made them, under which licence, and whether you may carry them: `granted`
(the author said yes), `not_needed` (the licence allows it, or you made it),
`requested` or `not_asked`. The Package section of the editor fills these
lines with you, and builds the package only once every file has one that is
`granted` or `not_needed`. The sample's registers show the form.
