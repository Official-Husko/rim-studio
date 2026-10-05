import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehrWithPatch } from '../support/mods.ts';
import { rememberProject } from '../support/session.ts';

test.describe.configure({ mode: 'serial' });
test.use({ viewport: { width: 1024, height: 700 } });

/** The page itself must not scroll sideways at the smallest supported window. */
async function noSidewaysScroll(page: Page): Promise<void> {
  const overflow = await page.evaluate(() => {
    const root = document.documentElement;
    return { scroll: root.scrollWidth, client: root.clientWidth };
  });
  expect(overflow.scroll).toBeLessThanOrEqual(overflow.client);
}

test.describe('1024 by 700 window, the new views', () => {
  test('setup facts, the patches table and the lint tab fit the window', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/setup?scan=run');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByText('Mods found').first()).toBeVisible({ timeout: 90_000 });
    await expect(page.getByRole('table', { name: 'Duplicate package ids' })).toBeVisible();
    await page.getByRole('table', { name: 'Duplicate package ids' }).scrollIntoViewIfNeeded();
    await noSidewaysScroll(page);
    await shot('w1024-setup-duplicates');

    const mod = copyGewehrWithPatch(join(env.tmp, 'work-narrow-data'));
    // a fresh load reads the remembered project; a hash change inside the loaded app does not
    await page.goto('about:blank');
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/patches');
    const table = page.getByRole('grid', { name: 'Weapons of the mod' });
    await expect(table.getByText('OH_G41m').first()).toBeVisible({ timeout: 60_000 });
    await noSidewaysScroll(page);
    await shot('w1024-patches-table');
    await page.getByRole('radio', { name: 'Lint' }).click();
    await expect(page.getByText(/^Files checked: 1\./)).toBeVisible({ timeout: 90_000 });
    await noSidewaysScroll(page);
    await shot('w1024-patches-lint');
  });
});
