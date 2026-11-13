<!--
  app/ui/lib/SerialTools.svelte — Outils des onglets Série et Traceur, dans la barre d'onglets
  (maquette) : port (UART du firmware ou sortie standard), débit du moniteur ;
  pour Série : horodatage et hexadécimal (boutons bascules), export, effacer.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import { BAUDS, DEFAULT_BAUD, serial } from './serial.svelte';
  import IconButton from './ui/IconButton.svelte';

  let { full = true }: { full?: boolean } = $props();

  const port = $derived(serial.active);

  async function exportPort(): Promise<void> {
    if (!port) return;
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, '0');
    const name = `serie-${port.key === 'stdout' ? 'printf' : port.label}-${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.txt`;
    await bench.exportSerial(name, serial.exportText(port));
  }
</script>

{#if serial.ports.length > 0}
  <select
    class="port data"
    title="Port série"
    aria-label="Port série"
    value={serial.current}
    onchange={(e) => serial.select(e.currentTarget.value)}
  >
    {#each serial.ports as p (p.key)}
      <option value={p.key}>{p.label}{p.firmwareBaud ? `, ${p.firmwareBaud.toLocaleString('fr-FR')} bauds` : ''}</option>
    {/each}
  </select>
  {#if port && port.id !== null}
    <select
      class="port data"
      title="Débit du moniteur"
      aria-label="Débit du moniteur"
      value={serial.monitorBaud[port.key] ?? DEFAULT_BAUD}
      onchange={(e) => (serial.monitorBaud = { ...serial.monitorBaud, [port.key]: Number(e.currentTarget.value) })}
    >
      {#each BAUDS as b (b)}
        <option value={b}>{b.toLocaleString('fr-FR')} bauds</option>
      {/each}
    </select>
  {/if}
{/if}
{#if full}
  <IconButton icon="clock" label="Afficher l'horodatage" pressed={serial.timestamps} onclick={() => (serial.timestamps = !serial.timestamps)} />
  <IconButton icon="hex" label="Vue hexadécimale" pressed={serial.hex} onclick={() => (serial.hex = !serial.hex)} />
  <IconButton icon="export" label="Exporter la sortie…" disabled={!port} onclick={exportPort} />
  <IconButton icon="trash" label="Effacer" disabled={!port} align="end" onclick={() => serial.clear()} />
{/if}

<style>
  .port {
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: transparent;
    color: var(--pb-text);
    font-size: 12px;
  }
</style>
