// app/tests-ui/pwm-adc.spec.ts — PWM et ADC (phase 2, étape 8).
//
// Inspecteur : fréquence et rapport cyclique d'une sortie PWM, tension et
// valeur brute d'une entrée ADC, dans la section État (broche et composant) ;
// pastilles en jauge ; paramètres modifiables en Édition (LED RGB : anode ou
// cathode commune), hors simulation de l'historique ; potentiomètre au clavier ;
// fréquence affichée par le buzzer.

import { demoProject, expect, test } from './fixture';

test('broche PWM : fonction du timer, fréquence et rapport cyclique, pastille en jauge', async ({ bench }) => {
  const { page } = bench;
  const inspector = page.getByRole('complementary', { name: 'Inspecteur' });
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:pins', {
    PA5: { mode: 3, pull: 0, function: 'pwm:TIM2_CH1', period_us: null, pwm: { freq_hz: 20000, duty: 0.5 }, adc: null },
  });
  await bench.emit('sim:nets', { PA5: 'high' });

  await expect(page.locator('.net.pwm')).toHaveCount(1);
  await page.locator('[data-endpoint="board.CN5:6"]').click();
  await expect(inspector.getByRole('heading', { level: 2 })).toHaveText('Broche PA5');
  await expect(inspector).toContainText('Alternative push-pull');
  await expect(inspector.locator('dd.data', { hasText: 'PWM TIM2_CH1' })).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: '20 kHz' })).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: /^50 %$/ })).toBeVisible();
  await expect(inspector).toContainText('Rapport cyclique');
  await expect(inspector).not.toContainText('Période mesurée');
  // LD2 (même net) : la mesure suit dans l'inspecteur du composant.
  await page.getByRole('button', { name: 'Édition', exact: true }).click();
  await page.locator('[data-component="LD2"]').click();
  await expect(inspector.locator('dd.data', { hasText: '20 kHz' })).toBeVisible();
});

test('entrée ADC : tension et valeur brute, sans niveau logique, potentiomètre relié', async ({ bench }) => {
  const { page } = bench;
  const inspector = page.getByRole('complementary', { name: 'Inspecteur' });
  const project = demoProject() as Record<string, unknown>;
  await bench.respond('open_project', { ...project, wires: [{ from: 'potentiometer.W', to: 'board.CN8:1' }] });
  await bench.emit('project:changed', { project: true, views: [], models: [] });
  await expect(page.locator('[data-wire="0"]')).toHaveCount(1);
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:states', { potentiometer: { position: 0.5, volts: 1.65 } });
  await bench.emit('sim:pins', {
    PA0: { mode: 5, pull: 0, function: null, period_us: null, pwm: null, adc: { volts: 1.65, raw: 2048, max: 4095 } },
  });
  await bench.emit('sim:nets', { PA0: 'floating' });

  await expect(page.locator('.net.analog')).toHaveCount(1);
  await page.locator('[data-endpoint="board.CN8:1"]').click();
  await expect(inspector.getByRole('heading', { level: 2 })).toHaveText('Broche PA0');
  await expect(inspector.locator('dd.data', { hasText: 'ADC1 CH0' })).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: '1,65 V' })).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: '2 048 / 4 095' })).toBeVisible();
  await expect(inspector).not.toContainText('Flottant');

  await page.getByRole('button', { name: 'Édition', exact: true }).click();
  await page.locator('[data-component="potentiometer"]').click();
  await expect(inspector).toContainText('Valeur ADC (A0)');
  await expect(inspector).toContainText('Tension du curseur');
  await expect(inspector.locator('dd.data', { hasText: /^50 %$/ })).toBeVisible(); // position
});

test('paramètres en Édition : LED RGB en anode commune, annulable', async ({ bench }) => {
  const { page } = bench;
  const inspector = page.getByRole('complementary', { name: 'Inspecteur' });
  const saved = async (): Promise<unknown> => {
    const saves = (await bench.calls('save_bench')) as { components: { id: string; params: Record<string, unknown> }[] }[];
    return saves.at(-1)?.components.find((c) => c.id === 'led-rgb')?.params.common;
  };
  await page.getByRole('button', { name: 'Édition', exact: true }).click();
  await page.locator('[data-component="led-rgb"]').click();
  await expect(inspector.getByRole('heading', { name: 'Paramètres' })).toBeVisible();
  const common = inspector.getByLabel('Commun');
  await expect(common).toHaveValue('cathode');
  await common.selectOption({ label: 'Anode commune' });
  await expect.poll(saved).toBe('anode');
  await expect(inspector.locator('dd', { hasText: 'Anode commune' })).toBeVisible();

  await page.locator('main').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(saved).toBe('cathode');

  // Hors Édition : pas de champs de paramètres.
  await page.getByRole('button', { name: 'Simulation', exact: true }).click();
  await page.locator('[data-component="led-rgb"]').click();
  await expect(inspector.getByRole('heading', { name: 'Paramètres' })).toHaveCount(0);
});

test('potentiomètre au clavier, buzzer qui affiche sa fréquence', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:states', {
    potentiometer: { position: 0.5, volts: 1.65 },
    buzzer: { sounding: true, freq_hz: 2300 },
  });
  const knob = page.getByRole('slider', { name: 'Position du potentiomètre' });
  await knob.focus();
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('End');
  await expect
    .poll(() => bench.calls('component_control'))
    .toEqual([
      { component: 'potentiometer', control: 'position', value: 0.51 },
      { component: 'potentiometer', control: 'position', value: 0.4 },
      { component: 'potentiometer', control: 'position', value: 1 },
    ]);
  await expect(bench.figure('buzzer').locator('text.freq')).toHaveText('2,3 kHz');
  await bench.emit('sim:states', { buzzer: { sounding: false, freq_hz: null } });
  await expect(bench.figure('buzzer').locator('text.freq')).toHaveCount(0);
});
