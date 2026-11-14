<!--
  app/ui/lib/SelectionInspector.svelte — Inspecteur de l'élément sélectionné sur la paillasse
  (maquette : titre, sous-titre, sections Configuration, État, Net ; valeurs de
  données en chasse fixe ; états en pastille, toujours doublés d'un mot).
  - Broche de la carte : mode, fonction et tirage réglés par le firmware,
    niveau et période mesurée, membres du net et fils.
  - Broche d'un composant : rôle, niveau, membres du net.
  - Fil : extrémités et couleur, niveau du net, membres du net.
  - Composant : type et paramètres, état du modèle et niveau de chaque
    broche, mesures des broches de la carte reliées (PWM, ADC), liaisons de
    chaque broche. Nom affiché et paramètres modifiables en Édition.
  - Note : texte (modifiable en Édition), suppression.
  Étape 8 : fréquence et rapport cyclique d'une sortie PWM, tension et valeur
  brute d'une entrée ADC, dans la section État (broche, fil, composant).
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import { boardPads, defaultColor, LEVEL_TONE, LEVEL_WORD, WIRE_HEX, WIRE_LABEL } from './geometry';
  import { adcRaw, frequency, fr, millis, percent, volts } from './format';
  import { memberLabel, Netlist, netKey } from './netlist';
  import type { Level, ParamDef, WireColor } from './types';
  import Button from './ui/Button.svelte';
  import PropList, { type Prop } from './ui/PropList.svelte';

  const COLORS = Object.keys(WIRE_HEX) as WireColor[];

  /** GPIO_CONFIG (docs/protocol.md) : mode et tirage en mots. */
  const MODE_WORD = ['Entrée', 'Sortie push-pull', 'Sortie drain ouvert', 'Alternative push-pull', 'Alternative drain ouvert', 'Analogique', 'Désactivée'];
  const PULL_WORD = ['Aucune', 'Pull-up', 'Pull-down'];
  /** Fonction de chaque couleur automatique (charte 2.4). */
  const WIRE_FUNCTION: Record<WireColor, string> = {
    red: 'alimentation',
    black: 'masse',
    yellow: 'signal',
    green: 'analogique',
    blue: 'bus',
    orange: 'bus',
    purple: 'bus',
  };

  const project = $derived(bench.project);
  const pads = $derived(project ? boardPads(project) : []);
  const placed = $derived(bench.instances.map((i) => ({ id: i.id, type: i.type, pos: i.pos })));
  const sel = $derived(bench.selection);
  const editing = $derived(bench.mode === 'edit');
  const netlist = $derived(project ? new Netlist(project, pads, bench.wires) : null);
  const off = $derived(bench.status.state === 'off');

  /** Libellés français des états de modèle connus ; les autres gardent leur clé. */
  const STATE_LABEL: Record<string, string> = {
    brightness: 'Luminosité',
    pressed: 'Appuyé',
    angle: 'Angle',
    pulse_ms: 'Impulsion',
    freq_hz: 'Fréquence',
    status: 'Signal',
    position: 'Position',
    volts: 'Tension du curseur',
    sounding: 'Son',
    r: 'Rouge',
    g: 'Vert',
    b: 'Bleu',
  };
  /** Valeurs de mots (paramètres enum, statut d'un modèle) en français. */
  const VALUE_WORD: Record<string, string> = {
    red: 'Rouge',
    green: 'Vert',
    blue: 'Bleu',
    yellow: 'Jaune',
    orange: 'Orange',
    white: 'Blanc',
    black: 'Noir',
    cathode: 'Cathode commune',
    anode: 'Anode commune',
    passive: 'Passif',
    active: 'Actif',
    ok: 'Dans la plage',
    'out-of-range': 'Hors plage',
    'no-signal': 'Sans signal PWM',
    unpowered: 'Non alimenté',
  };

  /** Valeur d'état d'un modèle, en texte, selon sa clé. */
  function modelValue(key: string, v: unknown): string {
    if (v === null) return key === 'volts' ? 'Sans tension' : 'Aucun';
    if (typeof v === 'number') {
      if (['brightness', 'position', 'r', 'g', 'b'].includes(key)) return percent(v);
      if (key === 'angle') return `${fr(v, 1)}°`;
      if (key === 'pulse_ms') return millis(v);
      if (key === 'freq_hz') return frequency(v);
      if (key === 'volts') return volts(v);
    }
    if (typeof v === 'string') return VALUE_WORD[v] ?? v;
    return stateValue(v);
  }

  /** Mesures des broches de la carte d'un net : PWM (fréquence, rapport) et ADC (tension, valeur brute). */
  function measures(key: string | null, suffix = false): Prop[] {
    if (project === null || netlist === null || key === null) return [];
    const out: Prop[] = [];
    for (const m of netlist.members(key)) {
      if (!m.startsWith('sig:')) continue;
      const signal = m.slice(4);
      const fw = bench.pins[signal];
      const at = suffix ? ` (${project.pins[signal]?.label ?? signal})` : '';
      if (fw?.pwm) {
        out.push({ label: `Fréquence${at}`, value: frequency(fw.pwm.freq_hz), data: true });
        out.push({ label: `Rapport cyclique${at}`, value: percent(fw.pwm.duty), data: true });
      }
      if (fw?.adc) {
        out.push({ label: `Tension${at}`, value: volts(fw.adc.volts), data: true });
        out.push({ label: `Valeur ADC${at}`, value: adcRaw(fw.adc.raw, fw.adc.max), data: true });
      }
    }
    return out;
  }

  /** Capacité ADC d'une broche de la carte (« adc1:CH0 »). */
  function adcCap(signal: string): string | undefined {
    return project?.pins[signal]?.caps.find((c) => c.startsWith('adc'));
  }

  /** « uart2:TX » -> « USART2 TX », « i2c1:SCL » -> « I2C1 SCL ». */
  function functionLabel(cap: string): string {
    const [bus = cap, signal = ''] = cap.split(':');
    const name = bus.toUpperCase().replace(/^UART/, 'USART');
    return signal ? `${name} ${signal}` : name;
  }

  /** Période en µs : « 1 000 ms », « 2,5 ms », « 250 µs ». */
  function period(us: number): string {
    if (us < 1000) return `${us.toLocaleString('fr-FR')} µs`;
    const ms = us / 1000;
    return `${ms.toLocaleString('fr-FR', { maximumFractionDigits: ms < 100 ? 1 : 0 })} ms`;
  }

  /** Valeur d'état d'un modèle de composant, en texte. */
  function stateValue(v: unknown): string {
    if (typeof v === 'number') return v.toLocaleString('fr-FR', { maximumFractionDigits: 3 });
    if (typeof v === 'boolean') return v ? 'Oui' : 'Non';
    if (typeof v === 'string') return v;
    return JSON.stringify(v);
  }

  /** Niveau d'un net en propriété : pastille, ou phrase hors simulation. */
  function levelProp(label: string, level: Level | undefined): Prop {
    if (level) return { label, value: LEVEL_WORD[level], tone: LEVEL_TONE[level] };
    if (off) return { label, value: 'Hors tension' };
    return { label, value: bench.live ? 'Non utilisé' : 'Hors simulation' };
  }

  /** Autres membres du net d'une extrémité, ou « Aucun ». */
  function others(key: string | null): string {
    if (project === null || netlist === null || key === null) return 'Aucun';
    const list = netlist
      .members(key)
      .filter((m) => m !== key)
      .map((m) => memberLabel(project, m));
    return list.length > 0 ? list.join(', ') : 'Aucun';
  }

  /** Fils qui touchent une extrémité : couleur et fonction (maquette « Jaune, signal »). */
  function wireProps(endpoints: string[]): Prop[] {
    if (project === null) return [];
    return bench.wires
      .filter((w) => endpoints.includes(w.from) || endpoints.includes(w.to))
      .map((w) => {
        const auto = defaultColor(project, pads, placed, w);
        const color = w.color ?? auto;
        return { label: 'Fil', value: `${WIRE_LABEL[color]}, ${WIRE_FUNCTION[auto]}`, swatch: WIRE_HEX[color] };
      });
  }

  const view = $derived.by(() => {
    if (project === null || sel === null || netlist === null) return null;
    if (sel.kind === 'pin' && sel.endpoint.startsWith('board.')) {
      const pad = pads.find((p) => p.endpoint === sel.endpoint);
      if (!pad) return null;
      const def = project.pins[pad.signal];
      const volts = project.power[pad.signal];
      const fw = bench.pins[pad.signal];
      const key = `sig:${pad.signal}`;
      const config: Prop[] = [];
      if (volts !== undefined) {
        config.push({ label: 'Alimentation', value: `${volts.toLocaleString('fr-FR')} V`, data: true });
      } else if (fw) {
        config.push({ label: 'Mode', value: MODE_WORD[fw.mode] ?? `Mode ${fw.mode}` });
        const adc = fw.mode === 5 ? adcCap(pad.signal) : undefined;
        config.push({
          label: 'Fonction',
          value: fw.function ? functionLabel(fw.function) : adc ? functionLabel(adc) : fw.mode === 5 ? 'Analogique' : 'GPIO',
          data: fw.function !== null || adc !== undefined,
        });
        config.push({ label: 'Résistance de tirage', value: PULL_WORD[fw.pull] ?? '?' });
      } else if (def) {
        config.push({ label: 'Mode', value: bench.live && !off ? 'Non configurée par le firmware' : 'Hors simulation' });
      }
      if (def?.external_pull) {
        config.push({ label: 'Sur la carte', value: def.external_pull === 'up' ? 'Pull-up soudé' : 'Pull-down soudé' });
      }
      if (def) config.push({ label: 'Capacités', value: def.caps.join(', '), data: true, full: def.caps.join(', ') });
      if (!pad.wireable) config.push({ label: 'Câblage', value: 'Non câblable' });
      const state: Prop[] = [];
      if (volts !== undefined && volts > 0 && off) state.push({ label: 'Tension', value: 'Coupée' });
      else if (!fw?.adc) state.push(levelProp('Niveau', bench.nets[pad.signal] ?? netlist.level(key, bench.nets)));
      if (fw?.period_us && !fw.pwm) state.push({ label: 'Période mesurée', value: period(fw.period_us), data: true });
      state.push(...measures(key));
      return {
        title: def ? `Broche ${pad.signal}` : pad.signal,
        subtitle: `Connecteur ${pad.header}, repère ${pad.label}`,
        sections: [
          { title: 'Configuration', props: config },
          { title: 'État', props: state },
          {
            title: 'Net',
            props: [
              { label: 'Relié à', value: others(key), data: true, full: others(key) },
              ...wireProps(pads.filter((p) => p.signal === pad.signal).map((p) => p.endpoint)),
            ],
          },
        ],
      };
    }
    if (sel.kind === 'pin') {
      const [id, pin] = sel.endpoint.split('.');
      const inst = bench.instances.find((i) => i.id === id);
      const def = inst?.info?.manifest.pins.find((p) => p.name === pin);
      return {
        title: `Broche ${sel.endpoint}`,
        subtitle: def?.label ?? inst?.info?.manifest.name ?? '',
        sections: [
          { title: 'Configuration', props: [{ label: 'Rôle', value: def?.role ?? '?', data: true }] },
          {
            title: 'État',
            props: [levelProp('Niveau', netlist.level(sel.endpoint, bench.nets)), ...measures(sel.endpoint)],
          },
          {
            title: 'Net',
            props: [
              { label: 'Relié à', value: others(sel.endpoint), data: true, full: others(sel.endpoint) },
              ...wireProps([sel.endpoint]),
            ],
          },
        ],
      };
    }
    if (sel.kind === 'wire') {
      const w = bench.wires[sel.index];
      if (!w) return null;
      const auto = defaultColor(project, pads, placed, w);
      const color = w.color ?? auto;
      const key = netKey(project, pads, w.from) ?? netKey(project, pads, w.to);
      const members = key === null ? [] : netlist.members(key).map((m) => memberLabel(project, m));
      return {
        title: 'Fil',
        subtitle: `${w.from.replace(/^board\./, '')} vers ${w.to.replace(/^board\./, '')}`,
        sections: [
          {
            title: 'Configuration',
            props: [
              { label: 'De', value: w.from.replace(/^board\./, ''), data: true },
              { label: 'Vers', value: w.to.replace(/^board\./, ''), data: true },
              {
                label: 'Couleur',
                value: `${WIRE_LABEL[color]}, ${WIRE_FUNCTION[auto]}${w.color ? '' : ' (auto)'}`,
                swatch: WIRE_HEX[color],
              },
            ],
          },
          {
            title: 'État',
            props: [levelProp('Niveau', key === null ? undefined : netlist.level(key, bench.nets)), ...measures(key)],
          },
          {
            title: 'Net',
            props: [{ label: 'Membres', value: members.join(', ') || 'Aucun', data: true, full: members.join(', ') }],
          },
        ],
      };
    }
    if (sel.kind === 'note') {
      const n = bench.notes[sel.index];
      if (!n) return null;
      return {
        title: 'Note',
        subtitle: 'Repère posé sur la paillasse',
        sections: [{ title: 'Position', props: [{ label: 'Position', value: `${n.pos[0]}, ${n.pos[1]}`, data: true }] }],
      };
    }
    const inst = bench.instances.find((i) => i.id === sel.id);
    if (!inst) return null;
    const manifest = inst.info?.manifest;
    const config: Prop[] = [
      { label: 'Type', value: manifest?.name ?? inst.type },
      { label: 'Identifiant', value: inst.id, data: true },
      ...Object.entries(inst.params).map(([k, v]) => {
        const def = manifest?.params?.[k];
        const word = typeof v === 'string' ? VALUE_WORD[v] : undefined;
        return {
          label: def?.label ?? k,
          value: word ?? `${stateValue(v)}${def?.unit ? ` ${def.unit}` : ''}`,
          data: word === undefined,
        };
      }),
      inst.onboard
        ? { label: 'Emplacement', value: 'Intégré à la carte' }
        : { label: 'Position', value: `${inst.pos[0]}, ${inst.pos[1]}`, data: true },
    ];
    const modelState =
      inst.state !== null && typeof inst.state === 'object'
        ? Object.entries(inst.state as Record<string, unknown>).map(([k, v]) => ({
            label: STATE_LABEL[k] ?? k,
            value: modelValue(k, v),
            data: !(typeof v === 'string' && v in VALUE_WORD),
          }))
        : [];
    const pinsOf = manifest?.pins ?? [];
    return {
      title: inst.name || inst.id,
      subtitle: inst.name ? `${inst.id}, ${manifest?.name ?? inst.type}` : (manifest?.name ?? inst.type),
      sections: [
        { title: 'Configuration', props: config },
        {
          title: 'État',
          props: [
            ...modelState,
            // Broche reliée à une entrée ADC : tension et valeur brute plutôt qu'un niveau logique.
            ...pinsOf.flatMap((p) => {
              const m = measures(`${inst.id}.${p.name}`, true);
              const analog = m.some((x) => x.label.startsWith('Valeur ADC'));
              return analog ? m : [levelProp(p.label ?? p.name, netlist.level(`${inst.id}.${p.name}`, bench.nets)), ...m];
            }),
          ],
        },
        {
          title: 'Net',
          props: pinsOf.map((p) => {
            const v = others(`${inst.id}.${p.name}`);
            return { label: p.label ?? p.name, value: v, data: true, full: v };
          }),
        },
      ],
    };
  });

  const wire = $derived(sel?.kind === 'wire' ? bench.wires[sel.index] : undefined);
  const autoColor = $derived(wire && project ? defaultColor(project, pads, placed, wire) : 'yellow');
  const comp = $derived(sel?.kind === 'component' ? bench.instances.find((i) => i.id === sel.id) : undefined);
  const note = $derived(sel?.kind === 'note' ? bench.notes[sel.index] : undefined);
  /** Paramètres déclarés par le manifeste du composant sélectionné. */
  const paramDefs = $derived(Object.entries(comp?.info?.manifest.params ?? {}) as [string, ParamDef][]);
  const deletable = $derived(sel?.kind === 'wire' || sel?.kind === 'note' || (comp !== undefined && !comp.onboard));
  const DELETE_LABEL = { wire: 'Supprimer le fil', note: 'Supprimer la note', component: 'Supprimer le composant', pin: '' };

  /** Valide le champ au départ du focus ou à Entrée ; Échap rétablit. */
  function commitOnKey(e: KeyboardEvent, reset: () => void): void {
    if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
    if (e.key === 'Escape') {
      reset();
      (e.currentTarget as HTMLInputElement).blur();
    }
    e.stopPropagation(); // Supprimer, E, Espace : touches du texte, pas de la paillasse
  }
</script>

{#if view}
  <div class="insp-head">
    <h2>{view.title}</h2>
    {#if view.subtitle}<p>{view.subtitle}</p>{/if}
  </div>
  {#each view.sections as s (s.title)}
    <section class="insp-section">
      <h3>{s.title}</h3>
      <PropList props={s.props} />
    </section>
  {/each}
  {#if wire && sel?.kind === 'wire'}
    {@const index = sel.index}
    <section class="insp-section">
      <h3>Couleur</h3>
      <div class="swatches" role="radiogroup" aria-label="Couleur du fil">
        <button
          class="swatch auto"
          role="radio"
          aria-checked={!wire.color}
          disabled={!editing}
          title="Selon la fonction ({WIRE_LABEL[autoColor].toLowerCase()})"
          style:--c={WIRE_HEX[autoColor]}
          onclick={() => bench.setWireColor(index, undefined)}>Auto</button
        >
        {#each COLORS as c (c)}
          <button
            class="swatch"
            role="radio"
            aria-checked={wire.color === c}
            aria-label={WIRE_LABEL[c]}
            title={WIRE_LABEL[c]}
            disabled={!editing}
            style:--c={WIRE_HEX[c]}
            onclick={() => bench.setWireColor(index, c)}
          ></button>
        {/each}
      </div>
    </section>
  {/if}
  {#if comp && !comp.onboard}
    {@const id = comp.id}
    <section class="insp-section">
      <label class="field">
        <span>Nom affiché</span>
        <input
          value={comp.name}
          placeholder={id}
          maxlength="40"
          disabled={!editing}
          onkeydown={(e) => commitOnKey(e, () => ((e.currentTarget as HTMLInputElement).value = comp?.name ?? ''))}
          onchange={(e) => bench.renameComponent(id, e.currentTarget.value)}
        />
        <span class="hint">Les fils gardent l'identifiant <span class="data">{id}</span>.</span>
      </label>
    </section>
  {/if}
  {#if comp && !comp.onboard && editing && paramDefs.length > 0}
    {@const id = comp.id}
    <section class="insp-section">
      <h3>Paramètres</h3>
      <div class="params">
        {#each paramDefs as [key, def] (key)}
          {@const value = comp.params[key] ?? def.default}
          <label class="field">
            <span>{def.label ?? key}{def.unit ? ` (${def.unit})` : ''}</span>
            {#if def.type === 'enum'}
              <select
                value={String(value)}
                onkeydown={(e) => e.stopPropagation()}
                onchange={(e) => {
                  const raw = e.currentTarget.value;
                  const match = (def.values ?? []).find((x) => String(x) === raw);
                  if (match !== undefined) void bench.setParam(id, key, match);
                }}
              >
                {#each def.values ?? [] as v (v)}
                  <option value={String(v)}>{VALUE_WORD[String(v)] ?? v}</option>
                {/each}
              </select>
            {:else if def.type === 'boolean'}
              <input
                type="checkbox"
                class="check"
                checked={value === true}
                onchange={(e) => void bench.setParam(id, key, e.currentTarget.checked)}
              />
            {:else if def.type === 'integer' || def.type === 'number'}
              <input
                type="number"
                class="data"
                value={String(value)}
                min={def.min}
                max={def.max}
                step={def.step ?? (def.type === 'integer' ? 1 : 'any')}
                onkeydown={(e) => commitOnKey(e, () => ((e.currentTarget as HTMLInputElement).value = String(value)))}
                onchange={(e) => {
                  const n = Number(e.currentTarget.value);
                  const ok = Number.isFinite(n) && (def.min === undefined || n >= def.min) && (def.max === undefined || n <= def.max);
                  if (ok) void bench.setParam(id, key, def.type === 'integer' ? Math.round(n) : n);
                  else e.currentTarget.value = String(value);
                }}
              />
            {:else}
              <input
                value={String(value)}
                onkeydown={(e) => commitOnKey(e, () => ((e.currentTarget as HTMLInputElement).value = String(value)))}
                onchange={(e) => void bench.setParam(id, key, e.currentTarget.value)}
              />
            {/if}
          </label>
        {/each}
      </div>
    </section>
  {/if}
  {#if note && sel?.kind === 'note'}
    {@const index = sel.index}
    <section class="insp-section">
      <label class="field">
        <span>Texte</span>
        <input
          value={note.text}
          maxlength="200"
          disabled={!editing}
          onkeydown={(e) => commitOnKey(e, () => ((e.currentTarget as HTMLInputElement).value = note?.text ?? ''))}
          onchange={(e) => bench.setNoteText(index, e.currentTarget.value)}
        />
      </label>
    </section>
  {/if}
  {#if deletable && editing && sel}
    <section class="insp-section actions">
      <Button icon="trash" onclick={() => bench.deleteSelection()}>{DELETE_LABEL[sel.kind]}</Button>
    </section>
  {/if}
{/if}

<style>
  .insp-head {
    min-width: 280px;
    padding: 12px 12px 8px;
    border-bottom: 1px solid var(--pb-line);
  }
  .insp-head h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .insp-head p {
    margin: 4px 0 0;
    color: var(--pb-text-quiet);
    font-size: 12px;
  }
  .insp-section {
    min-width: 280px;
    padding: 8px 12px 12px;
    border-bottom: 1px solid var(--pb-line);
  }
  .insp-section h3 {
    margin: 4px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--pb-text-quiet);
  }
  .field input {
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    color: var(--pb-text);
    font-size: 13px;
    user-select: text;
  }
  .field input:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
    border-color: transparent;
  }
  .field input:disabled {
    opacity: 0.6;
  }
  .field select {
    height: 28px;
    padding: 0 4px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    color: var(--pb-text);
    font-size: 13px;
  }
  .field select:focus,
  .field .check:focus-visible {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
  }
  .field input.data {
    font: 400 12px var(--pb-font-data);
    font-variant-numeric: tabular-nums;
  }
  .field .check {
    width: 16px;
    height: 16px;
    margin: 6px 0;
    accent-color: var(--pb-accent);
  }
  .params {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hint {
    font-size: 12px;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .swatch {
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--c);
    cursor: pointer;
  }
  .swatch.auto {
    width: auto;
    padding: 0 8px;
    background: transparent;
    border-left: 6px solid var(--c);
    font-size: 12px;
  }
  .swatch[aria-checked='true'] {
    outline: 2px solid var(--pb-accent);
    outline-offset: 1px;
  }
  .swatch:disabled {
    cursor: default;
    opacity: 0.6;
  }
</style>
