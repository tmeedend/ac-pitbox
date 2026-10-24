# Scenes

A scene is a **place** - a bedroom, a garage - shown behind the phone and
behind the dialogues. The sample has two, in `scenes/scenes.yaml`: the
bedroom where the story starts, and the garage, with its variants and its
sound.

## 1. A place and its variants

In the Scenes screen, type a name under **Places**. A new place starts sharp
behind the phone (its blur box unticked) with one variant to fill.

A place has **variants**: the same place by season, time of day, weather or
mood. Each variant has its picture (an image, 1920 × 1080, or a seamless
video loop) and its criteria: the seasons and times of day ticked, the
weathers and moods typed. A criterion left empty holds always.

**The first variant that holds shows**, in the order of the list, and the
last one shows when none does - so the general case goes last. The sample's
garage: `tense` (a mood a Beat sets), then `night` (evening and night), then
`day`, the default.

Beside the sheet, the preview shows the place as the player sees it: set the
season, the time of day, the weather and the mood, and see which variant
shows, with the phone over it or not.

## 2. Blur and sound

- **Blurred behind the frame** - a place can be blurred while the phone is
  over it; a variant can say otherwise. The sample's places are sharp; its
  `tense` garage is blurred, the mind elsewhere.
- **Ambient sound** - a loop played while the place shows, faded in and out
  when it changes; a variant can have its own. Listen with ▶. The sample's
  garage plays `media/scenes/garage.mp3`. The game's Sound button mutes it.

## 3. Where the story is

- **The start** (Settings) names the first place.
- **A Beat** sets the place from there, and can force one of its variants
  until a Beat sets a place again. It also sets the **weather** and the
  **mood** the variants ask for; they stay until a Beat changes them, and
  `none` clears them.
- **A dialogue** can play in a place of its own (its sheet's Place): the
  current place comes back after it.

The sample: the bedroom at first, the garage after the first visit (chapter
1), chapter 3's dialogue in the garage at night, a `tense` mood, the garage
by day a week later, the bedroom when the phone is back.

A change of place fades, or cuts when the dialogue that brings it - or ends
it - says `transition: cut`.
