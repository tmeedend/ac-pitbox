"""Draws the sample story's phone media (Story Mode, L3d): two phone bodies
with their screen cut out, a wallpaper, seven app icons, the `chat_blue`
skin's 9-slice bubbles and reply button, and the phone's two beeps. Every
file is CC0, made for Pit Box, and listed in `media/credits.yaml` and the
skin's own `credits.yaml`.

Not run by the build: the files it makes are committed. Run it again only to
change them, from the repository's root:

    python -I scripts/story-sample-media.py src-tauri/crates/story-engine/sample

Needs Pillow (`pip install pillow`) and, for the beeps, ffmpeg on the PATH.
"""
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

root = Path(sys.argv[1])
media = root / "media" / "frames"
skin = root / "frames" / "skins" / "chat_blue"


def save(img, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.suffix == ".webp":
        img.save(path, quality=80, method=6)
    else:
        img.save(path, optimize=True)
    print(path, path.stat().st_size)


# A smartphone: a dark rounded slab, a thin bezel, the screen cut out.
W, H = 1180, 2400
SCREEN = (50, 50, 1080, 2300, 90)  # x, y, width, height, radius
body = Image.new("RGBA", (W, H), (0, 0, 0, 0))
d = ImageDraw.Draw(body)
d.rounded_rectangle((0, 0, W - 1, H - 1), radius=140, fill=(28, 30, 36, 255))
d.rounded_rectangle((6, 6, W - 7, H - 7), radius=134, outline=(70, 74, 84, 255), width=4)
x, y, w, h, r = SCREEN
d.rounded_rectangle((x, y, x + w - 1, y + h - 1), radius=r, fill=(0, 0, 0, 0))
# The camera, in the bezel above the screen's top edge.
d.ellipse((W // 2 - 14, 18, W // 2 + 14, 46), fill=(10, 10, 14, 255))
save(body, media / "smartphone" / "body.png")

# A basic phone: a candy bar, a small screen up top, a keypad below.
W, H = 520, 1100
SMALL = (60, 110, 400, 470, 12)
body = Image.new("RGBA", (W, H), (0, 0, 0, 0))
d = ImageDraw.Draw(body)
d.rounded_rectangle((0, 0, W - 1, H - 1), radius=90, fill=(54, 60, 70, 255))
d.rounded_rectangle((24, 24, W - 25, 620), radius=60, fill=(36, 40, 48, 255))
x, y, w, h, r = SMALL
d.rounded_rectangle((x, y, x + w - 1, y + h - 1), radius=r, fill=(0, 0, 0, 0))
for row in range(4):
    for col in range(3):
        cx, cy = 130 + col * 130, 700 + row * 95
        d.rounded_rectangle((cx - 48, cy - 30, cx + 48, cy + 30), radius=24, fill=(88, 96, 108, 255))
save(body, media / "basic_phone" / "body.png")

# The smartphone's wallpaper: soft circles over a night-blue gradient.
W, H = 1080, 2300
wall = Image.new("RGB", (W, H))
px = wall.load()
for j in range(H):
    t = j / H
    c = (int(18 + 20 * t), int(28 + 30 * t), int(52 + 40 * t))
    for i in range(W):
        px[i, j] = c
glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
g = ImageDraw.Draw(glow)
for cx, cy, rad, col in [(250, 600, 420, (70, 120, 220, 90)), (850, 1500, 520, (190, 80, 150, 70)), (500, 2000, 300, (60, 180, 170, 60))]:
    g.ellipse((cx - rad, cy - rad, cx + rad, cy + rad), fill=col)
glow = glow.filter(ImageFilter.GaussianBlur(120))
wall = Image.alpha_composite(wall.convert("RGBA"), glow).convert("RGB")
save(wall, media / "smartphone" / "wallpaper.webp")


# App icons, 192 x 192: a coloured rounded square and a white glyph.
def icon(name, colour, glyph):
    img = Image.new("RGBA", (192, 192), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((0, 0, 191, 191), radius=44, fill=colour)
    glyph(d)
    save(img, media / "smartphone" / "icons" / f"{name}.png")


white = (255, 255, 255, 255)
icon("messages", (52, 199, 89, 255), lambda d: (d.rounded_rectangle((40, 50, 152, 128), radius=36, fill=white), d.polygon([(64, 120), (56, 152), (92, 124)], fill=white)))
icon("mail", (10, 132, 255, 255), lambda d: (d.rectangle((38, 58, 154, 134), fill=white), d.line([(40, 60), (96, 104), (152, 60)], fill=(10, 132, 255, 255), width=8)))
icon("news", (255, 69, 58, 255), lambda d: [d.rectangle((44, y, 148, y + 14), fill=white) for y in (52, 84, 112, 140)])
icon("social", (175, 82, 222, 255), lambda d: (d.ellipse((70, 44, 122, 96), fill=white), d.pieslice((42, 100, 150, 208), 180, 360, fill=white)))
icon("bank", (255, 159, 10, 255), lambda d: (d.polygon([(96, 40), (156, 80), (36, 80)], fill=white), [d.rectangle((x, 88, x + 16, 136), fill=white) for x in (52, 88, 124)], d.rectangle((36, 142, 156, 154), fill=white)))


def handset(d):
    d.rounded_rectangle((52, 40, 92, 80), radius=14, fill=white)
    d.rounded_rectangle((100, 112, 140, 152), radius=14, fill=white)
    d.line([(68, 72), (66, 104), (84, 130), (112, 128)], fill=white, width=22, joint="curve")


def calendar(d):
    d.rounded_rectangle((40, 46, 152, 154), radius=14, fill=white)
    d.rectangle((40, 46, 152, 78), fill=(255, 59, 48, 255))
    for x in (66, 96, 126):
        for y in (98, 126):
            d.rectangle((x - 9, y - 8, x + 9, y + 8), fill=(200, 200, 205, 255))


icon("calls", (48, 209, 88, 255), handset)
icon("calendar", (242, 242, 247, 255), calendar)


# The chat skin's bubbles: 9-slice, a 40 px margin keeps the corners round.
def bubble(name, colour):
    img = Image.new("RGBA", (120, 120), (0, 0, 0, 0))
    ImageDraw.Draw(img).rounded_rectangle((0, 0, 119, 119), radius=40, fill=colour)
    save(img, skin / f"{name}.png")


bubble("bubble_in", (38, 44, 56, 255))
bubble("bubble_out", (10, 132, 255, 255))
reply = Image.new("RGBA", (120, 120), (0, 0, 0, 0))
ImageDraw.Draw(reply).rounded_rectangle((2, 2, 117, 117), radius=40, outline=(10, 132, 255, 255), width=6)
save(reply, skin / "reply.png")


# The phone's beeps: a short tick when an app opens, two notes when a reply
# goes. Synthesised by ffmpeg, so no recording is involved.
sounds = media / "smartphone" / "sounds"
sounds.mkdir(parents=True, exist_ok=True)
if shutil.which("ffmpeg"):
    base = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y"]
    subprocess.run(
        base + ["-f", "lavfi", "-i", "sine=frequency=1400:duration=0.05",
                "-af", "afade=t=out:st=0.02:d=0.03,volume=0.5", "-c:a", "libvorbis", "-q:a", "2",
                str(sounds / "tap.ogg")],
        check=True,
    )
    subprocess.run(
        base + ["-f", "lavfi", "-i", "sine=frequency=880:duration=0.12",
                "-f", "lavfi", "-i", "sine=frequency=1320:duration=0.12",
                "-filter_complex", "[0][1]concat=n=2:v=0:a=1,afade=t=out:st=0.18:d=0.06,volume=0.5",
                "-c:a", "libvorbis", "-q:a", "2", str(sounds / "send.ogg")],
        check=True,
    )
else:
    print("ffmpeg not found: the beeps are left as they are")
