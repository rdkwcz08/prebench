// components/buzzer/model.ts — Modèle du buzzer piézo (API composant v1)
//
// Le son réel attendra l'audio (V2) : le modèle indique seulement s'il sonne
// et à quelle fréquence.
// - Passif : sonne à la fréquence du signal PWM présent entre + et −
//   (rapport cyclique strictement entre 0 et 100 %). Un niveau fixe ne fait
//   pas de son : avertissement une fois (il faut un signal PWM, « tone »).
// - Actif : oscillateur intégré ; sonne à `tone_hz` dès que + est haut et −
//   bas (ou sous PWM, le son est alors haché).

import type { ComponentFactory, PinState } from '../component-api';

export type BuzzerKind = 'passive' | 'active';

export type BuzzerParams = {
  kind: BuzzerKind;
  /** Fréquence de l'oscillateur d'un buzzer actif, en Hz. */
  tone_hz: number;
};

export type BuzzerState = {
  sounding: boolean;
  /** Fréquence du son en Hz ; null en silence. */
  freq_hz: number | null;
};

function driven(state: PinState): boolean {
  if (state.pwm !== undefined) return state.pwm.duty > 0;
  return state.level === 'high';
}

const createBuzzer: ComponentFactory<BuzzerParams, BuzzerState> = (ctx) => {
  let sounding = false;
  let freq: number | null = null;
  let dcWarned = false;

  function update(): void {
    const plus = ctx.pin('+');
    const minusLow = ctx.pin('-').level === 'low';
    let next: number | null = null;
    if (ctx.params.kind === 'active') {
      if (driven(plus) && minusLow) next = ctx.params.tone_hz;
    } else if (plus.pwm !== undefined && plus.pwm.freqHz > 0 && plus.pwm.duty > 0 && plus.pwm.duty < 1 && minusLow) {
      next = plus.pwm.freqHz;
    } else if (plus.pwm === undefined && plus.level === 'high' && minusLow && !dcWarned) {
      dcWarned = true;
      ctx.warn('buzzer passif alimenté par un niveau fixe : il ne sonne qu’avec un signal PWM. Utilisez un timer en PWM, ou passez le paramètre Type à « active ».');
    }
    if (next === freq) return;
    freq = next;
    sounding = next !== null;
    ctx.invalidate();
  }

  return {
    onPinChange(): void {
      update();
    },
    onParamsChange(): void {
      dcWarned = false;
      update();
    },
    onPower(): void {
      update();
    },
    getState(): BuzzerState {
      return { sounding, freq_hz: freq };
    },
  };
};

export default createBuzzer;
