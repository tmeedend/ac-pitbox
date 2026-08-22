// Preview of an ancillary file from the mod's resources folder (§4.5.2).
//
// Only formats that are genuinely readable inline are handled: anything else
// keeps the historical behaviour (open with the OS default application). The
// classification is by extension alone — the resources folder holds whatever
// the mod author shipped, there is no manifest to consult.

/** How a resource can be shown inside the detail page. `null` = not previewable. */
export type PreviewKind = "text" | "markdown" | "image" | "pdf";

// `jsgme` : descripteur de variante JSGME, du texte brut malgré son extension
// exotique — première ligne le nom de l'option, le reste son explication.
// C'est souvent la seule chose qui dise à quoi sert un dossier optionnel.
const TEXT_EXTS = ["txt", "nfo", "log", "ini", "cfg", "csv", "json", "yml", "yaml", "lua", "jsgme"];
const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "avif"];

/** Lowercase extension of a relative path, without the dot. */
function extOf(relPath: string): string {
  const name = relPath.slice(relPath.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function previewKind(relPath: string): PreviewKind | null {
  const ext = extOf(relPath);
  if (ext === "md" || ext === "markdown") return "markdown";
  if (ext === "pdf") return "pdf";
  if (TEXT_EXTS.includes(ext)) return "text";
  if (IMAGE_EXTS.includes(ext)) return "image";
  return null;
}

/**
 * Decodes the bytes of a text resource.
 *
 * Mod readmes are very often **not** UTF-8: they come from Windows text
 * editors and are encoded in the legacy ANSI code page, where a French accent
 * or a degree sign is a single byte that UTF-8 rejects. Decoding them as UTF-8
 * with replacement would silently pepper the text with U+FFFD, so the strict
 * decoder is tried first and windows-1252 is the fallback — it never fails,
 * every byte maps to something. A UTF-8 BOM, when present, is stripped: it
 * would otherwise show up as a stray character on the first line.
 */
export function decodeText(bytes: ArrayBuffer): string {
  const view = new Uint8Array(bytes);
  const body = view[0] === 0xef && view[1] === 0xbb && view[2] === 0xbf ? view.subarray(3) : view;
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(body);
  } catch {
    return new TextDecoder("windows-1252").decode(body);
  }
}
