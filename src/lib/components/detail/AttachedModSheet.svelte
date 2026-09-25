<script lang="ts">
  // Fiche d'un mod greffé, ouverte par-dessus celle de l'hôte — comme celle
  // d'une couche, et pour la même raison : on y est arrivé DEPUIS ce mod (§4.3).
  //
  // The gestures are those of the inventory, followed by a reload of both the
  // row shown here and the host's list of attached mods: without it, the
  // screen lies until the next visit.
  import OtherModDetail from "$lib/components/inventory/OtherModDetail.svelte";
  import { listAttached, type InventoryRow } from "$lib/inventory/inventory";
  import { listOtherMods, activateOther, deactivateOther, openOtherModFolder, type OtherModRow } from "$lib/inventory/others";
  import { setEntityDisplayName, setEntityNote } from "$lib/detail/userMeta";
  import { errorText } from "$lib/errors";

  interface Props {
    /** The open row; set back to `null` to close the sheet, which is also what
     * happens when the mod is gone after a reload. */
    row: OtherModRow | null;
    /** The host mod, whose attached list is reloaded after each gesture. */
    hostId: string;
    onattached: (rows: InventoryRow[]) => void;
    onerror: (message: string) => void;
  }
  let { row = $bindable(), hostId, onattached, onerror }: Props = $props();

  async function refresh() {
    try {
      const all = await listOtherMods();
      if (row) row = all.find((o) => o.id === row!.id) ?? null;
      onattached(await listAttached(hostId));
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function toggle() {
    const current = row;
    if (!current) return;
    try {
      if (current.is_active) await deactivateOther(current.id);
      else await activateOther(current.id);
      await refresh();
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function rename(value: string | null) {
    if (!row) return;
    try {
      await setEntityDisplayName("OTHER", row.id, value ?? "");
      await refresh();
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function note(value: string | null) {
    if (!row) return;
    try {
      await setEntityNote("OTHER", row.id, value ?? "");
      await refresh();
    } catch (e) {
      onerror(errorText(e));
    }
  }
</script>

{#if row}
  <OtherModDetail
    {row}
    busy={false}
    warnings={[]}
    onclose={() => (row = null)}
    ontoggle={() => void toggle()}
    ontogglePriority={() => {}}
    onopenFolder={() => void openOtherModFolder(row!.id)}
    ondelete={() => (row = null)}
    onrename={(v) => void rename(v)}
    onnote={(v) => void note(v)}
  />
{/if}
