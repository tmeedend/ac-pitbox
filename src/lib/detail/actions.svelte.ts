// The gestures of a car or track sheet (§6.3): the ⋮ menu — activate,
// showroom, export, reinstall, delete — and the version history's activate
// and delete (§10), with the pack's uninstall (§4.4). Each one writes to disk
// through the backend, then reloads the sheet and tells the library.
//
// Out of `DetailPage.svelte`, which keeps the data and what is shown: these
// change with the mod's lifecycle (showcase, versions, archives kept), that
// with the layout of the sheet. The page hands over what it owns through
// getters (`ActionsHost`), the way `ServerFilter` reads the Online page.
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { t } from "$lib/i18n/index.svelte";
import { errorText } from "$lib/errors";
import { activateMod, deactivateMod, openModFolder, type ModDetail } from "$lib/library/library";
import { deleteMods } from "$lib/library/showcase.svelte";
import { openNativeShowroom } from "$lib/launch/launch";
import { editBrand } from "$lib/workshop/brandFocus.svelte";
import { showInGameFolder } from "$lib/gamestate/gameFolder.svelte";
import {
  deleteModVersion,
  deletePack,
  exportMod,
  profilesUsingVersion,
  reinstallFromArchive,
  type ExportReport,
} from "$lib/workshop/maintenance";

/** One entry of the sheet's ⋮ menu, as `FicheHeader` renders it. */
export interface MenuAction {
  label: string;
  onclick: () => void;
  disabled?: boolean;
  danger?: boolean;
}

/** What the actions read from the sheet. Getters: every one of them changes
 * under it. */
export interface ActionsHost {
  readonly id: string;
  readonly detail: ModDetail | null;
  readonly isCar: boolean;
  /** The other mods of the same pack, counted in the uninstall confirmation. */
  readonly packSiblings: number;
  /** The livery to open the showroom with, once the sheet's skins are known:
   * asked before they are, SKIN= leaves empty and the car shows all white. */
  showroomSkin(): Promise<string | null>;
  /** Reads the mod again (activation state, versions). */
  reload(): Promise<void>;
  /** Reads the mod and what hangs on it again (skins, layouts, sounds). */
  refresh(): Promise<void>;
  /** The library changed: its list and the session column follow. */
  changed(): void;
  close(): void;
  /** The sheet's error line; `""` clears it. */
  setError(message: string): void;
}

export class DetailActions {
  #host: ActionsHost;

  /** Activation and versions: one at a time, the history's buttons too. */
  busy = $state(false);
  deleteBusy = $state(false);
  reinstallBusy = $state(false);
  packBusy = $state(false);
  exporting = $state(false);
  /** The native 3D preview (acShowroom.exe) is starting. */
  showroomBusy = $state(false);
  reinstallOk = $state(false);
  exportResult = $state<ExportReport | null>(null);
  /** What became of a deleted version (§10) — recycle bin or permanent
   * deletion. A notice, not an error. */
  versionNotice = $state("");

  constructor(host: ActionsHost) {
    this.#host = host;
  }

  #fail(e: unknown) {
    this.#host.setError(errorText(e));
  }

  async activate(versionId?: string) {
    const d = this.#host.detail;
    if (!d || this.busy) return;
    this.busy = true;
    this.#host.setError("");
    try {
      await activateMod(d.id_interne, versionId);
      await this.#host.reload();
      this.#host.changed();
    } catch (e) {
      this.#fail(e);
    } finally {
      this.busy = false;
    }
  }

  async deactivate() {
    const d = this.#host.detail;
    if (!d || this.busy) return;
    this.busy = true;
    this.#host.setError("");
    try {
      await deactivateMod(d.id_interne);
      await this.#host.reload();
      this.#host.changed();
    } catch (e) {
      this.#fail(e);
    } finally {
      this.busy = false;
    }
  }

  /** Deletes a stored version (§10). Two warnings come BEFORE it is
   * irreversible: the profiles that pinned it (they fall back to the version
   * in place), and the recycle bin that may refuse a large version, in which
   * case the deletion is permanent. What really happened comes back in the
   * result, and is shown. */
  async deleteVersion(versionId: string) {
    const d = this.#host.detail;
    if (!d || this.busy) return;
    const v = d.versions.find((ver) => ver.id === versionId);
    const label = v?.version_label ?? t("detail.noVersionNumber");
    let pinned: string[] = [];
    try {
      pinned = await profilesUsingVersion(versionId);
    } catch (e) {
      this.#fail(e);
      return;
    }
    const message = [
      t("detail.deleteVersionConfirm", { label }),
      pinned.length ? t("detail.deleteVersionProfiles", { profiles: pinned.join(", ") }) : "",
    ]
      .filter(Boolean)
      .join("\n\n");
    const ok = await confirm(message, { title: t("detail.deleteVersion"), kind: "warning" });
    if (!ok) return;
    this.busy = true;
    this.#host.setError("");
    this.versionNotice = "";
    try {
      const outcome = await deleteModVersion(versionId);
      this.versionNotice = outcome.recycled
        ? t("detail.deleteVersionRecycled", { label })
        : t("detail.deleteVersionPurged", { label });
      await this.#host.reload();
      this.#host.changed();
    } catch (e) {
      this.#fail(e);
    } finally {
      this.busy = false;
    }
  }

  /** Delete (ESPACE§5.1): the confirmation offers the showcase, by default,
   * or the complete deletion. The sheet stays open on a mod put in the
   * showcase — it is still there, and its banner says what became of it —
   * and closes on one deleted. */
  async delete() {
    const d = this.#host.detail;
    if (!d || this.deleteBusy) return;
    this.deleteBusy = true;
    this.#host.setError("");
    try {
      const done = await deleteMods([d]);
      if (done) this.#host.changed();
      if (done === "complete") this.#host.close();
      else if (done === "showcase") await this.#host.refresh();
    } catch (e) {
      this.#fail(e);
    } finally {
      this.deleteBusy = false;
    }
  }

  async reinstall() {
    const d = this.#host.detail;
    if (!d || this.reinstallBusy) return;
    const ok = await confirm(t("detail.reinstallConfirm", { name: d.display_name ?? d.id_interne }), {
      title: t("detail.reinstallConfirmTitle"),
      kind: "warning",
    });
    if (!ok) return;
    this.reinstallBusy = true;
    this.#host.setError("");
    this.reinstallOk = false;
    try {
      await reinstallFromArchive(d.id_interne);
      await this.#host.reload();
      this.#host.changed();
      this.reinstallOk = true;
    } catch (e) {
      this.#fail(e);
    } finally {
      this.reinstallBusy = false;
    }
  }

  /** Uninstalls the whole pack the mod came with (§4.4). The sheet closes:
   * the mod it shows is gone with the rest. */
  async uninstallPack() {
    const pack = this.#host.detail?.source_pack;
    if (!pack || this.packBusy) return;
    const ok = await confirm(t("detail.uninstallConfirm", { pack, count: this.#host.packSiblings + 1 }), {
      title: t("detail.uninstallTitle"),
      kind: "warning",
    });
    if (!ok) return;
    this.packBusy = true;
    this.#host.setError("");
    try {
      await deletePack(pack);
      this.#host.changed();
      this.#host.close();
    } catch (e) {
      this.#fail(e);
      this.packBusy = false;
    }
  }

  async export() {
    const d = this.#host.detail;
    if (!d || this.exporting) return;
    const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
    if (!dir || typeof dir !== "string") return;
    this.exporting = true;
    this.#host.setError("");
    this.exportResult = null;
    try {
      this.exportResult = await exportMod(d.id_interne, dir);
    } catch (e) {
      this.#fail(e);
    } finally {
      this.exporting = false;
    }
  }

  /** The native 3D preview (acShowroom.exe), started as an **independent
   * process** over the app, with the game's video settings; the user closes
   * it to come back. Embedding its window in the page was tried and dropped
   * (see showroom.rs). A showroom already open keeps the livery it started
   * with — there is no known way to change it live — so the next one takes
   * the new livery. */
  async openShowroom() {
    const d = this.#host.detail;
    if (!d || this.showroomBusy) return;
    this.showroomBusy = true;
    this.#host.setError("");
    try {
      const skin = await this.#host.showroomSkin();
      await openNativeShowroom(d.id_interne, skin);
    } catch (e) {
      this.#fail(e);
    } finally {
      this.showroomBusy = false;
    }
  }

  /** The mod's real folder in Windows Explorer — also for Kunos content,
   * read-only. */
  async openFolder() {
    const d = this.#host.detail;
    if (!d) return;
    try {
      await openModFolder(d.id_interne);
    } catch (e) {
      this.#fail(e);
    }
  }

  /** The game folder pruned to what this mod lays (DOSSIER§3.2). */
  showInGameFolder() {
    const id = this.#host.id;
    void showInGameFolder({ kind: this.#host.isCar ? "car" : "track", id }, this.#host.detail?.display_name ?? id);
  }

  /** The ⋮ menu (§6.3): `FicheHeader` renders and places it, only the list
   * is made here. The favourite heart and the state badge are not in it: they
   * are read all the time, they are not actions. */
  menu(): MenuAction[] {
    const d = this.#host.detail;
    if (!d) return [];
    const isCar = this.#host.isCar;
    const items: MenuAction[] = [];
    // In the showcase (ESPACE§6): no activation, no showroom, no export —
    // the banner's "Recover the files" replaces them, and the only deletion
    // left is the complete one.
    if (!d.is_stock && !d.showcase) {
      items.push({
        label: d.active ? t("common.deactivate") : t("common.activate"),
        onclick: d.active ? () => void this.deactivate() : () => void this.activate(),
        disabled: this.busy,
      });
    }
    if (isCar && !d.showcase) {
      items.push({
        label: this.showroomBusy ? t("detail.showroomLaunching") : t("detail.showroom"),
        onclick: () => void this.openShowroom(),
        disabled: this.showroomBusy,
      });
    }
    if (isCar && d.brand) {
      const brand = d.brand;
      items.push({ label: t("detail.editBrand", { brand }), onclick: () => void editBrand(brand) });
    }
    items.push({ label: t("detail.openFolder"), onclick: () => void this.openFolder() });
    items.push({ label: t("detail.showInGameFolder"), onclick: () => this.showInGameFolder() });
    if (!d.is_stock && !d.showcase) {
      items.push({
        label: this.exporting ? t("detail.exporting") : t("detail.export"),
        onclick: () => void this.export(),
        disabled: this.exporting,
      });
    }
    if (!d.is_stock) {
      if (keptArchive(d) && !d.showcase) {
        items.push({
          label: this.reinstallBusy ? t("detail.reinstalling") : t("detail.reinstallFromArchive"),
          onclick: () => void this.reinstall(),
          disabled: this.reinstallBusy,
        });
      }
      items.push({
        label: this.deleteBusy
          ? t("common.working")
          : d.showcase
            ? t("showcase.deleteCompletelyMenu")
            : t("detail.deleteFromLibrary"),
        onclick: () => void this.delete(),
        disabled: this.deleteBusy,
        danger: true,
      });
    }
    return items;
  }
}

/** The archive or folder kept for the active version (§10/§11), if any —
 * what the "Reinstall" entry needs. */
function keptArchive(d: ModDetail): string | null {
  return d.versions.find((v) => v.id === d.active_version_id)?.kept_archive_path ?? null;
}
