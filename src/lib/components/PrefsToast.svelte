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
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { prefsWriteFailure } from "$lib/uiPrefs.svelte";

  let dismissed = $state(false);
  const failure = $derived(prefsWriteFailure());
  const visible = $derived(failure.since !== null && !dismissed);
</script>

{#if visible}
  <Toast tone="warn" icon="⚠" title={t("prefs.writeFailedTitle")} onclose={() => (dismissed = true)}>
    <p class="body">{t("prefs.writeFailedBody")}</p>
    <p class="why mono">{failure.reason}</p>
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
</style>
