// components/led-rgb/model.ts — Modèle de la LED RGB (API composant v1)
//
// Trois LED dans un même boîtier, reliées par une patte commune :
// - cathode commune (COM à la masse) : un canal brille quand sa patte est
//   haute et COM basse ;
// - anode commune (COM à 3V3) : un canal brille quand sa patte est basse et
//   COM haute.
// Sous PWM, la luminosité d'un canal suit la part du temps où le courant passe.
// Charge passive : la LED ne pilote aucune broche.

import type { ComponentFactory, PinState } from '../component-api';

export type LedRgbCommon = 'cathode' | 'anode';

export type LedRgbParams = {
  common: LedRgbCommon;
};

export type LedRgbState = {
  /** Luminosité de chaque canal, de 0 à 1. */
  r: number;
  g: number;
  b: number;
};

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

/** Part du temps où la broche est pilotée haut (0..1). */
function highRatio(state: PinState): number {
  if (state.pwm !== undefined) return clamp01(state.pwm.duty);
  return state.level === 'high' ? 1 : 0;
}

/** Part du temps où la broche est pilotée bas (0..1). */
function lowRatio(state: PinState): number {
  if (state.pwm !== undefined) return clamp01(1 - state.pwm.duty);
  return state.level === 'low' ? 1 : 0;
}

const createLedRgb: ComponentFactory<LedRgbParams, LedRgbState> = (ctx) => {
  let current: LedRgbState = { r: 0, g: 0, b: 0 };

  function channel(pin: string): number {
    const com = ctx.pin('COM');
    const own = ctx.pin(pin);
    return ctx.params.common === 'anode' ? lowRatio(own) * highRatio(com) : highRatio(own) * lowRatio(com);
  }

  function update(): void {
    const next = { r: channel('R'), g: channel('G'), b: channel('B') };
    if (next.r === current.r && next.g === current.g && next.b === current.b) return;
    current = next;
    ctx.invalidate();
  }

  return {
    onPinChange(): void {
      update();
    },
    onParamsChange(): void {
      update();
    },
    onPower(): void {
      update();
    },
    getState(): LedRgbState {
      return current;
    },
  };
};

export default createLedRgb;
