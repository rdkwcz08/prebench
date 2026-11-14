// components/potentiometer/model.ts — Modèle du potentiomètre (API composant v1)
//
// Pont diviseur idéal : la tension du curseur (W) vaut
//   V(GND) + position × (V(VCC) − V(GND)),
// position de 0 (bouton à fond à gauche, côté GND) à 1 (à fond à droite, côté VCC).
// La tension de chaque extrémité est celle du net (alimentation de la carte),
// sinon celle du niveau logique (sortie haute = tension logique de la carte).
// Une extrémité flottante (non câblée, alimentation coupée) laisse le curseur
// sans tension : le net redevient logique et l'ADC lit 0.
// Aucun courant n'est calculé : la résistance n'est qu'une indication.

import type { ComponentFactory, PinState } from '../component-api';

export type PotentiometerParams = {
  /** Résistance totale, en kΩ (indication, pas de calcul de courant). */
  resistance_kohm: number;
};

export type PotentiometerState = {
  /** Position du bouton, de 0 à 1. */
  position: number;
  /** Tension du curseur en volts ; null si une extrémité n'a pas de tension. */
  volts: number | null;
};

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

const createPotentiometer: ComponentFactory<PotentiometerParams, PotentiometerState> = (ctx) => {
  let position = 0.5;
  let volts: number | null = null;

  /** Tension d'une extrémité ; null si elle n'est fixée par rien. */
  function endVolts(state: PinState): number | null {
    if (state.volts !== undefined) return state.volts;
    if (state.pwm !== undefined) return state.pwm.duty * ctx.logicVoltage;
    if (state.level === 'high') return ctx.logicVoltage;
    if (state.level === 'low') return 0;
    return null;
  }

  function update(): void {
    const low = endVolts(ctx.pin('GND'));
    const high = endVolts(ctx.pin('VCC'));
    const next = low === null || high === null ? null : low + position * (high - low);
    if (next === volts) return;
    volts = next;
    ctx.setVoltage('W', next ?? Number.NaN);
    ctx.invalidate();
  }

  return {
    onPinChange(pin): void {
      if (pin !== 'W') update(); // W : la tension qu'il vient de fixer
    },
    onControl(control, value): void {
      if (control !== 'position' || typeof value !== 'number' || !Number.isFinite(value)) return;
      const next = clamp01(value);
      if (next === position) return;
      position = next;
      ctx.invalidate();
      update();
    },
    onPower(): void {
      update();
    },
    getState(): PotentiometerState {
      return { position, volts };
    },
  };
};

export default createPotentiometer;
