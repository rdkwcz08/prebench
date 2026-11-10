// components/led/model.ts — Modèle de la LED (API composant v1)
//
// La LED est une charge passive : elle ne pilote aucune broche. Elle brille
// quand le courant peut passer de l'anode (A) vers la cathode (K) :
//   - A au niveau haut et K au niveau bas -> allumée ;
//   - PWM sur A (ou sur K, montage actif bas) -> luminosité = rapport cyclique.
// Un tirage faible (pull-up), un net flottant ou en conflit n'allument pas la LED.

import type { ComponentFactory, PinState } from '../component-api';

export type LedColor = 'red' | 'green' | 'blue' | 'yellow' | 'orange' | 'white';

export type LedParams = {
  color: LedColor;
};

export type LedState = {
  /** Luminosité de 0 (éteinte) à 1 (pleine). */
  brightness: number;
};

/** Part du temps où la broche est au niveau haut et pilotée (0..1). */
function highRatio(state: PinState): number {
  if (state.pwm !== undefined) return clamp01(state.pwm.duty);
  return state.level === 'high' ? 1 : 0;
}

/** Part du temps où la broche est au niveau bas et pilotée (0..1). */
function lowRatio(state: PinState): number {
  if (state.pwm !== undefined) return clamp01(1 - state.pwm.duty);
  return state.level === 'low' ? 1 : 0;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

const createLed: ComponentFactory<LedParams, LedState> = (ctx) => {
  let brightness = 0;

  function update(): void {
    const next = highRatio(ctx.pin('A')) * lowRatio(ctx.pin('K'));
    if (next !== brightness) {
      brightness = next;
      ctx.invalidate();
    }
  }

  return {
    onPinChange(): void {
      update();
    },
    onPower(on: boolean): void {
      if (!on && brightness !== 0) {
        brightness = 0;
        ctx.invalidate();
      } else if (on) {
        update();
      }
    },
    getState(): LedState {
      return { brightness };
    },
  };
};

export default createLed;
