<script lang="ts" module>
  /** Un critère retenu : la valeur, et le sens qu'on lui donne. */
  export interface Token {
    value: string;
    mode: "inc" | "exc";
  }
</script>

<script lang="ts">
  // Filtre à jetons inclure/exclure (§6.3) : les tags et les catégories de la
  // bibliothèque.
  //
  // Il remplace un champ texte à virgules et un `<select>` mono-valué, qui
  // avaient la même limite : on ne pouvait dire que « ceux-ci », jamais « tous
  // sauf ceux-là », ni mélanger les deux. Chaque valeur retenue devient un
  // jeton, vert si on la veut, rouge si on la refuse — un clic bascule.
  //
  // Le tableau est possédé par l'appelant (`$bindable`) : c'est lui qui filtre,
  // persiste et réinitialise. Ici, rien que la saisie.
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** Valeurs proposées à l'autocomplétion (déjà triées par l'appelant). */
    options: string[];
    tokens: Token[];
    placeholder?: string;
    /** Combien de mods portent cette valeur — affiché à droite de chaque ligne
     * du menu. Sans lui, choisir entre deux tags voisins se fait à l'aveugle. */
    countOf?: (value: string) => number;
    /** Largeur minimale du champ ; il s'étire jusqu'à `maxWidth` quand les
     * jetons s'accumulent, puis les fait passer à la ligne. */
    minWidth?: number;
    maxWidth?: number;
  }
  let {
    options,
    tokens = $bindable(),
    placeholder = "",
    countOf,
    minWidth = 220,
    maxWidth = 340,
  }: Props = $props();

  let text = $state("");
  let open = $state(false);
  let active = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  /** Un `-` en tête bascule la saisie en mode exclusion (accélérateur clavier,
   * annoncé dans l'infobulle du champ — pas dans une légende permanente). */
  const excMode = $derived(text.trimStart().startsWith("-"));
  const needle = $derived(text.trim().replace(/^-/, "").trim().toLowerCase());

  const suggestions = $derived(
    options.filter((o) => !tokens.some((tk) => tk.value === o) && o.toLowerCase().includes(needle)),
  );

  // L'index actif ne doit jamais dépasser une liste qui vient de rétrécir sous
  // la frappe : sans ça, Entrée valide `undefined` et ne fait rien.
  const activeIndex = $derived(Math.min(active, Math.max(suggestions.length - 1, 0)));

  function add(value: string, mode: "inc" | "exc") {
    tokens = [...tokens, { value, mode }];
    text = "";
    active = 0;
    inputEl?.focus();
  }

  function toggle(value: string) {
    tokens = tokens.map((tk) => (tk.value === value ? { ...tk, mode: tk.mode === "inc" ? "exc" : "inc" } : tk));
  }

  function remove(value: string) {
    tokens = tokens.filter((tk) => tk.value !== value);
    inputEl?.focus();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = Math.min(activeIndex + 1, suggestions.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(activeIndex - 1, 0);
    } else if (e.key === "Enter") {
      const pick = suggestions[activeIndex];
      if (!pick) return;
      e.preventDefault();
      add(pick, e.altKey || excMode ? "exc" : "inc");
    } else if (e.key === "Backspace" && text === "" && tokens.length) {
      // Retirer le dernier jeton à la touche retour, comme un champ de
      // destinataires d'e-mail : le geste est acquis, il n'a pas à s'expliquer.
      remove(tokens[tokens.length - 1].value);
    } else if (e.key === "Escape") {
      open = false;
      inputEl?.blur();
    }
  }
</script>

<div class="tf" style="min-width:{minWidth}px;max-width:{maxWidth}px">
  <!-- Cliquer n'importe où dans le cadre donne le focus à la saisie : le champ
       se comporte comme un `<input>`, jetons compris. -->
  <div
    class="field"
    role="presentation"
    onmousedown={(e) => {
      if (e.target === e.currentTarget) {
        e.preventDefault();
        inputEl?.focus();
      }
    }}
  >
    {#each tokens as tk (tk.value)}
      <span class="chip {tk.mode}">
        <button type="button" class="body" title={t("filters.tokenToggle")} onclick={() => toggle(tk.value)}>
          <span class="state" aria-hidden="true">{tk.mode === "inc" ? "✓" : "⊘"}</span>
          <span class="label">{tk.value}</span>
        </button>
        <button type="button" class="del" title={t("filters.tokenRemove")} onclick={() => remove(tk.value)}>×</button>
      </span>
    {/each}
    <input
      bind:this={inputEl}
      bind:value={text}
      class="entry"
      type="text"
      autocomplete="off"
      spellcheck="false"
      {placeholder}
      title={t("filters.tokenInputHint")}
      onfocus={() => (open = true)}
      onblur={() => setTimeout(() => (open = false), 120)}
      oninput={() => (active = 0)}
      onkeydown={onKeydown}
    />
  </div>

  {#if open}
    <div class="menu">
      {#if suggestions.length === 0}
        <div class="empty">{t("filters.tokenNoMatch")}</div>
      {:else}
        {#each suggestions as o, i (o)}
          <!-- `mousedown` et non `click` : le `blur` de la saisie ferme le menu
               avant qu'un `click` n'ait le temps de partir. -->
          <div
            class="row"
            class:on={i === activeIndex}
            role="presentation"
            onmousedown={(e) => {
              e.preventDefault();
              add(o, e.altKey || excMode ? "exc" : "inc");
            }}
          >
            <span class="name">{o}</span>
            {#if countOf}<span class="count mono">{countOf(o)}</span>{/if}
            <button
              type="button"
              class="quick inc"
              title={t("filters.tokenInclude")}
              onmousedown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                add(o, "inc");
              }}>✓</button
            >
            <button
              type="button"
              class="quick exc"
              title={t("filters.tokenExclude")}
              onmousedown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                add(o, "exc");
              }}>⊘</button
            >
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .tf {
    position: relative;
    flex: 1 1 auto;
  }
  .field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    background: var(--panel);
    border: 1px solid var(--line);
    padding: 4px 5px;
    min-height: 30px;
    cursor: text;
  }
  .field:focus-within {
    border-color: var(--rosso-border);
  }
  .entry {
    flex: 1 1 70px;
    min-width: 70px;
    background: none;
    border: 0;
    outline: 0;
    color: var(--txt);
    font: inherit;
    font-size: 12px;
    padding: 2px;
  }
  .entry::placeholder {
    color: var(--faint);
  }

  /* --- jetons --- */
  .chip {
    display: inline-flex;
    align-items: center;
    border: 1px solid;
    font-size: 11px;
    overflow: hidden;
    white-space: nowrap;
  }
  .chip .body,
  .chip .del {
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .chip .body {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px;
  }
  .chip .state {
    font-size: 10px;
  }
  .chip .del {
    padding: 2px 6px;
    border-left: 1px solid;
    border-color: inherit;
    opacity: 0.6;
    line-height: 1;
  }
  .chip .del:hover {
    opacity: 1;
  }
  /* Mêmes teintes que `TriCheck` : vert = gardé, rouge = écarté. Le texte barré
     double la couleur, pour qui ne distingue pas les deux. */
  .chip.inc {
    color: var(--green);
    border-color: var(--green-border);
    background: var(--green-dim);
  }
  .chip.exc {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .chip.exc .label {
    text-decoration: line-through;
  }

  /* --- autocomplétion --- */
  .menu {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 4px);
    z-index: 30;
    background: var(--panel2);
    border: 1px solid var(--line);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.5);
    max-height: 260px;
    overflow: auto;
    padding: 3px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px;
    cursor: pointer;
  }
  .row.on,
  .row:hover {
    background: var(--raised);
  }
  .name {
    flex: 1;
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--muted2);
    font-size: 10.5px;
  }
  .quick {
    width: 21px;
    height: 21px;
    flex: none;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .quick.inc:hover {
    color: var(--green);
    border-color: var(--green-border);
  }
  .quick.exc:hover {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .empty {
    padding: 9px 8px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
