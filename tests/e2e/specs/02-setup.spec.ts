import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { REAL } from '../support/env.ts';
import { pickFolderPath } from '../support/ui.ts';

test.describe.configure({ mode: 'serial' });

test.describe('setup flow with the real install', () => {
  test('first run: detect the game and Steam', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/setup');
    await expect(page.getByRole('heading', { name: 'Setup', level: 1 })).toBeVisible();
    await shot('setup-first-run');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByLabel('Detection report').getByText('Install folder')).toBeVisible();
    await expect(page.getByText(/common\/RimWorld/).first()).toBeVisible();
    await shot('setup-detected');
  });

  test('add the owner custom folder and scan it', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/setup');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByLabel('Detection report').getByText('Install folder')).toBeVisible();
    await expect(page.getByText(/common\/RimWorld/).first()).toBeVisible();
    await page.getByRole('button', { name: 'Add folder' }).click();
    await pickFolderPath(page, REAL.customDir);
    const dialog = page.getByRole('dialog', { name: 'Add a mod folder' });
    await expect(dialog.getByText(/mods? found/)).toBeVisible();
    await shot('setup-add-probe');
    await dialog.getByRole('button', { name: 'Add folder' }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('list', { name: 'Mod folders' }).getByText(REAL.customDir).first()).toBeVisible();

    const started = Date.now();
    await page.getByRole('button', { name: 'Scan', exact: true }).click();
    await expect(page.getByRole('group', { name: 'Scan counts' }).or(page.getByText('Mods found'))).toBeVisible({ timeout: 90_000 });
    await expect(page.getByText('Mods found').first()).toBeVisible();
    process.stdout.write(`scan took ${Date.now() - started} ms\n`);
    await shot('setup-scanned');
  });
});
