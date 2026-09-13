<script lang="ts">
  // Naming a saved session (§8.4bis). The frame, the list and the overwrite
  // confirmation live in `NamedListDialog`, shared with saved grids (§5): the
  // gesture was written twice before, and the user recognised it from one
  // screen to the other.
  //
  // What stays here is what is proper to sessions: they are listed BY TYPE, and
  // the meta line is the date the snapshot was taken.
  import { onMount } from "svelte";
  import type { SessionType } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import { listSavedSessions, deleteSavedSession, formatSavedAt, type SavedSession } from "$lib/savedSessions";
  import NamedListDialog from "./NamedListDialog.svelte";

  interface Props {
    sessionType: SessionType;
    onsave: (name: string) => void;
    onclose: () => void;
  }
  let { sessionType, onsave, onclose }: Props = $props();

  // Loaded once on opening (the component is recreated each time, see the
  // caller) then refreshed by hand after a deletion.
  let sessions = $state<SavedSession[]>([]);

  onMount(() => {
    listSavedSessions(sessionType).then((list) => (sessions = list));
  });

  const entries = $derived(sessions.map((s) => ({ name: s.name, meta: formatSavedAt(s.savedAt) })));

  async function remove(name: string) {
    await deleteSavedSession(sessionType, name);
    sessions = await listSavedSessions(sessionType);
  }
</script>

<NamedListDialog
  mode="save"
  title={t("launch.saveSessionTitle")}
  placeholder={t("launch.sessionNamePlaceholder")}
  emptyText={t("launch.noSavedSessions")}
  {entries}
  {onsave}
  ondelete={(name) => void remove(name)}
  {onclose}
/>
