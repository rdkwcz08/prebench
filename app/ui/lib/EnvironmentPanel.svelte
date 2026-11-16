<!--
  app/ui/lib/EnvironmentPanel.svelte — Onglet Environnement (section 8.5, charte 5
  « Panneau Environnement ») : grandeurs physiques lues par les capteurs.
  Une ligne de 28 px par grandeur : libellé, curseur, valeur en chasse fixe.
  Luminosité sur une échelle logarithmique (1 lx à 100 000 lx, 0 en butée
  gauche). Accélération en lecture seule : gravité déduite de l'inclinaison
  par le cœur. Les valeurs sont gardées pendant la session de l'app, pas dans
  prebench.toml ; chaque changement part au cœur au plus une fois par image.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { fr } from './format';
  import Button from './ui/Button.svelte';

  interface Env {
    temperatureC: number;
    humidityPercent: number;
    illuminanceLux: number;
    tiltDeg: { roll: number; pitch: number };
    accelerationG: { x: number; y: number; z: number };
    distanceM: number;
  }

  const DEFAULT: Env = {
    temperatureC: 22,
    humidityPercent: 50,
    illuminanceLux: 300,
    tiltDeg: { roll: 0, pitch: 0 },
    accelerationG: { x: 0, y: 0, z: 1 },
    distanceM: 1,
  };

  let env = $state<Env>(structuredClone(DEFAULT));
  let pending = false;

  void invoke<Env>('get_environment')
    .then((e) => (env = e))
    .catch(() => {});

  /** Envoi au cœur, au plus une fois par image ; la réponse porte l'accélération calculée. */
  function push(): void {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      void invoke<Env>('set_environment', { environment: $state.snapshot(env) })
        .then((e) => (env.accelerationG = e.accelerationG))
        .catch(() => {});
    });
  }

  function reset(): void {
    env = structuredClone(DEFAULT);
    push();
  }

  /** Luminosité : position du curseur (0 à 100) <-> lux, 0 puis 1 lx à 100 000 lx. */
  const luxToPos = (lux: number): number => (lux < 1 ? 0 : 20 * Math.log10(lux));
  const posToLux = (pos: number): number => (pos <= 0 ? 0 : Math.round(10 ** (pos / 20)));

  const g = (v: number): string => `${fr(v, 2)} g`;

  /** Les flèches règlent le curseur, pas la paillasse. */
  const stop = (e: KeyboardEvent): void => e.stopPropagation();
</script>

{#snippet row(id: string, label: string, value: string)}
  <label class="label" for={id}>{label}</label>
  <span class="value data" aria-live="off">{value}</span>
{/snippet}

<div class="env">
  <div class="grid" role="group" aria-label="Environnement">
    {@render row('env-temp', 'Température', `${fr(env.temperatureC, 1)} °C`)}
    <input onkeydown={stop} id="env-temp" type="range" min="-40" max="85" step="0.5" bind:value={env.temperatureC} oninput={push} />

    {@render row('env-hum', 'Humidité', `${fr(env.humidityPercent)} %`)}
    <input onkeydown={stop} id="env-hum" type="range" min="0" max="100" step="1" bind:value={env.humidityPercent} oninput={push} />

    {@render row('env-lux', 'Luminosité', `${fr(env.illuminanceLux)} lx`)}
    <input
      onkeydown={stop}
      id="env-lux"
      type="range"
      min="0"
      max="100"
      step="1"
      value={luxToPos(env.illuminanceLux)}
      aria-valuetext="{fr(env.illuminanceLux)} lux"
      oninput={(e) => {
        env.illuminanceLux = posToLux(Number(e.currentTarget.value));
        push();
      }}
    />

    {@render row('env-roll', 'Inclinaison, roulis', `${fr(env.tiltDeg.roll)}°`)}
    <input onkeydown={stop} id="env-roll" type="range" min="-180" max="180" step="1" bind:value={env.tiltDeg.roll} oninput={push} />

    {@render row('env-pitch', 'Inclinaison, tangage', `${fr(env.tiltDeg.pitch)}°`)}
    <input onkeydown={stop} id="env-pitch" type="range" min="-90" max="90" step="1" bind:value={env.tiltDeg.pitch} oninput={push} />

    {@render row('env-dist', 'Distance', `${fr(env.distanceM, 2)} m`)}
    <input onkeydown={stop} id="env-dist" type="range" min="0.02" max="4" step="0.01" bind:value={env.distanceM} oninput={push} />

    <span class="label">Accélération</span>
    <span class="value data accel">x {g(env.accelerationG.x)}, y {g(env.accelerationG.y)}, z {g(env.accelerationG.z)}</span>
    <span class="hint">Gravité selon l'inclinaison</span>
  </div>
  <div class="actions">
    <Button icon="reset" onclick={reset}>Valeurs par défaut</Button>
    <span class="hint">Gardées jusqu'à la fermeture de l'app, pour toutes les simulations.</span>
  </div>
</div>

<style>
  .env {
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: var(--pb-font-ui);
    font-size: 13px;
  }
  .grid {
    display: grid;
    grid-template-columns: max-content 96px minmax(120px, 320px);
    column-gap: 12px;
    align-items: center;
  }
  .grid > * {
    min-height: 28px;
    display: flex;
    align-items: center;
  }
  .label {
    color: var(--pb-text-quiet);
  }
  .value {
    justify-content: flex-end;
    color: var(--pb-text);
  }
  .data {
    font: 400 12px var(--pb-font-data);
    font-variant-numeric: tabular-nums;
  }
  .accel {
    justify-content: flex-start;
    grid-column: 2 / span 2;
  }
  .accel + .hint {
    grid-column: 2 / span 2;
    min-height: 0;
  }
  input[type='range'] {
    width: 100%;
    margin: 0;
    accent-color: var(--pb-accent);
  }
  input[type='range']:focus-visible {
    outline: 2px solid var(--pb-accent);
    outline-offset: 2px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .hint {
    color: var(--pb-text-quiet);
    font-size: 12px;
  }
</style>
