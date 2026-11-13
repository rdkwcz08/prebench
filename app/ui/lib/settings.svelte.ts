// app/ui/lib/settings.svelte.ts — Réglages vus par l'interface (section 10.7).
//
// Les réglages sont déclarés dans le cœur (app/src-tauri/src/settings.rs), avec
// leur valeur par défaut ; l'interface les lit au démarrage (get_settings) et
// modifie ceux de l'app (set_setting). Aucun défaut n'est recopié ici, sauf en
// repli si le cœur ne répond pas.

import { invoke } from '@tauri-apps/api/core';

export interface DeclaredSetting {
  key: string;
  scope: 'app' | 'project';
  label: string;
  values: string[];
  editable: boolean;
}

class Settings {
  declared = $state<DeclaredSetting[]>([]);
  values = $state<Record<string, unknown>>({});

  async load(): Promise<void> {
    try {
      const view = await invoke<{ declared: DeclaredSetting[]; values: Record<string, unknown> }>('get_settings');
      this.declared = view.declared;
      this.values = view.values;
    } catch {
      // Cœur injoignable : valeurs de repli des appelants.
    }
  }

  number(key: string, fallback: number): number {
    const v = this.values[key];
    return typeof v === 'number' ? v : fallback;
  }

  text(key: string, fallback: string): string {
    const v = this.values[key];
    return typeof v === 'string' ? v : fallback;
  }

  async set(key: string, value: unknown): Promise<void> {
    this.values = { ...this.values, [key]: value };
    await invoke('set_setting', { key, value });
  }
}

export const settings = new Settings();
