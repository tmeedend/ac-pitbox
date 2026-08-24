<script lang="ts">
  // Bouton d'écoute du son moteur : une clé de contact qui se tourne.
  //
  // **Pourquoi une clé et pas un triangle de lecture.** Une flèche dit « lire un
  // fichier » ; l'action ici est « démarrer *cette* voiture », et la clé le dit
  // sans légende. Elle a surtout un état d'arrêt que le triangle n'a pas : elle
  // revient en position, comme un vrai contact — là où un triangle doit se
  // changer en carré pour dire la même chose.
  //
  // Le bouton est **distinct de la ligne** qui le porte, et ce n'est pas un
  // détail de mise en page : cliquer la ligne **active** le mod, c'est-à-dire
  // remplace pour de vrai les fichiers `sfx/` du jeu. Si écouter et installer
  // partageaient le même clic, on installerait des mods en croyant les
  // auditionner.
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** `off` au repos, `loading` pendant la lecture du bank, `on` quand ça tourne. */
    state: "off" | "loading" | "on";
    onclick: () => void;
  }

  const { state, onclick }: Props = $props();

  const label = $derived(state === "on" ? t("detail.soundStop") : t("detail.soundListen"));
</script>

<button
  class="key"
  class:on={state === "on"}
  class:loading={state === "loading"}
  type="button"
  title={label}
  aria-label={label}
  aria-pressed={state === "on"}
  {onclick}
>
  <!-- Panneton en bas, tête en haut : l'axe de rotation passe par la tête, donc
       c'est le panneton qui balaie, comme sur un vrai barillet. -->
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="7" r="4" />
    <path d="M12 11 v9" />
    <path d="M12 16 h3" />
    <path d="M12 19 h2.5" />
  </svg>
</button>

<style>
  .key {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex: none;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 50%;
    color: var(--txt3);
    cursor: pointer;
    transition:
      color 0.18s ease,
      border-color 0.18s ease,
      background 0.18s ease;
  }
  .key:hover,
  .key:focus-visible {
    color: var(--txt);
    border-color: var(--line);
    background: var(--panel);
  }
  .key.on {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }

  svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    /* La rotation part de la tête de la clé, pas du centre de la boîte. */
    transform-origin: 50% 29%;
    transition: transform 0.32s cubic-bezier(0.34, 1.2, 0.64, 1);
  }
  .key.on svg {
    transform: rotate(38deg);
  }
  /* Pendant la lecture du fichier, la clé hésite : un aller-retour court, qui
     dit « ça vient » sans promettre que c'est parti. */
  .key.loading svg {
    animation: ignition-wait 0.9s ease-in-out infinite;
  }
  @keyframes ignition-wait {
    0%,
    100% {
      transform: rotate(0deg);
    }
    50% {
      transform: rotate(14deg);
    }
  }

  /* Une préférence système « moins d'animations » retire le mouvement, pas
     l'information : l'état reste lisible à la couleur et au cadre. */
  @media (prefers-reduced-motion: reduce) {
    svg,
    .key.on svg {
      transition: none;
      transform: none;
    }
    .key.loading svg {
      animation: none;
    }
  }
</style>
