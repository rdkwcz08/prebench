// app/ui/lib/format.ts — Valeurs mesurées en français (inspecteur, pastilles, vues).
//
// Données affichées en chasse fixe : « 20 kHz », « 50 % », « 1,65 V »,
// « 2 048 / 4 095 ». Vocabulaire de l'électronicien (charte, section 7).

/** Nombre à la française (espace des milliers, virgule décimale). */
export function fr(value: number, digits = 0): string {
  return value.toLocaleString('fr-FR', { maximumFractionDigits: digits });
}

/** « 50 Hz », « 1 kHz », « 2,3 kHz », « 20 kHz », « 1,5 MHz ». */
export function frequency(hz: number): string {
  if (hz >= 1_000_000) return `${fr(hz / 1_000_000, 2)} MHz`;
  if (hz >= 1000) return `${fr(hz / 1000, 2)} kHz`;
  return `${fr(hz, 1)} Hz`;
}

/** Rapport de 0 à 1 en pourcentage : « 50 % », « 7,5 % ». */
export function percent(ratio: number): string {
  const p = ratio * 100;
  return `${fr(p, p > 0 && p < 10 ? 1 : 0)} %`;
}

/** Tension : « 1,65 V », « 0 V ». */
export function volts(v: number): string {
  return `${fr(v, 2)} V`;
}

/** Valeur brute de l'ADC sur son maximum : « 2 048 / 4 095 ». */
export function adcRaw(raw: number, max: number): string {
  return `${fr(raw)} / ${fr(max)}`;
}

/** Entier en hexadécimal : « 0x3C » (adresses I2C, registres). */
export function hex(value: number, digits = 2): string {
  return `0x${Math.round(value).toString(16).toUpperCase().padStart(digits, '0')}`;
}

/**
 * Saisie d'un entier : en hexadécimal (« 0x3C » ou « 3C ») si `asHex`, sinon
 * en décimal ou avec le préfixe 0x. NaN si la saisie n'est pas un entier.
 */
export function parseInteger(text: string, asHex: boolean): number {
  const t = text.trim().replace(/^0x/i, (p) => (asHex ? '' : p));
  if (asHex) return /^[0-9a-f]+$/i.test(t) ? Number.parseInt(t, 16) : Number.NaN;
  if (/^0x[0-9a-f]+$/i.test(t)) return Number.parseInt(t.slice(2), 16);
  return /^-?\d+$/.test(t) ? Number.parseInt(t, 10) : Number.NaN;
}

/** Durée en ms : « 1,5 ms ». */
export function millis(ms: number): string {
  return `${fr(ms, 3)} ms`;
}
