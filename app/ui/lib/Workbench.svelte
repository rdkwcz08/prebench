<!--
  app/ui/lib/Workbench.svelte — Paillasse : tapis ESD, carte, composants et fils (section 10.4,
  charte 2.2 et 2.4).

  Couches, de bas en haut : tapis quadrillé (24 px), carte, fils, composants, broches.
  - Déplacer la vue : molette ; Espace + glisser (mode Édition) ; bouton du milieu.
  - Zoom : Ctrl ou Cmd + molette (pincement du pavé tactile), autour du pointeur.
  - Mode Édition : glisser un composant (aimanté au pas de 24) ; tracer un fil d'une
    broche à une autre (glisser, ou cliquer puis cliquer) ; les broches compatibles
    s'allument ; Échap abandonne le fil ; Supprimer retire la sélection.
  - Mode Simulation : on interagit avec les vues (B1…), rien ne bouge.
  - Notes : posées depuis la barre d'outils ou par un clic droit sur le tapis
    (Édition) ; déplacées, modifiées (double-clic), supprimées ; visibles dans les
    deux modes.
  - Ajuster la vue : au chargement d'un projet, bouton « Ajuster la vue » (Cmd/Ctrl+0),
    double-clic sur le tapis vide ; Cmd/Ctrl+1 : 100 %.
  - Mode Édition visible sans lire la barre d'outils : cadre --pb-accent, étiquette
    « Édition », quadrillage plus marqué (charte, « Mode Édition de la paillasse »).
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import Board from './Board.svelte';
  import { frequency, percent, volts } from './format';
  import ComponentHost from './ComponentHost.svelte';
  import { untrack } from 'svelte';
  import {
    BOARD_ORIGIN,
    boardPads,
    boardSignal,
    compatible,
    defaultColor,
    endpointPos,
    GENERIC_BOARD,
    GRID,
    LEVEL_WORD,
    pinRole,
    snap,
    WIRE_HEX,
    wirePath,
    type Point,
  } from './geometry';

  const MIN_ZOOM = 0.5;
  const MAX_ZOOM = 2.5;
  /** Marge autour de la paillasse quand la vue s'ajuste. */
  const FIT_MARGIN = 48;
  /** Hauteur réservée sous un composant pour son repère. */
  const CAPTION = 32;

  let viewport: HTMLElement | undefined = $state();
  let pan = $state<Point>([0, 0]);
  let zoom = $state(1);
  let spaceDown = $state(false);
  /** Fil en cours de tracé : extrémité de départ et pointeur (monde). */
  let drawing = $state<{ from: string; to: Point; dragged: boolean } | null>(null);
  /** Note en cours de saisie (index) ; null sinon. */
  let editingNote = $state<number | null>(null);
  let gesture = $state<{ kind: 'note'; index: number; start: Point; pos0: Point; before: ReturnType<typeof bench.begin>; moved: boolean } | { kind: 'pan'; start: Point; pan0: Point } | { kind: 'move'; id: string; start: Point; pos0: Point; before: ReturnType<typeof bench.begin>; moved: boolean } | null>(null);

  const project = $derived(bench.project);
  const pads = $derived(project ? boardPads(project) : []);
  const placed = $derived(bench.instances.map((i) => ({ id: i.id, type: i.type, pos: i.pos })));
  const editing = $derived(bench.mode === 'edit');
  /** Broches des composants placés (les composants intégrés sont câblés par la carte). */
  const compPins = $derived(
    bench.instances
      .filter((i) => !i.onboard)
      .flatMap((i) =>
        (i.info?.manifest.pins ?? []).map((p) => ({
          endpoint: `${i.id}.${p.name}`,
          id: i.id,
          role: p.role,
          label: p.label ?? p.name,
          pos: [i.pos[0] + p.x, i.pos[1] + p.y] as Point,
        })),
      ),
  );
  const wires = $derived(
    project
      ? bench.wires.map((w, index) => {
          const a = endpointPos(project, pads, placed, w.from);
          const b = endpointPos(project, pads, placed, w.to);
          const color = w.color ?? defaultColor(project, pads, placed, w);
          return { index, a, b, color, ground: color === 'black' };
        })
      : [],
  );
  const selectedPin = $derived(bench.selection?.kind === 'pin' ? bench.selection.endpoint : null);

  /** Une extrémité peut-elle recevoir le fil en cours ? */
  function accepts(endpoint: string): boolean {
    if (drawing === null || project === null || endpoint === drawing.from) return false;
    const fromBoard = drawing.from.startsWith('board.');
    const toBoard = endpoint.startsWith('board.');
    if (fromBoard && toBoard) return false;
    if (!fromBoard && !toBoard) return endpoint.split('.')[0] !== drawing.from.split('.')[0];
    const boardEnd = fromBoard ? drawing.from : endpoint;
    const compEnd = fromBoard ? endpoint : drawing.from;
    const signal = boardSignal(project, pads, boardEnd);
    const role = pinRole(project, placed, compEnd);
    return signal !== null && role !== null && compatible(role, signal, project.pins, project.power);
  }

  function toWorld(e: { clientX: number; clientY: number }): Point {
    const r = viewport?.getBoundingClientRect();
    if (!r) return [0, 0];
    return [(e.clientX - r.left - pan[0]) / zoom, (e.clientY - r.top - pan[1]) / zoom];
  }

  function endpointAt(target: EventTarget | null): string | null {
    return (target as Element | null)?.closest<SVGElement>('[data-endpoint]')?.dataset.endpoint ?? null;
  }

  function onpointerdown(e: PointerEvent): void {
    const target = e.target as Element;
    if (e.button === 1 || (e.button === 0 && spaceDown && editing)) {
      gesture = { kind: 'pan', start: [e.clientX, e.clientY], pan0: [pan[0], pan[1]] };
      viewport?.setPointerCapture(e.pointerId);
      e.preventDefault();
      return;
    }
    if (e.button !== 0) return;
    const endpoint = endpointAt(target);
    if (endpoint !== null) {
      bench.selection = { kind: 'pin', endpoint };
      if (!editing) return;
      if (drawing !== null) {
        if (accepts(endpoint)) void bench.addWire(drawing.from, endpoint);
        drawing = null;
        return;
      }
      const pad = pads.find((p) => p.endpoint === endpoint);
      if (pad && !pad.wireable) return;
      drawing = { from: endpoint, to: toWorld(e), dragged: false };
      viewport?.setPointerCapture(e.pointerId);
      return;
    }
    const noteEl = target.closest<HTMLElement>('[data-note]');
    if (noteEl) {
      if (!editing || target.closest('input')) return;
      const index = Number(noteEl.dataset.note);
      const n = bench.notes[index];
      bench.selection = { kind: 'note', index };
      drawing = null;
      if (n) {
        gesture = { kind: 'note', index, start: toWorld(e), pos0: [n.pos[0], n.pos[1]], before: bench.begin(), moved: false };
        viewport?.setPointerCapture(e.pointerId);
      }
      return;
    }
    const wireEl = target.closest<SVGElement>('[data-wire]');
    if (wireEl) {
      bench.selection = { kind: 'wire', index: Number(wireEl.dataset.wire) };
      drawing = null;
      return;
    }
    if (target.closest('.name.renamable, .rename')) return; // double-clic : renommer
    const compEl = target.closest<HTMLElement>('[data-component]');
    if (compEl && editing) {
      const id = compEl.dataset.component ?? '';
      bench.selection = { kind: 'component', id };
      drawing = null;
      const inst = bench.instances.find((i) => i.id === id);
      if (inst && !inst.onboard) {
        gesture = { kind: 'move', id, start: toWorld(e), pos0: [inst.pos[0], inst.pos[1]], before: bench.begin(), moved: false };
        viewport?.setPointerCapture(e.pointerId);
      }
      return;
    }
    if (compEl) return; // simulation : la vue gère ses contrôles
    drawing = null;
    bench.selection = null;
  }

  function onpointermove(e: PointerEvent): void {
    if (gesture?.kind === 'pan') {
      pan = [gesture.pan0[0] + e.clientX - gesture.start[0], gesture.pan0[1] + e.clientY - gesture.start[1]];
    } else if (gesture?.kind === 'note') {
      const w = toWorld(e);
      const n = bench.notes[gesture.index];
      const next: Point = [snap(gesture.pos0[0] + w[0] - gesture.start[0]), snap(gesture.pos0[1] + w[1] - gesture.start[1])];
      if (n && (next[0] !== n.pos[0] || next[1] !== n.pos[1])) {
        bench.notes = bench.notes.map((x, i) => (i === (gesture as { index: number }).index ? { text: x.text, pos: next } : x));
        gesture.moved = true;
      }
    } else if (gesture?.kind === 'move') {
      const w = toWorld(e);
      const inst = bench.instances.find((i) => i.id === (gesture as { id: string }).id);
      if (inst) {
        const next: Point = [snap(gesture.pos0[0] + w[0] - gesture.start[0]), snap(gesture.pos0[1] + w[1] - gesture.start[1])];
        if (next[0] !== inst.pos[0] || next[1] !== inst.pos[1]) {
          inst.pos = next;
          gesture.moved = true;
        }
      }
    } else if (drawing) {
      const w = toWorld(e);
      if (Math.hypot(w[0] - drawing.to[0], w[1] - drawing.to[1]) > 4) drawing.dragged = true;
      drawing.to = w;
    }
  }

  function onpointerup(e: PointerEvent): void {
    if ((gesture?.kind === 'move' || gesture?.kind === 'note') && gesture.moved) void bench.commit(gesture.before);
    gesture = null;
    if (drawing?.dragged) {
      // Glisser-déposer : fil posé si on relâche sur une broche compatible.
      const under = document.elementFromPoint(e.clientX, e.clientY);
      const endpoint = endpointAt(under);
      if (endpoint !== null && accepts(endpoint)) void bench.addWire(drawing.from, endpoint);
      drawing = null;
    }
  }

  function onwheel(e: WheelEvent): void {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const r = viewport?.getBoundingClientRect();
      if (!r) return;
      const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom * Math.exp(-e.deltaY * 0.01)));
      const cx = e.clientX - r.left;
      const cy = e.clientY - r.top;
      pan = [cx - ((cx - pan[0]) * next) / zoom, cy - ((cy - pan[1]) * next) / zoom];
      zoom = next;
    } else {
      pan = [pan[0] - e.deltaX, pan[1] - e.deltaY];
    }
  }

  function typing(e: KeyboardEvent): boolean {
    return (e.target as Element | null)?.closest('input, select, textarea, [contenteditable]') !== null;
  }

  function onkeydown(e: KeyboardEvent): void {
    if (typing(e)) return;
    if (e.key === ' ' && editing) {
      spaceDown = true; // déplacement de la vue ; la pause n'agit qu'en mode Simulation
    } else if (e.key === 'Escape') {
      if (drawing) drawing = null;
      else bench.selection = null;
    } else if ((e.key === 'Delete' || e.key === 'Backspace') && editing) {
      e.preventDefault();
      void bench.deleteSelection();
    }
  }

  function onkeyup(e: KeyboardEvent): void {
    if (e.key === ' ') spaceDown = false;
  }

  /** Rectangle monde qui contient la carte et les composants, repères compris. */
  function contentBox(): [number, number, number, number] | null {
    if (project === null) return null;
    const size = project.layout?.size ?? GENERIC_BOARD;
    let [x0, y0] = BOARD_ORIGIN;
    let [x1, y1] = [BOARD_ORIGIN[0] + size[0], BOARD_ORIGIN[1] + size[1]];
    for (const inst of bench.instances) {
      const v = inst.info?.manifest.view ?? { width: 40, height: 40 };
      x0 = Math.min(x0, inst.pos[0]);
      y0 = Math.min(y0, inst.pos[1]);
      x1 = Math.max(x1, inst.pos[0] + v.width);
      y1 = Math.max(y1, inst.pos[1] + v.height + CAPTION);
    }
    return [x0, y0, x1, y1];
  }

  /** Ajuste la vue à la carte et aux composants, avec une marge (au plus 100 %… 150 %). */
  export function fit(): void {
    const r = viewport?.getBoundingClientRect();
    const box = contentBox();
    if (!r || !box || r.width === 0 || r.height === 0) return;
    const [x0, y0, x1, y1] = box;
    const z = Math.min(
      1.5,
      Math.max(MIN_ZOOM, Math.min((r.width - 2 * FIT_MARGIN) / (x1 - x0), (r.height - 2 * FIT_MARGIN) / (y1 - y0))),
    );
    zoom = z;
    pan = [(r.width - (x1 - x0) * z) / 2 - x0 * z, (r.height - (y1 - y0) * z) / 2 - y0 * z];
  }

  /** Zoom 100 %, autour du centre de la vue. */
  export function actualSize(): void {
    const r = viewport?.getBoundingClientRect();
    if (!r) return;
    const cx = r.width / 2;
    const cy = r.height / 2;
    pan = [cx - (cx - pan[0]) / zoom, cy - (cy - pan[1]) / zoom];
    zoom = 1;
  }

  // Projet ouvert : la vue s'ajuste une fois, quand la paillasse a sa taille.
  let fittedPath = '';
  $effect(() => {
    const path = project?.path ?? '';
    if (path === '' || path === fittedPath || viewport === undefined) return;
    fittedPath = path;
    untrack(() => requestAnimationFrame(() => fit()));
  });

  function ondblclick(e: MouseEvent): void {
    // Élément sous le pointeur : la capture d'un déplacement détourne la cible.
    const t = document.elementFromPoint(e.clientX, e.clientY) ?? (e.target as Element);
    if (t.closest('.name, .rename, .note-edit')) return;
    const noteEl = t.closest<HTMLElement>('[data-note]');
    if (noteEl) {
      if (editing) editingNote = Number(noteEl.dataset.note);
      return;
    }
    if (t.closest('[data-endpoint], [data-wire], [data-component], .board')) return;
    fit();
  }

  /** Clic droit sur le tapis vide, en Édition : une note à cet endroit. */
  function oncontextmenu(e: MouseEvent): void {
    if (!editing || !project) return;
    const t = e.target as Element;
    if (t.closest('[data-endpoint], [data-wire], [data-component], [data-note], .board')) return;
    e.preventDefault();
    const w = toWorld(e);
    void addNote([snap(w[0]), snap(w[1])]);
  }

  async function addNote(pos: Point): Promise<void> {
    editingNote = await bench.addNote(pos);
  }

  /** Note au centre de la vue (bouton « Ajouter une note » de la barre d'outils). */
  export function addNoteAtCenter(): void {
    bench.mode = 'edit';
    void addNote(center());
  }

  function finishNote(index: number, input: HTMLInputElement): void {
    if (editingNote !== index) return;
    editingNote = null;
    void bench.setNoteText(index, input.value);
  }

  function noteKey(e: KeyboardEvent, index: number): void {
    e.stopPropagation(); // Supprimer, E, Espace : touches du texte
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter') finishNote(index, input);
    if (e.key === 'Escape') editingNote = null;
  }

  /** Place le curseur dans le champ d'une note à sa création. */
  function focusOnMount(el: HTMLInputElement): void {
    el.focus();
    el.select();
  }

  /** Centre de la vue, en coordonnées monde, aimanté : position d'un nouveau composant. */
  export function center(): Point {
    const r = viewport?.getBoundingClientRect();
    if (!r) return [GRID * 20, GRID * 4];
    return [snap((r.width / 2 - pan[0]) / zoom), snap((r.height / 3 - pan[1]) / zoom)];
  }

  const drawingFrom = $derived(drawing && project ? endpointPos(project, pads, placed, drawing.from) : null);
</script>

<svelte:window {onkeydown} {onkeyup} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main
  class="bench"
  class:editing
  class:panning={spaceDown || gesture?.kind === 'pan'}
  aria-label="Paillasse"
  bind:this={viewport}
  style:background-size="{GRID * zoom}px {GRID * zoom}px"
  style:background-position="{pan[0]}px {pan[1]}px"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  {onwheel}
  {ondblclick}
  {oncontextmenu}
>
  {#if editing}
    <span class="mode-label" aria-hidden="true">Édition</span>
  {/if}
  {#if project}
    <div class="world" style:transform="translate({pan[0]}px, {pan[1]}px) scale({zoom})">
      <Board name={project.board_name} mcu={project.mcu} layout={project.layout} {pads} />

      <svg class="layer wires" aria-label="Fils">
        {#each wires as w (w.index)}
          {#if w.a && w.b}
            {@const d = wirePath(w.a, w.b)}
            {@const sel = bench.selection?.kind === 'wire' && bench.selection.index === w.index}
            <g class="wire" data-wire={w.index} style:--wire={WIRE_HEX[w.color]}>
              <path {d} class="hit" />
              {#if sel}<path {d} class="selected" />{/if}
              <path {d} class="outline" class:ground={w.ground} />
              <path {d} class="core" />
              {#each [w.a, w.b] as end, i (i)}
                <rect x={end[0] - 5} y={end[1] - 5} width="10" height="10" rx="1" class="tip" />
              {/each}
            </g>
          {/if}
        {/each}
      </svg>

      {#each bench.notes as n, i (i)}
        <div
          class="note"
          class:movable={editing}
          class:selected={bench.selection?.kind === 'note' && bench.selection.index === i}
          data-note={i}
          style:left="{n.pos[0]}px"
          style:top="{n.pos[1]}px"
        >
          {#if editingNote === i}
            <input
              class="note-edit"
              value={n.text}
              maxlength="200"
              aria-label="Texte de la note"
              use:focusOnMount
              onkeydown={(e) => noteKey(e, i)}
              onblur={(e) => finishNote(i, e.currentTarget)}
            />
          {:else}
            {n.text}
          {/if}
        </div>
      {/each}

      {#each bench.instances as inst (inst.id)}
        <ComponentHost
          {inst}
          mode={bench.mode}
          selected={bench.selection?.kind === 'component' && bench.selection.id === inst.id}
          pending={bench.pending.includes(inst.id)}
          onrename={(name) => bench.renameComponent(inst.id, name)}
        />
      {/each}

      <svg class="layer pins" aria-label="Broches">
        <defs>
          <pattern id="net-floating" width="3" height="3" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="3" height="3" class="hatch-bg" />
            <line x1="0" y1="0" x2="0" y2="3" class="hatch-line" />
          </pattern>
        </defs>
        <!-- États des nets pendant la simulation (charte 2.3) : couleur + forme, mot au survol -->
        {#each pads as p (p.endpoint)}
          {@const level = bench.nets[p.signal]}
          {@const pwm = bench.pins[p.signal]?.pwm}
          {@const adc = bench.pins[p.signal]?.adc}
          {#if adc && p.wireable}
            <!-- Entrée ADC : jauge verticale de la tension, du bas vers le haut -->
            {@const h = adc.max > 0 ? (9 * adc.raw) / adc.max : 0}
            <g class="net analog" aria-hidden="true">
              <rect x={p.pos[0] - 4.5} y={p.pos[1] - 4.5} width="9" height="9" rx="1" />
              <rect x={p.pos[0] - 4.5} y={p.pos[1] + 4.5 - h} width="9" height={h} class="duty" />
            </g>
          {:else if pwm && p.wireable && (level === 'high' || level === 'low')}
            <!-- PWM : jauge du rapport cyclique, part haute à gauche (forme + mot au survol) -->
            <g class="net pwm" aria-hidden="true">
              <rect x={p.pos[0] - 4.5} y={p.pos[1] - 4.5} width="9" height="9" rx="1" />
              <rect x={p.pos[0] - 4.5} y={p.pos[1] - 4.5} width={9 * Math.min(1, Math.max(0, pwm.duty))} height="9" class="duty" />
            </g>
          {:else if level && p.wireable}
            <g class="net {level}" aria-hidden="true">
              <rect x={p.pos[0] - 4.5} y={p.pos[1] - 4.5} width="9" height="9" rx="1" />
              {#if level === 'conflict'}
                <path d="M{p.pos[0] - 2.5} {p.pos[1] - 2.5}l5 5M{p.pos[0] + 2.5} {p.pos[1] - 2.5}l-5 5" class="cross" />
              {/if}
            </g>
          {/if}
        {/each}
        {#each pads as p (p.endpoint)}
          <rect
            x={p.pos[0] - 6}
            y={p.pos[1] - 6}
            width="12"
            height="12"
            rx="2"
            class="pin"
            class:off={!p.wireable}
            class:ok={drawing !== null && accepts(p.endpoint)}
            class:dim={drawing !== null && !accepts(p.endpoint) && p.endpoint !== drawing.from}
            class:chosen={selectedPin === p.endpoint || drawing?.from === p.endpoint}
            data-endpoint={p.endpoint}
          >
            <title
              >{p.label}{p.label !== p.signal ? ` (${p.signal})` : ''}, {p.header}:{p.index}{bench.pins[p.signal]?.pwm
                ? ` : PWM ${frequency(bench.pins[p.signal]!.pwm!.freq_hz)}, ${percent(bench.pins[p.signal]!.pwm!.duty)}`
                : bench.pins[p.signal]?.adc
                  ? ` : ADC ${volts(bench.pins[p.signal]!.adc!.volts)}`
                  : bench.nets[p.signal]
                    ? ` : ${LEVEL_WORD[bench.nets[p.signal]!].toLowerCase()}`
                    : ''}</title
            >
          </rect>
        {/each}
        {#each compPins as p (p.endpoint)}
          <circle
            cx={p.pos[0]}
            cy={p.pos[1]}
            r="5"
            class="pin comp"
            class:shown={editing}
            class:ok={drawing !== null && accepts(p.endpoint)}
            class:dim={drawing !== null && !accepts(p.endpoint) && p.endpoint !== drawing.from}
            class:chosen={selectedPin === p.endpoint || drawing?.from === p.endpoint}
            data-endpoint={p.endpoint}
          >
            <title>{p.id}.{p.label}</title>
          </circle>
        {/each}
        {#if drawing && drawingFrom}
          <path d={wirePath(drawingFrom, drawing.to)} class="preview" />
        {/if}
      </svg>
    </div>
  {/if}
</main>

<style>
  .bench {
    grid-area: bench;
    position: relative;
    overflow: hidden;
    touch-action: none;
    background-color: var(--pb-mat);
    background-image:
      linear-gradient(var(--pb-mat-grid) 1px, transparent 1px),
      linear-gradient(90deg, var(--pb-mat-grid) 1px, transparent 1px);
  }
  .panning {
    cursor: grab;
  }
  /* Mode Édition : cadre et étiquette d'accent, quadrillage plus marqué ; le tapis
     garde sa couleur (charte, « Mode Édition de la paillasse »). */
  .bench.editing {
    background-image:
      linear-gradient(color-mix(in srgb, var(--pb-mat-grid) 78%, var(--pb-mat-ink)) 1px, transparent 1px),
      linear-gradient(90deg, color-mix(in srgb, var(--pb-mat-grid) 78%, var(--pb-mat-ink)) 1px, transparent 1px);
  }
  .bench.editing::after {
    content: '';
    position: absolute;
    inset: 0;
    border: 2px solid var(--pb-accent);
    pointer-events: none;
    z-index: 2;
  }
  .mode-label {
    position: absolute;
    left: 8px;
    top: 8px;
    z-index: 2;
    padding: 0 8px;
    border-radius: 3px;
    background: var(--pb-accent);
    color: var(--pb-on-accent);
    font-size: 12px;
    font-weight: 600;
    line-height: 20px;
    pointer-events: none;
  }
  .world {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    transform-origin: 0 0;
  }
  /* Note sur le tapis (charte, « Note sur la paillasse ») : texte sobre, filet à gauche */
  .note {
    position: absolute;
    max-width: 240px;
    padding: 2px 8px;
    border-left: 2px solid color-mix(in srgb, var(--pb-mat-ink) 50%, transparent);
    color: var(--pb-mat-ink);
    font-size: 13px;
    font-weight: 600;
    line-height: 20px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: none;
  }
  .note.movable {
    cursor: grab;
  }
  .note.selected {
    outline: 2px solid var(--pb-accent);
    outline-offset: 2px;
    border-radius: 3px;
  }
  .note-edit {
    width: 220px;
    height: 24px;
    padding: 0 4px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-panel);
    color: var(--pb-text);
    font: inherit;
    user-select: text;
  }
  .note-edit:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
  }
  .layer {
    position: absolute;
    left: 0;
    top: 0;
    width: 1px;
    height: 1px;
    overflow: visible;
  }
  .wires {
    pointer-events: none;
  }
  .wire {
    pointer-events: auto;
    cursor: pointer;
  }
  .wire path {
    fill: none;
    stroke-linecap: round;
  }
  .hit {
    stroke: transparent;
    stroke-width: 14;
  }
  .selected {
    stroke: var(--pb-accent);
    stroke-width: 10;
  }
  /* Fil Dupont : contour sombre puis âme colorée (charte 2.4) */
  .outline {
    stroke: color-mix(in srgb, var(--wire) 60%, #000000);
    stroke-width: 6;
  }
  .outline.ground {
    stroke: var(--pb-gnd-outline);
  }
  .core {
    stroke: var(--wire);
    stroke-width: 4;
  }
  .tip {
    fill: #1a1c1e;
  }
  .pins {
    pointer-events: none;
  }
  .pin {
    pointer-events: auto;
    fill: transparent;
    stroke: transparent;
    stroke-width: 2;
    cursor: crosshair;
  }
  .pin.off {
    cursor: not-allowed;
  }
  .bench:not(.editing) .pin {
    cursor: pointer;
  }
  .pin.comp {
    pointer-events: none;
  }
  .pin.comp.shown {
    pointer-events: auto;
    fill: color-mix(in srgb, var(--pb-copper) 70%, transparent);
  }
  /* Tracé en cours : broches compatibles allumées, les autres atténuées */
  .pin.ok {
    fill: color-mix(in srgb, var(--pb-accent) 35%, transparent);
    stroke: var(--pb-accent);
    stroke-width: 1;
  }
  .pin.dim {
    opacity: 0.3;
  }
  .pin.chosen {
    stroke: var(--pb-accent);
    stroke-width: 2;
  }
  /* États des nets (charte 2.3) : haut et bas pleins, tirés en contour, flottant
     hachuré, conflit rouge et croix */
  .net rect {
    stroke-width: 1.5;
  }
  .net.high rect {
    fill: var(--pb-high);
    stroke: var(--pb-high);
  }
  .net.low rect {
    fill: var(--pb-low);
    stroke: var(--pb-low);
  }
  .net.pulled-high rect {
    fill: var(--pb-board);
    stroke: var(--pb-high);
    stroke-width: 2;
  }
  .net.pulled-low rect {
    fill: var(--pb-board);
    stroke: var(--pb-low);
    stroke-width: 2;
  }
  .net.floating rect {
    fill: url(#net-floating);
    stroke: var(--pb-text-quiet);
  }
  .hatch-bg {
    fill: var(--pb-board);
  }
  .hatch-line {
    stroke: var(--pb-silk);
    stroke-width: 1.2;
  }
  .net.pwm rect,
  .net.analog rect {
    fill: var(--pb-low);
    stroke: var(--pb-high);
  }
  .net.pwm .duty,
  .net.analog .duty {
    fill: var(--pb-high);
    stroke: none;
  }
  .net.conflict rect {
    fill: var(--pb-conflict);
    stroke: var(--pb-conflict);
  }
  .cross {
    stroke: var(--pb-silk);
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .preview {
    fill: none;
    stroke: var(--pb-accent);
    stroke-width: 3;
    stroke-dasharray: 6 4;
    stroke-linecap: round;
    pointer-events: none;
  }
</style>
