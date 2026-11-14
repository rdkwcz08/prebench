<!--
  app/ui/lib/SerialPanel.svelte — Onglet Série (maquette : sortie en chasse fixe, horodatage
  en texte secondaire, champ d'envoi avec fin de ligne et « Envoyer »).
  Les outils (port, débit, horodatage, hexa, export, effacer) sont dans la barre
  d'onglets (SerialTools.svelte). Alerte de débit : ligne d'avertissement en tête.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { bench } from './bench.svelte';
  import { decode, LINE_ENDINGS, serial, stamp, toHex, type Chunk, type LineEnding } from './serial.svelte';
  import Button from './ui/Button.svelte';
  import Icon from './ui/Icon.svelte';

  /** Lignes affichées au plus (le tampon, lui, garde 1 Mo). */
  const MAX_SHOWN = 2000;

  let log: HTMLDivElement | undefined = $state();
  let text = $state('');
  let stick = true; // suit la fin tant que l'utilisateur n'est pas remonté

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

  /** Lignes à afficher : horodatage de leur premier octet, texte ou hexadécimal. */
  const lines = $derived.by(() => {
    if (!port) return [];
    const count = port.chunks.length; // dépendance : nouveaux morceaux
    if (cache.key !== port.key || cache.hex !== serial.hex || cache.epoch !== port.epoch || cache.done < port.dropped) {
      cache = { key: port.key, hex: serial.hex, epoch: port.epoch, done: port.dropped, nextId: 0, lines: [], open: false };
    }
    for (let i = cache.done - port.dropped; i < count; i++) {
      const c = port.chunks[i];
      if (c) append(c);
    }
    cache.done = port.dropped + count;
    return cache.lines.slice(-MAX_SHOWN);
  });

  $effect(() => {
    void lines.length;
    if (stick) void tick().then(() => log?.scrollTo({ top: log.scrollHeight }));
  });

  function onscroll(): void {
    if (log) stick = log.scrollTop + log.clientHeight >= log.scrollHeight - 8;
  }

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
  <div class="log data" bind:this={log} {onscroll} aria-label="Sortie série">
    {#if !port}
      <p class="empty">Aucune sortie série. Démarrez la simulation : les octets émis par les UART du firmware et sa sortie standard (printf) s'affichent ici.</p>
    {:else if lines.length === 0}
      <p class="empty">{port.label} : rien reçu pour l'instant.</p>
    {/if}
    {#each lines as l (l.id)}
      <div class="line" class:sys={l.sys}>{#if serial.timestamps}<span class="t">{stamp(l.t)}</span>{/if}{l.text}</div>
    {/each}
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
  .line {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
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
