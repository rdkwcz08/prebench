// components/echo/model.ts — Composant d'écho : latence (section 8.1) et esclave SPI en boucle.
//
// onPing ne fait rien : son retour suffit à déclencher la réponse PONG.
// onSPI renvoie les octets reçus (SDI relié à SDO) : le firmware relit ce
// qu'il a émis, dans la même transaction. Les compteurs ne sont redessinés
// qu'une fois par image (invalidate est coalescé).

import type { ComponentFactory, SPIRequest } from '../component-api';

export type EchoParams = Record<string, never>;

export type EchoState = {
  pings: number;
  /** Octets reçus en SPI (émis par le firmware, lectures seules comprises). */
  spiBytes: number;
};

const createEcho: ComponentFactory<EchoParams, EchoState> = (ctx) => {
  let pings = 0;
  let spiBytes = 0;
  return {
    onPing(): void {
      pings += 1;
      ctx.invalidate();
    },
    onSPI(request: SPIRequest): Uint8Array | void {
      spiBytes += request.tx.length;
      ctx.invalidate();
      return request.wantsRx ? request.tx : undefined;
    },
    getState(): EchoState {
      return { pings, spiBytes };
    },
  };
};

export default createEcho;
