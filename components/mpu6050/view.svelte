<!--
  components/mpu6050/view.svelte — Module MPU6050 (GY-521) vu de dessus (Svelte 5) :
  vernis et sérigraphie de la carte (charte 2.2), puce noire, axes X et Y
  sérigraphiés comme sur le module. L'inclinaison se règle dans l'onglet
  Environnement ; la vue ne bouge pas (aucune animation, charte 6). En veille
  ou hors tension, le mot « veille » remplace le nom de la puce.
  Coordonnées identiques au manifeste : 96 × 72, broches VCC, GND, SCL, SDA en
  bas, en x = 12, 36, 60, 84.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { Mpu6050Params, Mpu6050State } from './model';

  let { state: model }: ComponentViewProps<Mpu6050Params, Mpu6050State> = $props();

  const label = $derived(
    `Accéléromètre MPU6050, ${model.awake ? `±${model.accel_range_g} g, ±${model.gyro_range_dps} °/s` : 'en veille'}`,
  );
</script>

<svg width="96" height="72" viewBox="0 0 96 72" role="img" aria-label={label}>
  <rect x="0" y="0" width="96" height="60" rx="3" class="pcb" />
  <path d="M12 60V72M36 60V72M60 60V72M84 60V72" class="leg" />
  <rect x="36" y="10" width="24" height="24" rx="1" class="chip" />
  <text x="48" y="45" class="silk middle">{model.awake ? 'MPU-6050' : 'veille'}</text>
  <!-- Axes du capteur : X vers la droite, Y vers le haut (sérigraphie du GY-521). -->
  <path d="M10 28H26M22 24L26 28L22 32M10 28V12M6 16L10 12L14 16" class="axis" />
  <text x="28" y="31" class="silk">X</text>
  <text x="13" y="11" class="silk">Y</text>
  <text x="12" y="56" class="silk middle">VCC</text>
  <text x="36" y="56" class="silk middle">GND</text>
  <text x="60" y="56" class="silk middle">SCL</text>
  <text x="84" y="56" class="silk middle">SDA</text>
</svg>

<style>
  .pcb {
    fill: var(--pb-board);
  }
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .chip {
    fill: #2a2a2a;
  }
  .axis {
    fill: none;
    stroke: var(--pb-silk);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .silk {
    fill: var(--pb-silk);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
  }
  .middle {
    text-anchor: middle;
  }
</style>
