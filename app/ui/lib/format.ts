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

/** Durée en ms : « 1,5 ms ». */
export function millis(ms: number): string {
  return `${fr(ms, 3)} ms`;
}
