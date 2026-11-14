<!--
  app/ui/lib/SerialPanel.svelte — Onglet Série (maquette : sortie en chasse fixe, horodatage
  en texte secondaire, champ d'envoi avec fin de ligne et « Envoyer »).
  Les outils (port, débit, horodatage, hexa, export, effacer) sont dans la barre
  d'onglets (SerialTools.svelte). Alerte de débit : ligne d'avertissement en tête.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { bench } from './bench.svelte';
  import { decode, LINE_ENDINGS, serial, stamp, toHex, type LineEnding } from './serial.svelte';
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

  /** Lignes à afficher : horodatage de leur premier octet, texte ou hexadécimal. */
  const lines = $derived.by(() => {
    if (!port) return [];
    const out: { t: number; text: string; sys?: boolean }[] = [];
    if (serial.hex) {
      for (const c of port.chunks) out.push({ t: c.t, text: c.sys ? decode(c.data) : toHex(c.data), sys: c.sys });
    } else {
      let current: { t: number; text: string } | null = null;
      for (const c of port.chunks) {
        if (c.sys) {
          // Repère sur sa propre ligne : la ligne en cours est close.
          if (current !== null && (current as { text: string }).text !== '') out.push(current);
          current = null;
          out.push({ t: c.t, text: decode(c.data), sys: true });
          continue;
        }
        const parts = decode(c.data).replace(/\r\n?/g, '\n').split('\n');
        parts.forEach((part, i) => {
          if (current === null) current = { t: c.t, text: '' };
          current.text += part;
          if (i < parts.length - 1) {
            out.push(current);
            current = null;
          }
        });
      }
      if (current !== null && (current as { text: string }).text !== '') out.push(current);
    }
    return out.slice(-MAX_SHOWN);
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
  <div class="log data" bind:this={log} {onscroll} aria-live="polite" aria-label="Sortie série">
    {#if !port}
      <p class="empty">Aucune sortie série. Démarrez la simulation : les octets émis par les UART du firmware et sa sortie standard (printf) s'affichent ici.</p>
    {:else if lines.length === 0}
      <p class="empty">{port.label} : rien reçu pour l'instant.</p>
    {/if}
    {#each lines as l, i (i)}
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
