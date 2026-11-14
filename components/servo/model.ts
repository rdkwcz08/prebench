// components/servo/model.ts — Modèle du servomoteur (API composant v1)
//
// On simule la consigne, pas la mécanique : l'angle se déduit de la largeur
// d'impulsion du signal PWM (rapport cyclique ÷ fréquence), 1 ms pour 0° et
// 2 ms pour 180°, à 50 Hz (cahier des charges, section 8.2).
// - Hors de la plage (fréquence de 40 à 60 Hz, impulsion de 1 à 2 ms) : un
//   avertissement par sortie de plage, l'angle est borné à 0° ou 180°.
// - Sans signal PWM (niveau fixe, rapport de 0 ou 100 %) : le servo garde sa position.
// - Sans alimentation (VCC non haut ou GND non bas) : il ne bouge pas.

import type { ComponentFactory, PinState } from '../component-api';

export type ServoParams = Record<string, never>;

export type ServoStatus = 'ok' | 'out-of-range' | 'no-signal' | 'unpowered';

export type ServoState = {
  /** Angle en degrés, de 0 à 180. */
  angle: number;
  /** Largeur d'impulsion en ms ; null sans signal PWM. */
  pulse_ms: number | null;
  /** Fréquence du signal en Hz ; null sans signal PWM. */
  freq_hz: number | null;
  status: ServoStatus;
};

const PULSE_MIN_MS = 1;
const PULSE_MAX_MS = 2;
const FREQ_MIN_HZ = 40;
const FREQ_MAX_HZ = 60;
/** Tolérance d'arrondi des compteurs du timer, en ms. */
const PULSE_TOLERANCE_MS = 0.01;

/** Nombre à la française, `digits` décimales au plus (QuickJS n'a pas Intl). */
function fr(value: number, digits: number): string {
  return String(Number(value.toFixed(digits))).replace('.', ',');
}

function supplied(state: PinState): boolean {
  return state.level === 'high' || (state.volts !== undefined && state.volts > 2);
}

const createServo: ComponentFactory<ServoParams, ServoState> = (ctx) => {
  let angle = 90;
  let pulse: number | null = null;
  let freq: number | null = null;
  let status: ServoStatus = 'no-signal';
  /** Avertissement déjà émis pour la sortie de plage en cours. */
  let warned = false;

  function update(): void {
    const powered = supplied(ctx.pin('VCC')) && ctx.pin('GND').level === 'low';
    const sig = ctx.pin('SIG').pwm;
    let nextStatus: ServoStatus;
    let nextAngle = angle;
    // Rapport de 0 ou 100 % : niveau fixe, aucune impulsion.
    if (sig === undefined || sig.freqHz <= 0 || sig.duty <= 0 || sig.duty >= 1) {
      pulse = null;
      freq = null;
      nextStatus = 'no-signal';
    } else {
      freq = sig.freqHz;
      pulse = (sig.duty / sig.freqHz) * 1000;
      const freqOk = freq >= FREQ_MIN_HZ && freq <= FREQ_MAX_HZ;
      const pulseOk = pulse >= PULSE_MIN_MS - PULSE_TOLERANCE_MS && pulse <= PULSE_MAX_MS + PULSE_TOLERANCE_MS;
      nextStatus = freqOk && pulseOk ? 'ok' : 'out-of-range';
      if (!freqOk && !warned) {
        warned = true;
        ctx.warn(`signal à ${fr(freq, 1)} Hz, hors de la plage d'un servomoteur (50 Hz attendus, de 40 à 60 Hz).`);
      } else if (!pulseOk && !warned) {
        warned = true;
        ctx.warn(
          `impulsion de ${fr(pulse, 2)} ms, hors de la plage de 1 à 2 ms : l'angle est borné à ${pulse < 1 ? 0 : 180}°.`,
        );
      }
      if (freqOk && pulseOk) warned = false;
      if (powered) {
        const ratio = (pulse - PULSE_MIN_MS) / (PULSE_MAX_MS - PULSE_MIN_MS);
        nextAngle = Math.round(Math.min(1, Math.max(0, ratio)) * 1800) / 10;
      }
    }
    if (!powered) nextStatus = 'unpowered';
    angle = nextAngle;
    status = nextStatus;
    ctx.invalidate(); // largeur et fréquence affichées dans l'inspecteur
  }

  return {
    onPinChange(): void {
      update();
    },
    onPower(): void {
      update();
    },
    getState(): ServoState {
      return {
        angle,
        pulse_ms: pulse === null ? null : Math.round(pulse * 1000) / 1000,
        freq_hz: freq,
        status,
      };
    },
  };
};

export default createServo;
