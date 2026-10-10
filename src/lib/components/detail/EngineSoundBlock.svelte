<script lang="ts">
  // "Engine sound" card of a car's detail page (§8.3): the sounds that can
  // replace the car's own, each with a key to listen to it, and the rev slider
  // while the game's own engine plays. The page owns the list and the two
  // gestures — it is also the one that reloads the list when the library
  // changes, and that stops the engine when the car changes.
  import type { SubModRow } from "$lib/inventory/submods";
  import IgnitionKey from "./IgnitionKey.svelte";
  import EngineRevControls from "./EngineRevControls.svelte";
  import { engineState } from "$lib/detail/enginePlayer.svelte";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    modId: string;
    sounds: SubModRow[];
    /** A sound is being deployed: the radio buttons are disarmed meanwhile. */
    busy: boolean;
    /** Deploy a sound, or restore the original with `null`. */
    onpick: (subId: string | null) => void;
    /** Listen to a sound without deploying anything. */
    onlisten: (subId: string | null) => void;
    /** In the showcase (ESPACE§6): no bank to play, nothing to switch. */
    freed?: boolean;
  }
  let { modId, sounds, busy, onpick, onlisten, freed = false }: Props = $props();

  const activeSound = $derived(sounds.find((s) => s.is_active) ?? null);
</script>

<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("detail.engineSound")}</span></header>
  <div class="blk-b">
    {#if freed}
      <p class="freed">{t("showcase.noSound")}</p>
    {:else}
    <div class="sounds">
      <!-- Deux boutons par ligne, et c'est délibéré : le premier **active**
           le son (il remplace les fichiers du jeu), la clé ne fait
           qu'écouter. Un bouton imbriqué dans un autre serait invalide, et
           surtout les deux gestes ne doivent pas se confondre. -->
      <div class="sound-row">
        <button class="sound" class:sel={!activeSound} type="button" onclick={() => onpick(null)} disabled={busy}>
          <span class="radio"></span>
          <span class="s-name">{t("detail.soundOrigin")}</span>
          <span class="s-tag mono">{t("library.baseBadge")}</span>
        </button>
        <IgnitionKey state={engineState(modId, null)} onclick={() => onlisten(null)} />
      </div>
      {#each sounds as snd (snd.id)}
        <div class="sound-row">
          <button class="sound" class:sel={snd.is_active} type="button" onclick={() => onpick(snd.id)} disabled={busy}>
            <span class="radio"></span>
            <span class="s-name">{snd.name}</span>
            <span class="s-tag mono">{t("detail.modTag")}</span>
          </button>
          <IgnitionKey state={engineState(modId, snd.id)} onclick={() => onlisten(snd.id)} />
        </div>
      {/each}
    </div>
    <EngineRevControls />
    <!-- L'exclusivité et l'absence de mod se lisent sur les boutons radio
         eux-mêmes : « Origine » seule et cochée dit tout. -->
    {/if}
  </div>
</section>

<style>
  .freed {
    color: var(--muted);
    font-size: 11.5px;
  }
  .sounds {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* The rev controls sit under the list of sounds. */
  .blk-b :global(.rev) {
    margin-top: 10px;
  }

  .sound-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sound-row .sound {
    flex: 1;
    min-width: 0;
  }
  .sound {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
    padding: 7px 10px;
    text-align: left;
  }
  .sound.sel {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .sound:disabled {
    opacity: 0.6;
  }
  .radio {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 1px solid var(--muted2);
    flex: none;
  }
  .sound.sel .radio {
    border-color: var(--rosso-bright);
    background: radial-gradient(var(--rosso-bright) 40%, transparent 45%);
  }
  .s-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .s-tag {
    font-size: 7px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
</style>
