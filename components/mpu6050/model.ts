// components/mpu6050/model.ts — Centrale inertielle MPU6050 en I2C (API composant v1, phase 3).
//
// Registres de 8 bits, pointeur auto-incrémenté (lecture groupée des 14
// octets de mesure depuis 0x3B, comme le font les bibliothèques).
// - Accélération : gravité du panneau Environnement (accelerationG), selon la
//   plage d'ACCEL_CONFIG (±2 g : 16 384 LSB/g … ±16 g : 2 048 LSB/g).
// - Température : temperatureC, codée comme la puce (T = brut / 340 + 36,53).
// - Gyroscope : vitesse de rotation pendant un changement d'inclinaison
//   (roulis autour de x, tangage autour de y), retombée à 0 après 100 ms sans
//   changement ; plage de GYRO_CONFIG (±250 °/s : 131 LSB/(°/s) … ±2000 °/s : 16,4).
// - Veille : au reset (PWR_MGMT_1 = 0x40), les mesures lisent 0 ; écrire 0 la
//   réveille ; le bit 7 réinitialise les registres.
// - Hors tension (VCC ou GND non reliés) : la puce n'acquitte pas.
// Non simulés : filtres (CONFIG), FIFO, interruptions, DMP, bus auxiliaire.

import type { ComponentFactory, Environment, I2CResponse, PinState } from '../component-api';

export type Mpu6050Params = {
  address: number;
};

export type Mpu6050State = {
  /** Sous tension et sorti de veille. */
  awake: boolean;
  /** Plage de l'accéléromètre, en g (2, 4, 8 ou 16). */
  accel_range_g: number;
  /** Plage du gyroscope, en °/s (250, 500, 1000 ou 2000). */
  gyro_range_dps: number;
};

const REG = {
  CONFIG: 0x1a,
  GYRO_CONFIG: 0x1b,
  ACCEL_CONFIG: 0x1c,
  INT_STATUS: 0x3a,
  ACCEL_XOUT_H: 0x3b,
  PWR_MGMT_1: 0x6b,
  WHO_AM_I: 0x75,
} as const;

const ACCEL_LSB = [16384, 8192, 4096, 2048];
const GYRO_LSB = [131, 65.5, 32.8, 16.4];
/** Après ce délai sans changement d'inclinaison, le gyroscope revient à 0. */
const GYRO_HOLD_US = 100_000;

function int16(v: number): number {
  return Math.max(-32768, Math.min(32767, Math.round(v)));
}

const createMpu6050: ComponentFactory<Mpu6050Params, Mpu6050State> = (ctx) => {
  const regs = new Uint8Array(128);
  let pointer = 0;
  let env: Environment | null = null;
  let rate = { x: 0, y: 0, at: 0 };

  function reset(): void {
    regs.fill(0);
    regs[REG.PWR_MGMT_1] = 0x40; // veille
    regs[REG.WHO_AM_I] = 0x68;
    pointer = 0;
    ctx.invalidate();
  }

  function live(s: PinState): boolean {
    return s.level === 'high' || s.level === 'pulled-high' || (s.volts ?? 0) > 2;
  }

  function powered(): boolean {
    const gnd = ctx.pin('GND');
    return live(ctx.pin('VCC')) && (gnd.level === 'low' || gnd.volts === 0);
  }

  const asleep = (): boolean => ((regs[REG.PWR_MGMT_1] ?? 0) & 0x40) !== 0;

  /** Registres de mesure (0x3B à 0x48) à partir de l'environnement. */
  function sample(): void {
    const put = (reg: number, v: number): void => {
      const x = int16(v) & 0xffff;
      regs[reg] = x >> 8;
      regs[reg + 1] = x & 0xff;
    };
    if (asleep() || env === null) {
      regs.fill(0, REG.ACCEL_XOUT_H, REG.ACCEL_XOUT_H + 14);
      return;
    }
    const a = ACCEL_LSB[((regs[REG.ACCEL_CONFIG] ?? 0) >> 3) & 3] ?? 16384;
    const g = GYRO_LSB[((regs[REG.GYRO_CONFIG] ?? 0) >> 3) & 3] ?? 131;
    put(0x3b, env.accelerationG.x * a);
    put(0x3d, env.accelerationG.y * a);
    put(0x3f, env.accelerationG.z * a);
    put(0x41, (env.temperatureC - 36.53) * 340);
    const moving = ctx.now() - rate.at < GYRO_HOLD_US;
    put(0x43, moving ? rate.x * g : 0);
    put(0x45, moving ? rate.y * g : 0);
    put(0x47, 0);
  }

  function write(reg: number, value: number): void {
    if (reg === REG.PWR_MGMT_1 && (value & 0x80) !== 0) {
      reset(); // DEVICE_RESET
      return;
    }
    if (reg === REG.WHO_AM_I || (reg >= REG.ACCEL_XOUT_H && reg <= 0x48)) return; // lecture seule
    const before = regs[reg];
    regs[reg] = value;
    if (before !== value && (reg === REG.PWR_MGMT_1 || reg === REG.ACCEL_CONFIG || reg === REG.GYRO_CONFIG)) {
      ctx.invalidate();
    }
  }

  reset();
  let wasPowered = powered();

  return {
    onI2C(request): I2CResponse {
      if (!powered()) return { ack: false, nack: 'address' };
      const { tx, rxLength } = request;
      if (tx.length > 0) {
        pointer = (tx[0] ?? 0) & 0x7f;
        for (let i = 1; i < tx.length; i += 1) {
          write(pointer, tx[i] ?? 0);
          pointer = (pointer + 1) & 0x7f;
        }
      }
      if (rxLength === 0) return { ack: true };
      sample();
      regs[REG.INT_STATUS] = asleep() ? 0 : 0x01; // DATA_RDY
      const rx = new Uint8Array(rxLength);
      for (let i = 0; i < rxLength; i += 1) {
        rx[i] = regs[pointer] ?? 0;
        pointer = (pointer + 1) & 0x7f;
      }
      return { ack: true, rx };
    },
    onEnvironment(next): void {
      const now = ctx.now();
      if (env !== null) {
        const dt = (now - rate.at) / 1e6;
        const dRoll = next.tiltDeg.roll - env.tiltDeg.roll;
        const dPitch = next.tiltDeg.pitch - env.tiltDeg.pitch;
        if (dRoll !== 0 || dPitch !== 0) {
          // Intervalle borné : un premier changement après un long repos n'est pas un pic.
          const span = Math.min(Math.max(dt, 0.02), 0.2);
          rate = { x: dRoll / span, y: dPitch / span, at: now };
        }
      } else {
        rate.at = now - GYRO_HOLD_US;
      }
      env = next;
    },
    onPinChange(): void {
      const now = powered();
      if (now !== wasPowered) {
        wasPowered = now;
        reset(); // mise sous tension : registres par défaut, en veille
      }
    },
    onPower(): void {
      wasPowered = powered();
      reset();
    },
    getState(): Mpu6050State {
      return {
        awake: wasPowered && !asleep(),
        accel_range_g: 2 << (((regs[REG.ACCEL_CONFIG] ?? 0) >> 3) & 3),
        gyro_range_dps: 250 << (((regs[REG.GYRO_CONFIG] ?? 0) >> 3) & 3),
      };
    },
  };
};

export default createMpu6050;
