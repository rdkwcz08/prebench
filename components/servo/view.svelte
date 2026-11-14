<!--
  components/servo/view.svelte — Servomoteur de modélisme vu de dessus (Svelte 5).
  Le palonnier pointe vers la gauche à 0°, vers le haut à 90°, vers la droite à
  180° ; il se place sans animation (charte, section 6 : l'état réel est la
  consigne, la mécanique n'est pas simulée). Trois fils : marron (GND), rouge
  (VCC), orange (signal).
  Coordonnées identiques au manifeste : 80 × 88, broches GND, VCC, SIG en x = 28, 40, 52.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { ServoParams, ServoState } from './model';

  let { state }: ComponentViewProps<ServoParams, ServoState> = $props();

  const angle = $derived(Math.min(180, Math.max(0, state.angle)));
  const STATUS: Record<ServoState['status'], string> = {
    ok: '',
    'out-of-range': ', signal hors plage',
    'no-signal': ', sans signal PWM',
    unpowered: ', non alimenté',
  };
</script>

<svg
  width="80"
  height="88"
  viewBox="0 0 80 88"
  role="img"
  aria-label="Servomoteur à {angle.toLocaleString('fr-FR')}°{STATUS[state.status] ?? ''}"
>
  <path d="M28 72V88" class="wire brown" />
  <path d="M40 72V88" class="wire red" />
  <path d="M52 72V88" class="wire orange" />
  <rect x="14" y="40" width="52" height="7" rx="2" class="ears" />
  <rect x="24" y="24" width="32" height="48" rx="3" class="body" />
  <circle cx="40" cy="36" r="7" class="boss" />
  <g transform="rotate({angle - 90} 40 36)">
    <path d="M40 36V12" class="horn" />
    <circle cx="40" cy="16" r="1.5" class="hole" />
    <circle cx="40" cy="22" r="1.5" class="hole" />
  </g>
  <circle cx="40" cy="36" r="3" class="screw" />
</svg>

<style>
  .wire {
    stroke-width: 2;
  }
  .brown {
    stroke: #7a4a2a;
  }
  .red {
    stroke: #d9453b;
  }
  .orange {
    stroke: #e07b39;
  }
  .ears,
  .body {
    fill: #2e5fa8;
  }
  .ears {
    filter: brightness(0.85);
  }
  .boss {
    fill: #24497f;
  }
  .horn {
    stroke: #f2f3f4;
    stroke-width: 7;
    stroke-linecap: round;
  }
  .hole {
    fill: #9aa3a9;
  }
  .screw {
    fill: #b9c0c5;
  }
</style>
