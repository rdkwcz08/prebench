// components/joker-i2c/model.ts — Périphérique I2C générique (joker, section 8.3).
//
// Esclave à registres de 8 bits, comme la plupart des capteurs I2C :
// - écriture [reg, d0, d1, …] : le pointeur passe à reg, puis d0, d1… sont
//   écrits dans reg, reg + 1… ;
// - lecture de n octets : registres à partir du pointeur, qui avance d'autant
//   (Mem_Read = écriture du numéro de registre puis lecture, start répété) ;
// - le pointeur et les adresses bouclent sur 0x00..0xFF.
// La table de registres (paramètre « registers ») donne les valeurs de départ ;
// les lignes « slider » se règlent pendant la simulation (contrôle « reg:XX »),
// quatre au plus. Une écriture du firmware garde sa valeur jusqu'au prochain
// changement de paramètres, réglage du curseur ou coupure d'alimentation.
// Phase 3 : valeurs fixes et curseurs ; scripts en phase 5.

import type { ComponentFactory, I2CResponse } from '../component-api';

export type RegisterRow = {
  reg: number;
  value: number;
  mode: 'fixed' | 'slider';
};

export type JokerI2cParams = {
  address: number;
  registers: readonly RegisterRow[];
};

export type JokerI2cState = {
  address: number;
  /** Registres réglables par curseur (quatre au plus), dans l'ordre de la table. */
  sliders: { reg: number; value: number }[];
  /** Pointeur de registre courant. */
  pointer: number;
};

/** Curseurs affichés sur la vue. */
const MAX_SLIDERS = 4;

function byte(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.round(value) & 0xff : fallback;
}

const createJokerI2c: ComponentFactory<JokerI2cParams, JokerI2cState> = (ctx) => {
  const memory = new Uint8Array(256);
  let sliderRegs: number[] = [];
  let pointer = 0;
  let warnedSliders = false;

  function rows(): RegisterRow[] {
    const table = ctx.params.registers;
    return Array.isArray(table) ? table : [];
  }

  /** Mémoire et curseurs reconstruits depuis la table. */
  function load(): void {
    memory.fill(0);
    sliderRegs = [];
    for (const row of rows()) {
      const reg = byte(row.reg);
      memory[reg] = byte(row.value);
      if (row.mode === 'slider' && !sliderRegs.includes(reg)) sliderRegs.push(reg);
    }
    if (sliderRegs.length > MAX_SLIDERS) {
      if (!warnedSliders) {
        ctx.warn(`${sliderRegs.length} registres en mode curseur : seuls les ${MAX_SLIDERS} premiers sont réglables sur la paillasse.`);
        warnedSliders = true;
      }
      sliderRegs = sliderRegs.slice(0, MAX_SLIDERS);
    }
    pointer = 0;
    ctx.invalidate();
  }

  load();

  return {
    onI2C(request): I2CResponse {
      const { tx, rxLength } = request;
      if (tx.length > 0) {
        pointer = tx[0] ?? 0;
        for (let i = 1; i < tx.length; i += 1) {
          memory[pointer] = tx[i] ?? 0;
          pointer = (pointer + 1) & 0xff;
        }
      }
      const rx = new Uint8Array(rxLength);
      for (let i = 0; i < rxLength; i += 1) {
        rx[i] = memory[pointer] ?? 0;
        pointer = (pointer + 1) & 0xff;
      }
      if (tx.length > 1 || rxLength > 0) ctx.invalidate();
      return { ack: true, rx };
    },
    onControl(control, value): void {
      const match = /^reg:([0-9a-f]{2})$/i.exec(control);
      if (match === null || typeof value !== 'number') return;
      const reg = Number.parseInt(match[1] ?? '', 16);
      if (!sliderRegs.includes(reg)) return;
      memory[reg] = byte(value);
      ctx.invalidate();
    },
    onParamsChange(): void {
      warnedSliders = false;
      load();
    },
    onPower(): void {
      load(); // la mémoire d'une puce ne survit pas à une coupure
    },
    getState(): JokerI2cState {
      return {
        address: byte(ctx.params.address, 0x50),
        sliders: sliderRegs.map((reg) => ({ reg, value: memory[reg] ?? 0 })),
        pointer,
      };
    },
  };
};

export default createJokerI2c;
