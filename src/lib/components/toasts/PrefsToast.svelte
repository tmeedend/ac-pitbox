<script lang="ts">
  // « Vos réglages ne sont pas enregistrés », dans la pile de notifications.
  //
  // **Pourquoi une notification et pas un simple log.** Une écriture de
  // `ui_prefs.json` qui échoue ne se voit nulle part : le réglage reste en
  // mémoire, l'écran affiche ce qu'on vient de choisir, et la perte ne se
  // découvre qu'au redémarrage suivant — quand plus personne ne peut la
  // relier au geste qui l'a causée. Bug réel, resté invisible une journée
  // entière : corps et tenues de pilote adoptés, rien sur le disque.
  //
  // Elle ne se referme pas toute seule (comme la notification de nouveau
  // périphérique) : ce n'est pas une information de passage, c'est une perte
  // de données en cours.
  //
  // Two files can fail this way: `ui_prefs.json` and `launch_state.json`, the
  // session screen's settings. One notification for both — the same loss, the
  // same title — with a sentence and a reason for each file that fails.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { prefsWriteFailure } from "$lib/uiPrefs.svelte";
  import { launchStateWriteFailure } from "$lib/launch/launchState.svelte";

  let dismissed = $state(false);
  const prefs = $derived(prefsWriteFailure());
  const session = $derived(launchStateWriteFailure());
  const visible = $derived((prefs.since !== null || session.since !== null) && !dismissed);
</script>

{#if visible}
  <Toast tone="warn" icon="⚠" title={t("prefs.writeFailedTitle")} onclose={() => (dismissed = true)}>
    {#if prefs.since !== null}
      <p class="body">{t("prefs.writeFailedBody")}</p>
      <p class="why mono">{prefs.reason}</p>
    {/if}
    {#if session.since !== null}
      <p class="body">{t("prefs.writeFailedSessionBody")}</p>
      <p class="why mono">{session.reason}</p>
    {/if}
  </Toast>
{/if}

<style>
  .body {
    font-size: 12px;
    line-height: 1.5;
    color: var(--txt2);
  }
  /* La raison technique, en petit : elle ne s'adresse pas à l'utilisateur mais
     à qui lira son rapport de bug. */
  .why {
    margin-top: 6px;
    font-size: 10.5px;
    color: var(--muted2);
    overflow-wrap: anywhere;
  }
  /* Both files failing: a gap between the two. */
  .why + .body {
    margin-top: 10px;
  }
</style>
