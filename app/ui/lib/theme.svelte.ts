// app/ui/lib/theme.svelte.ts — Thème de l'interface (charte, section 2).
//
// Préférence « system » (défaut), « light » ou « dark ». Le thème effectif est
// posé sur <html data-theme> ; en mode « system », il suit prefers-color-scheme
// en direct. Le choix est un réglage de l'app (section 10.7, settings.toml) ;
// une copie dans le stockage de la WebView évite seulement l'éclair d'un
// mauvais thème au démarrage, avant la réponse du cœur.

import { settings } from './settings.svelte';

export type ThemePreference = 'system' | 'light' | 'dark';

const KEY = 'prebench.theme';
const ORDER: ThemePreference[] = ['system', 'light', 'dark'];

export const THEME_LABEL: Record<ThemePreference, string> = {
  system: 'Thème du système',
  light: 'Thème clair',
  dark: 'Thème sombre',
};

function readPreference(): ThemePreference {
  try {
    const v = localStorage.getItem(KEY);
    if (v === 'light' || v === 'dark' || v === 'system') return v;
  } catch {
    // stockage indisponible : thème du système
  }
  return 'system';
}

class Theme {
  preference = $state<ThemePreference>('system');
  private media: MediaQueryList | null = null;

  /** À appeler avant le premier rendu, pour éviter un flash du mauvais thème. */
  init(): void {
    this.media = window.matchMedia('(prefers-color-scheme: light)');
    this.media.addEventListener('change', () => this.apply());
    this.preference = readPreference();
    this.apply();
  }

  /** Préférence lue dans les réglages de l'app (fait foi sur la copie locale). */
  adoptSetting(): void {
    const v = settings.text('theme', this.preference);
    if (v === 'system' || v === 'light' || v === 'dark') {
      this.preference = v;
      this.cache(v);
      this.apply();
    }
  }

  /** Préférence suivante : système, clair, sombre. */
  get next(): ThemePreference {
    return ORDER[(ORDER.indexOf(this.preference) + 1) % ORDER.length] ?? 'system';
  }

  cycle(): void {
    this.set(this.next);
  }

  set(preference: ThemePreference): void {
    this.preference = preference;
    this.cache(preference);
    this.apply();
    settings.set('theme', preference).catch(() => {
      // non enregistré : le thème reste appliqué pour cette session
    });
  }

  private cache(preference: ThemePreference): void {
    try {
      localStorage.setItem(KEY, preference);
    } catch {
      // copie locale indisponible : sans conséquence
    }
  }

  private apply(): void {
    const effective =
      this.preference === 'system' ? (this.media?.matches === true ? 'light' : 'dark') : this.preference;
    document.documentElement.dataset.theme = effective;
  }
}

export const theme = new Theme();
