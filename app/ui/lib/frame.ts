// app/ui/lib/frame.ts — Une seule image d'affichage pour toute l'interface (section 11.3).
//
// États, nets et broches du cœur, octets du moniteur série, zones des écrans :
// chaque source dépose son travail ici, et tout est appliqué dans la même
// image, à la cadence d'affichage (réglage frame_rate). Sans cela, chaque
// source programmait sa propre image (requestAnimationFrame) : les octets
// série et les écrans, qui n'arrivent pas en même temps que l'image du cœur,
// faisaient dessiner WebKit deux à trois fois par période (mesuré dans l'app :
// jusqu'à 50 demandes d'image par seconde pour 15 images du cœur).
//
// Une seule demande d'image par image affichée : trop tôt pour la cadence,
// l'attente se fait d'abord par minuterie, sans image WebKit intermédiaire.
// Rien à faire : aucune demande (paillasse immobile, tests/ui frame.spec).

const tasks = new Set<() => void>();
let requested = false;
let last = -Infinity;
let cadence = (): number => 15;

/** Cadence d'affichage en images par seconde (réglage, batterie). */
export function setCadence(fps: () => number): void {
  cadence = fps;
}

function run(): void {
  last = performance.now();
  requested = false;
  const list = [...tasks];
  tasks.clear();
  for (const task of list) task();
}

/**
 * Travail à faire à la prochaine image d'affichage. Une même fonction
 * déposée plusieurs fois avant l'image n'est exécutée qu'une fois.
 */
export function onNextFrame(task: () => void): void {
  tasks.add(task);
  if (requested) return;
  requested = true;
  // 1 ms de marge : l'image WebKit arrive au plus tôt après la demande.
  const wait = 1000 / cadence() - 1 - (performance.now() - last);
  if (wait > 0) setTimeout(() => requestAnimationFrame(run), wait);
  else requestAnimationFrame(run);
}
