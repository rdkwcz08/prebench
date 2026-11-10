// components/button/model.ts — Modèle du bouton poussoir (API composant v1)
//
// Appuyé : la broche 1 prend le niveau de la broche 2 (masse -> bas, 3V3 -> haut).
// Relâché : la broche 1 est en haute impédance ; un pull-up (carte, firmware
// ou composant) fixe alors son niveau. Pas d'anti-rebond : un clic est propre.

import type { ComponentFactory, DriveLevel, NetLevel } from '../component-api';

export type ButtonColor = 'black' | 'blue' | 'red' | 'green' | 'yellow' | 'white';

export type ButtonParams = {
  color: ButtonColor;
};

export type ButtonState = {
  pressed: boolean;
};

function levelToDrive(level: NetLevel): DriveLevel {
  switch (level) {
    case 'low':
    case 'pulled-low':
      return 'low';
    case 'high':
    case 'pulled-high':
      return 'high';
    default:
      return 'z';
  }
}

const createButton: ComponentFactory<ButtonParams, ButtonState> = (ctx) => {
  let pressed = false;
  let driven: DriveLevel = 'z';

  function update(): void {
    const next: DriveLevel = pressed ? levelToDrive(ctx.pin('2').level) : 'z';
    if (next !== driven) {
      driven = next;
      ctx.drive('1', next);
    }
  }

  return {
    onControl(control, value): void {
      if (control !== 'press') return;
      const next = value === true;
      if (next !== pressed) {
        pressed = next;
        update();
        ctx.invalidate();
      }
    },
    onPinChange(pin): void {
      if (pin === '2') update();
    },
    onPower(on): void {
      if (!on && pressed) {
        pressed = false;
        update();
        ctx.invalidate();
      }
    },
    getState(): ButtonState {
      return { pressed };
    },
  };
};

export default createButton;
