<script lang="ts">
  // Groupe de boutons segmenté : un choix parmi trois ou quatre, tous visibles.
  //
  // Sept copies locales avant ce composant (bibliothèque, vues transversales,
  // import groupé, type de session, options de session, visionneuse PDF), le
  // CSS Svelte étant scopé par composant : mêmes bordures, mêmes couleurs,
  // mais six paddings et cinq tailles de police, chacune dérivant de son côté
  // sans que rien ne le signale. Même mécanisme que les trois `.tabs` d'avant
  // `Tabs.svelte`.
  //
  // Trois axes de variation, et trois seulement — chacun porte une raison, pas
  // un goût :
  //
  //  - `vertical` : une colonne d'options qui se parcourt à la manette (les
  //    options de session). Une liste déroulante n'y conviendrait pas, elle
  //    n'est pas pilotable au pad.
  //  - `tone` : `accent` (rouge éteint + filet, niveau 2 du barème) pour un
  //    choix qui porte sur la session ou la catégorie, `neutral` (fond
  //    surélevé) pour une bascule de présentation. Le rouge a un barème
  //    (SPEC §7.2ter) : une bascule d'affichage n'y a pas droit.
  //  - `size` : nommée par son rôle, jamais par une taille. `toolbar` s'aligne
  //    sur les 32 px des autres contrôles de la barre de filtres (§7.1) —
  //    c'est une contrainte mécanique, pas une préférence ; `main` est le
  //    choix principal d'un écran ; `mini` tient dans une ligne de liste
  //    dense. `compact` est le cas normal.
  export interface SegItem {
    value: string;
    /** Libellé, ou glyphe quand `icon` est posé. */
    label: string;
    title?: string;
    disabled?: boolean;
  }

  interface Props {
    items: SegItem[];
    /** Valeur courante : le bouton correspondant est marqué actif. */
    value: string;
    onselect: (value: string) => void;
    vertical?: boolean;
    tone?: "accent" | "neutral";
    size?: "compact" | "toolbar" | "mini" | "main";
    /** Les libellés sont des glyphes : boutons centrés, de largeur égale. */
    icon?: boolean;
    /** Le groupe entier est neutralisé par son contexte (SETUP§2.6) — distinct du
     * `disabled` d'un item, qui écarte un choix parmi d'autres. Un réglage
     * visible mais sans effet doit être éteint, pas seulement silencieux. */
    disabled?: boolean;
  }

  let {
    items,
    value,
    onselect,
    vertical = false,
    tone = "accent",
    size = "compact",
    icon = false,
    disabled = false,
  }: Props = $props();
</script>

<div class="seg {size}" class:vertical class:neutral={tone === "neutral"} class:icon>
  {#each items as item (item.value)}
    <button
      type="button"
      class:on={value === item.value}
      title={item.title}
      disabled={disabled || item.disabled}
      onclick={() => onselect(item.value)}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
  }
  .seg.vertical {
    flex-direction: column;
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    border-right: 1px solid var(--line);
  }
  .seg.vertical button {
    border-right: none;
    border-bottom: 1px solid var(--line);
    /* Une colonne se lit au fer à gauche : centrer des libellés de longueurs
       différentes rendrait la liste illisible. */
    text-align: left;
    color: var(--txt2);
  }
  .seg button:last-child {
    border-right: none;
    border-bottom: none;
  }
  .seg button:hover:not(.on):not(:disabled) {
    color: var(--txt2);
  }
  .seg.vertical button:hover:not(.on):not(:disabled) {
    background: var(--raised);
  }
  .seg button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* --- Marquage de l'actif : deux tons, deux rôles (voir l'en-tête) --- */
  /* Niveau 2 du barème (§7.2ter), pas niveau 1 : un fond rouge plein est
     réservé à « Démarrer la session », **un seul élément par écran**. L'écran
     de session en comptait cinq — quatre segmentés plus le bouton de
     lancement — parce que ce ton était le défaut du composant et que personne
     n'avait eu à l'écrire. Le traitement retenu est celui que le bloc
     Adversaires avait déjà inventé dans son coin pour ses trois modes :
     fond éteint, libellé rouge clair, filet de 2 px. C'est aussi, mot pour
     mot, ce que le barème appelle « ce qui est retenu pour la session ». */
  .seg button.on {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    box-shadow: inset 0 -2px 0 var(--rosso);
  }
  /* Empilés, le filet du bas se lirait comme un séparateur de plus : une
     colonne marque son option retenue sur le bord d'attaque, comme l'entrée
     active du rail. */
  .seg.vertical button.on {
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .seg.neutral button.on {
    background: var(--raised);
    color: var(--txt);
  }

  /* --- Tailles, nommées par leur rôle --- */
  .seg.compact button {
    padding: 6px 14px;
    font-size: 11px;
  }
  .seg.vertical.compact button {
    padding: 7px 10px;
  }
  /* Aligné sur la recherche et le bouton « + Filtre » : la barre de filtres ne
     connaît que deux hauteurs (§7.1), et 32 px est celle-ci. */
  .seg.toolbar button {
    height: 32px;
    padding: 0 11px;
    font-size: 11.5px;
  }
  .seg.mini button {
    padding: 3px 7px;
    font-size: 10px;
  }
  .seg.main button {
    padding: 9px 26px;
    font-size: 12px;
    letter-spacing: 1px;
  }

  .seg.icon button {
    min-width: 26px;
    padding-left: 10px;
    padding-right: 10px;
    text-align: center;
  }
  .seg.toolbar.icon button {
    font-size: 14px;
  }
</style>
