# Phones and skins

A story's **frame** is the device the character carries: their phone. A story
can have several - a basic phone at first, a smartphone later - and a Beat
hands over the next one. What the character reads, watches or answers in
person (a newspaper, a letter, a TV report, a landline call) is not a frame:
it plays on the scene (see [Scenes](scenes.md)).

The sample has two phones, in `frames/frames.yaml`: `basic_phone`, with
Messages only, and `smartphone`, with seven apps. Chapter 2's first Beat
hands the smartphone over (`flow/02-second-race.yaml`).

## 1. A phone

In the Frames screen, type a name under **Phones**. The new phone starts
with Pit Box's look and one Messages app. Then, in its sheet:

- **Picture of the phone** - an image of the device with its screen cut out
  (transparent). Import it, then give the **screen in the picture**: its left
  and top edges, width and height, in the image's pixels, and the radius of
  its corners. The preview shows it at once; the checks say when the screen
  goes past the picture. The sample's `media/frames/smartphone/body.png` has
  its screen at x 50, y 50, 1080 × 2300, corners 90.
- **Wallpaper** - behind the home screen. Make it the shape of the screen,
  at least as wide (the checks say otherwise).
- **Status bar** - full (clock, date, signal, Wi-Fi, battery), the clock
  only (an old phone: the sample's basic phone), or none.
- **Sounds** - one when an app opens, one when a reply goes. Listen to them
  with ▶ beside each. The game's Sound button mutes them.

Files you import for a phone go to `media/frames/<phone>/`.

## 2. Its apps

An app is a **template** - how its content is laid out - showing some
**content types**. The templates: thread (messages), inbox (mail), feed
(articles, social posts), statement (bank lines), agenda (calendar entries)
and call screen (calls). A type goes to **one** app of a phone; one that no
app shows waits until a phone that has one is up.

For each app: its name, its **icon** (square, 192 px or more), its template,
the types it shows, **when it appears** - always, with its first item (the
default), or from a Beat that unlocks it (at the latest with its first item)
- and its skin. Order them with ↑ ↓: that is the home screen's order. The
first messaging app is where the phone opens.

The sample's smartphone shows every case: Messages always there, News
unlocked by chapter 2's Beat before any article, the others with their first
item.

## 3. A skin

A skin is an app's look: colours, a font, and images on the template's
**slots**. It is a folder of its own, `frames/skins/<skin>/`, holding
`skin.yaml` and its files: copy the folder to another story and the skin
comes along.

Type a name under **Skins**, then pick the template it dresses (a skin for a
thread fits a thread, nothing else). In its sheet:

- **Colours** - background, text, quiet text, accent (your bubbles, the
  buttons). One left unset keeps Pit Box's.
- **Font** - for the app's text. Text always stays text over the skin,
  never drawn into an image: it changes with the story and is translated.
- **Slots** - one image per slot, with its **stretch margins**. The starter
  kit's sheet for the template shows each slot and the margins we suggest.

The sample's `frames/skins/chat_blue/` dresses the smartphone's Messages:
blue bubbles stretched by their 40 px corners, a reply button, Inter.
`frames/skins/chat_night/` is colours only - a skin can be that small.

Then pick the skin in the app's line of the phone's sheet. Skins can also
dress what plays on the scene (a dialogue, a newspaper, a letter, a TV
report, a landline call): pick the skin in that content item's sheet.

## 4. What a Beat changes in the phone

In a Beat's panel:

- **Frame** - the phone from this Beat on (or none: a visual novel).
- **Apps unlocked** - the apps that appear from here, by id.
- **Status bar** - battery, signal, Wi-Fi, airplane mode, and a date label
  instead of the story's date. What you leave alone stays as it was.
- **Skins from this beat** - another skin on an app, in whichever phone has
  it.

The sample's chapter 3 brings the smartphone back with the battery at 9 %,
no Wi-Fi, and Messages in `chat_night` (`flow/03-face-to-face.yaml`).
