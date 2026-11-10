// components/echo/model.ts — Composant d'écho pour la mesure de latence (section 8.1).
//
// onPing ne fait rien : son retour suffit à déclencher la réponse PONG. Le
// compteur n'est redessiné qu'une fois par image (invalidate est coalescé).

import type { ComponentFactory } from '../component-api';

export type EchoParams = Record<string, never>;

export type EchoState = {
  pings: number;
};

const createEcho: ComponentFactory<EchoParams, EchoState> = (ctx) => {
  let pings = 0;
  return {
    onPing(): void {
      pings += 1;
      ctx.invalidate();
    },
    getState(): EchoState {
      return { pings };
    },
  };
};

export default createEcho;
