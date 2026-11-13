<!--
  app/ui/lib/SelectionInspector.svelte — Inspecteur de l'élément sélectionné sur la paillasse
  (maquette : titre, sous-titre, sections, liste de propriétés).
  - Broche de la carte : connecteur, repère, capacités, fils reliés.
  - Broche d'un composant : rôle, fils reliés.
  - Fil : extrémités, couleur (charte 2.4), suppression.
  - Composant : nom affiché (modifiable en Édition), type, position, suppression
    (sauf composant intégré).
  - Note : texte (modifiable en Édition), suppression.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import { boardPads, defaultColor, LEVEL_TONE, LEVEL_WORD, WIRE_HEX, WIRE_LABEL } from './geometry';
  import type { WireColor } from './types';
  import Button from './ui/Button.svelte';
  import PropList, { type Prop } from './ui/PropList.svelte';

  const COLORS = Object.keys(WIRE_HEX) as WireColor[];

  const project = $derived(bench.project);
  const pads = $derived(project ? boardPads(project) : []);
  const placed = $derived(bench.instances.map((i) => ({ id: i.id, type: i.type, pos: i.pos })));
  const sel = $derived(bench.selection);
  const editing = $derived(bench.mode === 'edit');

  function attached(endpoint: string): string {
    const ends = bench.wires
      .filter((w) => w.from === endpoint || w.to === endpoint)
      .map((w) => (w.from === endpoint ? w.to : w.from).replace(/^board\./, ''));
    return ends.length > 0 ? ends.join(', ') : 'Aucun';
  }

  const view = $derived.by(() => {
    if (project === null || sel === null) return null;
    if (sel.kind === 'pin' && sel.endpoint.startsWith('board.')) {
      const pad = pads.find((p) => p.endpoint === sel.endpoint);
      if (!pad) return null;
      const def = project.pins[pad.signal];
      const volts = project.power[pad.signal];
      const props: Prop[] = [
        { label: 'Connecteur', value: `${pad.header}:${pad.index}`, data: true },
        { label: 'Repère', value: pad.label, data: true },
      ];
      if (def) props.push({ label: 'Fonctions', value: def.caps.join(', '), data: true, full: def.caps.join(', ') });
      if (volts !== undefined) props.push({ label: 'Tension', value: `${volts.toLocaleString('fr-FR')} V`, data: true });
      if (!pad.wireable) props.push({ label: 'Câblage', value: 'Non câblable' });
      const level = bench.nets[pad.signal];
      const state: Prop[] = level
        ? [{ label: 'Niveau', value: LEVEL_WORD[level], tone: LEVEL_TONE[level] }]
        : [{ label: 'Niveau', value: bench.live ? 'Non utilisée' : 'Hors simulation' }];
      return {
        title: def ? `Broche ${pad.signal}` : pad.signal,
        subtitle: `Connecteur ${pad.header}, repère ${pad.label}`,
        sections: [
          { title: 'Configuration', props },
          { title: 'État', props: state },
          { title: 'Net', props: [{ label: 'Relié à', value: attached(sel.endpoint), data: true }] },
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
          { title: 'Net', props: [{ label: 'Relié à', value: attached(sel.endpoint), data: true }] },
        ],
      };
    }
    if (sel.kind === 'wire') {
      const w = bench.wires[sel.index];
      if (!w) return null;
      return {
        title: 'Fil',
        subtitle: `${w.from.replace(/^board\./, '')} vers ${w.to.replace(/^board\./, '')}`,
        sections: [
          {
            title: 'Extrémités',
            props: [
              { label: 'De', value: w.from, data: true },
              { label: 'Vers', value: w.to, data: true },
            ],
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
    return {
      title: inst.name || inst.id,
      subtitle: inst.name ? `${inst.id}, ${inst.info?.manifest.name ?? inst.type}` : (inst.info?.manifest.name ?? inst.type),
      sections: [
        {
          title: 'Composant',
          props: [
            { label: 'Type', value: inst.type, data: true },
            { label: 'Position', value: `${inst.pos[0]}, ${inst.pos[1]}`, data: true },
            ...(inst.onboard ? [{ label: 'Emplacement', value: 'Intégré à la carte' }] : []),
          ],
        },
      ],
    };
  });

  const wire = $derived(sel?.kind === 'wire' ? bench.wires[sel.index] : undefined);
  const autoColor = $derived(wire && project ? defaultColor(project, pads, placed, wire) : 'yellow');
  const comp = $derived(sel?.kind === 'component' ? bench.instances.find((i) => i.id === sel.id) : undefined);
  const note = $derived(sel?.kind === 'note' ? bench.notes[sel.index] : undefined);
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
