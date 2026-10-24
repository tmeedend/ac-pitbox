// The starter kit (SPEC-story-mode, Skins: how a frame gets its look): the
// template sheets the engine draws (`story_engine::kit`), saved in a folder
// the author picks, in SVG as drawn and in PNG. The PNG is made here: the
// page has a browser to rasterise an SVG with, the backend has none.
import { invoke } from "@tauri-apps/api/core";

/** Every sheet, `[file name, SVG]`. */
const kitSheets = () => invoke<[string, string][]>("story_kit_sheets");

/** `svg` drawn at its own size, as PNG bytes. */
async function toPng(svg: string): Promise<number[]> {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    canvas.getContext("2d")!.drawImage(img, 0, 0);
    const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, "image/png"));
    if (!blob) throw new Error("the sheet did not draw");
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Saves the kit in `dir`; resolves to how many files were written. A sheet
 * that does not draw as PNG is still saved in SVG. */
export async function saveKit(dir: string): Promise<number> {
  const pngs: { name: string; png: number[] }[] = [];
  for (const [name, svg] of await kitSheets()) {
    try {
      pngs.push({ name, png: await toPng(svg) });
    } catch (e) {
      console.warn("story: a kit sheet did not draw as PNG", name, e);
    }
  }
  return invoke<number>("story_save_kit", { dir, pngs });
}
