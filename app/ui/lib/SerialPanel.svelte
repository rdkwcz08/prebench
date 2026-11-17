<!--
  app/ui/lib/SerialPanel.svelte — Onglet Série (maquette : sortie en chasse fixe, horodatage
  en texte secondaire, champ d'envoi avec fin de ligne et « Envoyer »).
  Les outils (port, débit, horodatage, hexa, export, effacer) sont dans la barre
  d'onglets (SerialTools.svelte). Alerte de débit : ligne d'avertissement en tête.
  Liste virtualisée (section 11.3) : lignes de 20 px, seules les lignes visibles
  (et une marge) sont dans le DOM ; les nouvelles lignes arrivent au plus une
  fois par image d'affichage. Une ligne plus large que le panneau défile
  horizontalement, sans retour à la ligne (hauteur fixe).
  Suivi de la fin (cas courant pendant une simulation) : seules les dernières
  lignes sont dessinées, sans zone défilante de la hauteur de l'historique
  (WKWebView garde cette zone composée : ~130 Mo de plus). Remonter (molette,
  Haut, Page précédente, Début) passe à la liste complète, défilante ;
  revenir en bas reprend le suivi.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { onNextFrame } from './frame';
  import { bench } from './bench.svelte';
  import { decode, LINE_ENDINGS, serial, stamp, toHex, type Chunk, type LineEnding } from './serial.svelte';
  import Button from './ui/Button.svelte';
  import Icon from './ui/Icon.svelte';

  /** Lignes gardées pour l'affichage (le tampon, lui, garde 1 Mo). */
  const MAX_SHOWN = 5000;
  /** Hauteur d'une ligne (charte : lignes de 20 px). */
  const LINE = 20;
  /** Lignes rendues en plus de part et d'autre de la zone visible. */
  const OVERSCAN = 10;
  /** Marge haute du journal (padding). */
  const PAD = 6;

  let log: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewHeight = $state(200);
  let text = $state('');
  /** Suit la fin tant que l'utilisateur n'est pas remonté. */
  let follow = $state(true);

  const port = $derived(serial.active);
  const warning = $derived(serial.mismatch(port));
  /** Alimentation coupée : aucun firmware pour recevoir. */
  const off = $derived(bench.status.state === 'off');

  type Line = { id: number; t: number; text: string; sys?: boolean };

  /**
   * Lignes construites au fil de l'eau (charge du rendu, section 11.1) : seuls
   * les nouveaux morceaux sont décodés, et chaque ligne garde son identifiant,
   * donc le DOM n'ajoute que les nouvelles lignes au lieu de tout redessiner.
   * Reconstruites seulement au changement de port, de vue (hexa) ou après
   * « Effacer ».
   */
  let cache = {
    key: '',
    hex: false,
    epoch: -1,
    done: 0, // index absolu du prochain morceau à traiter
    nextId: 0,
    lines: [] as Line[],
    open: false, // la dernière ligne n'est pas terminée
  };

  function append(c: Chunk): void {
    const lines = cache.lines;
    const push = (l: Omit<Line, 'id'>): void => {
      lines.push({ id: cache.nextId++, ...l });
    };
    if (serial.hex || c.sys) {
      cache.open = false;
      push({ t: c.t, text: c.sys ? decode(c.data) : toHex(c.data), sys: c.sys });
      return;
    }
    const parts = decode(c.data).replace(/\r\n?/g, '\n').split('\n');
    parts.forEach((part, i) => {
      const last = lines[lines.length - 1];
      if (cache.open && last) {
        // Ligne en cours : remplacée (nouvel objet) pour que la vue la redessine.
        lines[lines.length - 1] = { ...last, text: last.text + part };
      } else if (part !== '' || i < parts.length - 1) {
        push({ t: c.t, text: part });
        cache.open = true;
      }
      if (i < parts.length - 1) cache.open = false; // fin de ligne : la suivante sera neuve
    });
    if (lines.length > MAX_SHOWN * 2) lines.splice(0, lines.length - MAX_SHOWN);
  }

  /**
   * Nombre de morceaux pris en compte, avancé au plus une fois par image
   * d'affichage : plusieurs trames série dans la même image ne coûtent qu'une
   * mise à jour.
   */
  let seen = $state(0);
  let scheduled = false;
  $effect(() => {
    const n = port?.chunks.length ?? 0;
    if (n === seen || scheduled) return;
    scheduled = true;
    // Image commune (frame.ts) : les octets série ne font pas dessiner une image de plus.
    onNextFrame(() => {
      scheduled = false;
      seen = serial.active?.chunks.length ?? 0;
    });
  });

  /** Lignes à afficher : horodatage de leur premier octet, texte ou hexadécimal. */
  const lines = $derived.by(() => {
    if (!port) return [];
    const count = Math.min(seen, port.chunks.length); // nouveaux morceaux, une fois par image
    if (cache.key !== port.key || cache.hex !== serial.hex || cache.epoch !== port.epoch || cache.done < port.dropped) {
      cache = { key: port.key, hex: serial.hex, epoch: port.epoch, done: port.dropped, nextId: 0, lines: [], open: false };
    }
    for (let i = cache.done - port.dropped; i < count; i++) {
      const c = port.chunks[i];
      if (c) append(c);
    }
    cache.done = port.dropped + count;
    if (cache.lines.length > MAX_SHOWN) cache.lines.splice(0, cache.lines.length - MAX_SHOWN);
    // Nouveau tableau seulement pour signaler le changement : les lignes ne sont pas copiées une à une.
    return cache.lines.slice();
  });

  /** Fenêtre rendue : lignes visibles et marge. */
  const first = $derived(Math.max(0, Math.floor((scrollTop - PAD) / LINE) - OVERSCAN));
  const visible = $derived(lines.slice(first, first + Math.ceil(viewHeight / LINE) + 2 * OVERSCAN));

  /** Suivi : les seules lignes qui tiennent dans le panneau, la dernière en bas. */
  const tail = $derived(lines.slice(-(Math.ceil(viewHeight / LINE) + 1)));

  function onscroll(): void {
    if (!log || follow) return;
    scrollTop = log.scrollTop;
    viewHeight = log.clientHeight;
    if (log.scrollTop + log.clientHeight >= log.scrollHeight - 8) follow = true; // revenu en bas
  }

  /** Quitte le suivi : liste complète, positionnée en bas puis remontée de `up` pixels. */
  async function leaveFollow(up: number): Promise<void> {
    if (!follow || lines.length * LINE <= viewHeight) return;
    follow = false;
    scrollTop = Math.max(0, lines.length * LINE + 2 * PAD - viewHeight - up);
    await tick();
    log?.scrollTo({ top: scrollTop });
  }

  function onwheel(e: WheelEvent): void {
    if (follow && e.deltaY < 0) void leaveFollow(-e.deltaY);
  }

  function onkeydown(e: KeyboardEvent): void {
    const up: Record<string, number> = { ArrowUp: LINE, PageUp: viewHeight, Home: Infinity };
    const by = up[e.key];
    if (by === undefined) return;
    e.stopPropagation(); // les flèches ne déplacent pas la paillasse
    if (follow) {
      e.preventDefault();
      void leaveFollow(by);
    }
  }

  $effect(() => {
    if (!log) return;
    const el = log;
    // Hauteur lue à l'image suivante : la changer dans le rappel relancerait
    // la mise en page dans la même image (boucle ResizeObserver).
    const observer = new ResizeObserver(() => requestAnimationFrame(() => (viewHeight = el.clientHeight)));
    observer.observe(el);
    return () => observer.disconnect();
  });

  async function submit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    await serial.send(text);
    text = '';
  }
</script>

<div class="serial">
  {#if warning}
    <p class="warning" role="alert"><Icon name="warn" />{warning}</p>
  {/if}
  <!-- Pas de région annoncée (aria-live) : un lecteur d'écran lirait chaque
       octet reçu, et le navigateur recalculerait l'accessibilité à chaque ligne. -->
  <!-- Zone défilante nommée, focalisable : se parcourt au clavier comme à la souris. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="log data"
    class:follow
    bind:this={log}
    {onscroll}
    {onwheel}
    {onkeydown}
    tabindex="0"
    role="region"
    aria-label="Sortie série"
  >
    {#if !port}
      <p class="empty">Aucune sortie série. Démarrez la simulation : les octets émis par les UART du firmware et sa sortie standard (printf) s'affichent ici.</p>
    {:else if lines.length === 0}
      <p class="empty">{port.label} : rien reçu pour l'instant.</p>
    {/if}
    {#if lines.length > 0 && follow}
      <div class="tail">
        {#each tail as l (l.id)}
          <div class="line" class:sys={l.sys}>{#if serial.timestamps}<span class="t">{stamp(l.t)}</span>{/if}{l.text}</div>
        {/each}
      </div>
    {:else if lines.length > 0}
      <!-- Espaceur en haut, dans le flux : les lignes visibles gardent leur
           place dans le document, seule la nouvelle ligne est à peindre. -->
      <div class="rows" style:height="{lines.length * LINE}px" style:padding-top="{first * LINE}px">
        {#each visible as l (l.id)}
          <div class="line" class:sys={l.sys}>{#if serial.timestamps}<span class="t">{stamp(l.t)}</span>{/if}{l.text}</div>
        {/each}
      </div>
    {/if}
  </div>
  {#if port && port.id !== null}
    <form class="send" onsubmit={submit}>
      <input
        class="data"
        bind:value={text}
        placeholder={off ? 'Alimentation coupée : rien à recevoir' : `Texte à envoyer sur ${port.label}`}
        disabled={off}
        aria-label="Texte à envoyer"
        spellcheck="false"
        onkeydown={(e) => e.stopPropagation()}
      />
      <select
        class="ending"
        title="Fin de ligne"
        aria-label="Fin de ligne"
        value={serial.ending}
        onchange={(e) => (serial.ending = e.currentTarget.value as LineEnding)}
      >
        {#each Object.entries(LINE_ENDINGS) as [k, v] (k)}
          <option value={k}>{v.label}</option>
        {/each}
      </select>
      <Button type="submit" disabled={off}>Envoyer</Button>
    </form>
  {/if}
</div>

<style>
  .serial {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    height: 100%;
    margin: -6px -12px; /* occupe tout le panneau du bas */
  }
  .warning {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 0;
    padding: 4px 12px;
    border-bottom: 1px solid var(--pb-line);
    color: var(--pb-text);
    font-family: var(--pb-font-ui);
    font-size: 12px;
    line-height: 20px;
  }
  .warning :global(.icon) {
    margin-top: 2px;
    color: var(--pb-warn);
    flex: none;
  }
  .log {
    overflow: auto;
    padding: 6px 12px;
    font-size: 12px;
    line-height: 20px;
  }
  .rows {
    box-sizing: border-box;
    min-width: max-content;
  }
  /* Suivi de la fin : pas de défilement vertical, la dernière ligne en bas. */
  .log.follow {
    overflow-y: hidden;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }
  .tail {
    min-width: max-content;
  }
  .log:focus-visible {
    outline: 2px solid var(--pb-accent);
    outline-offset: -2px;
  }
  .line {
    height: 20px;
    white-space: pre;
  }
  .t {
    margin-right: 16px;
    color: var(--pb-text-quiet);
  }
  /* Repère de Prebench (maquette : .log .sys) : texte secondaire, police de l'interface. */
  .sys {
    color: var(--pb-text-quiet);
    font-family: var(--pb-font-ui);
  }
  .empty {
    margin: 0;
    color: var(--pb-text-quiet);
    font-family: var(--pb-font-ui);
  }
  .send {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 6px 12px;
    border-top: 1px solid var(--pb-line);
  }
  .send input {
    flex: 1;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    color: var(--pb-text);
    font-size: 12px;
    user-select: text;
  }
  .send input:disabled {
    opacity: 0.6;
  }
  .send input:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
    border-color: transparent;
  }
  .ending {
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: transparent;
    color: var(--pb-text);
    font-size: 12px;
  }
</style>
