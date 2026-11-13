// app/tests-ui/data.ts — Données de test partagées : build en échec (onglet Build, captures).

import { PROJECT_PATH } from './fixture';

export const MAIN_C = `${PROJECT_PATH}/Core/Src/main.c`;

export const FAILED_BUILD = {
  ok: false,
  duration_ms: 1840,
  errors: 2,
  warnings: 1,
  diagnostics: [
    {
      severity: 'error',
      file: MAIN_C,
      display: 'Core/Src/main.c',
      line: 34,
      column: 30,
      message: "expected ';' after expression",
    },
    {
      severity: 'warning',
      file: MAIN_C,
      display: 'Core/Src/main.c',
      line: 21,
      column: 12,
      message: "unused variable 'count' [-Wunused-variable]",
    },
    { severity: 'error', file: null, display: null, line: null, column: null, message: "undefined reference to `HAL_Foo'" },
  ],
};

