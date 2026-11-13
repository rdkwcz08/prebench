// app/tests-ui/serial.spec.ts — Moniteur série et traceur (phase 2, étape 6).

import { expect, test } from './fixture';

const bytes = (s: string) => Array.from(new TextEncoder().encode(s));

test('moniteur série : sortie, horodatage, hexa, envoi avec fin de ligne, effacer', async ({ bench }) => {
  const { page } = bench;
  await expect(page.getByRole('tab', { name: 'Série' })).toHaveAttribute('aria-selected', 'true');
  await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
  await bench.emit('sim:uart', { id: 2, data: bytes('Prebench : écho prêt\r\nPériode LD2 : 500 ms\r\n') });
  const log = page.getByLabel('Sortie série');
  await expect(log).toContainText('Prebench : écho prêt');
  await expect(log).toContainText('Période LD2 : 500 ms');
  await expect(page.getByRole('combobox', { name: 'Port série' })).toContainText('USART2, 115');

  await page.getByRole('button', { name: "Afficher l'horodatage" }).click();
  await expect(log.locator('.t').first()).toHaveText(/^\d\d:\d\d\.\d{3}$/);
  await page.getByRole('button', { name: 'Vue hexadécimale' }).click();
  await expect(log).toContainText('50 72 65 62');
  await page.getByRole('button', { name: 'Vue hexadécimale' }).click();

  await page.getByRole('combobox', { name: 'Fin de ligne' }).selectOption('crlf');
  await page.getByRole('textbox', { name: 'Texte à envoyer' }).fill('Bonjour');
  await page.getByRole('button', { name: 'Envoyer' }).click();
  await expect.poll(() => bench.calls('uart_send')).toEqual([{ id: 2, data: bytes('Bonjour\r\n') }]);

  await page.getByRole('button', { name: 'Exporter la sortie…' }).click();
  await expect.poll(async () => ((await bench.calls('export_serial')) as { name: string }[])[0]?.name).toMatch(
    /^serie-USART2-\d{8}-\d{6}\.txt$/,
  );

  await page.getByRole('button', { name: 'Effacer' }).click();
  await expect(log).toContainText('rien reçu');
});

test('alerte de débit : moniteur à 115 200, firmware à 9 600', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:uart-config', { id: 2, baud: 9600 });
  await bench.emit('sim:uart', { id: 2, data: bytes('ok\n') });
  await expect(page.getByRole('alert').filter({ hasText: 'Débit différent sur USART2' })).toBeVisible();
  await expect(page.getByRole('tab', { name: /Avertissements/ }).locator('.badge')).toHaveText('1');
  await page.getByRole('combobox', { name: 'Débit du moniteur' }).selectOption('9600');
  await expect(page.getByRole('alert').filter({ hasText: 'Débit différent' })).toHaveCount(0);
});

test('sortie standard (printf) : port distinct, sans champ d\'envoi', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:log', { source: 'stdout', level: 2, text: 'compteur = 3' });
  await expect(page.getByLabel('Sortie série')).toContainText('compteur = 3');
  await expect(page.getByRole('combobox', { name: 'Port série' })).toContainText('Sortie standard (printf)');
  await expect(page.getByRole('textbox', { name: 'Texte à envoyer' })).toHaveCount(0);
});

test('traceur : courbes depuis « temp:21.4,hum:55 », légende et dernière valeur', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
  for (let i = 0; i < 5; i++) {
    await bench.emit('sim:uart', { id: 2, data: bytes(`temp:${21 + i}.5,hum:${55 - i}\r\n`) });
  }
  await page.getByRole('tab', { name: 'Traceur' }).click();
  await expect(page.locator('polyline')).toHaveCount(2);
  const legend = page.locator('.legend');
  await expect(legend).toContainText('temp');
  await expect(legend).toContainText('25,5');
  await expect(legend).toContainText('hum');
  await expect(legend).toContainText('51');
});
