// How the phone looks, handed from the screen that knows (the game, the
// Content screen's preview) to every `PhoneFrame` under it, through the
// app views that never need to know: the frame's look, the open app's skin,
// and the story's folder for their files. Plus the two things a look needs
// beyond CSS: a skin's font, loaded once, and the phone's sounds.
import { getContext, setContext } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import type { PhoneLook, SkinShown, Source } from "../play";
import { fontFamily } from "./skins";
import { mediaUrl } from "./speakers";

export interface PhoneDress {
  look: PhoneLook | null;
  /** The skin of the app shown; `null` on the home screen or undressed. */
  skin: SkinShown | null;
  /** The story's folder; `null` for the sample. */
  folder: string | null;
  /** The home screen: the wallpaper shows. */
  home: boolean;
}

const KEY = Symbol("phone-dress");

/** Hands `get` - read anew each time, so it follows the screen's state -
 * to the phones under the calling component. */
export function providePhoneDress(get: () => PhoneDress): void {
  setContext(KEY, get);
}

/** The dress the screen above hands down, if any. */
export function phoneDress(): (() => PhoneDress) | undefined {
  return getContext(KEY);
}

const fonts = new Map<string, Promise<string | null>>();

/** Loads a skin's font (`path` inside the story) once per file, and
 * resolves to the family it is registered under; `null` when it does not
 * load - the skin then keeps Pit Box's font. Read through a command, not a
 * URL: a font from another origin needs CORS headers (`story/media.rs`). */
export function skinFont(folder: string | null, path: string): Promise<string | null> {
  const key = `${folder ?? ""}|${path}`;
  let loading = fonts.get(key);
  if (!loading) {
    const source: Source = folder ? { kind: "folder", path: folder } : { kind: "sample" };
    const family = fontFamily(key);
    loading = invoke<ArrayBuffer>("story_skin_font", { source, path })
      .then((bytes) => new FontFace(family, bytes).load())
      .then((face) => {
        document.fonts.add(face);
        return family;
      })
      .catch((e) => {
        console.warn("story: skin font not loaded", path, e);
        return null;
      });
    fonts.set(key, loading);
  }
  return loading;
}

/** Plays one of the phone's sounds (`path` inside `media/`), unless muted. */
export function playPhoneSound(folder: string | null, path: string | undefined, muted: boolean): void {
  const src = mediaUrl(folder, path);
  if (!src || muted) return;
  // A refused start (no sound device) leaves the phone silent: nothing the
  // player can act on.
  new Audio(src).play().catch(() => {});
}
