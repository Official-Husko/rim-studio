import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { cloneReference } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
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

test.describe('1024 by 700 window', () => {
  test('setup, project, weapons and patches fit the window', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/setup?scan=run');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByText('Mods found').first()).toBeVisible({ timeout: 90_000 });
    await noSidewaysScroll(page);
    await shot('w1024-setup');

    await rememberProject(page, join(env.tmp, 'work', 'QA Test Mod'));
    await page.reload();
    await gotoRoute(page, env, '/project');
    await expect(page.getByRole('tree')).toBeVisible();
    await noSidewaysScroll(page);
    await shot('w1024-project');

    await page.getByRole('link', { name: 'Weapons' }).click();
    await cloneReference(page, 'bolt', 'Gun_QaNarrow');
    await expect(page.getByRole('button', { name: /^Preview Defs\// }).first()).toBeVisible({ timeout: 30_000 });
    await noSidewaysScroll(page);
    // below the wide layout the editor stacks above the output and keeps its height
    const editor = await page.getByRole('region', { name: 'Editor' }).boundingBox();
    expect(editor?.height ?? 0).toBeGreaterThan(300);
    await shot('w1024-weapons');

    await page.getByRole('link', { name: 'Patches' }).click();
    await expect(page.getByRole('heading', { level: 1, name: 'Patches' })).toBeVisible();
    await page.waitForTimeout(3000);
    await noSidewaysScroll(page);
    await shot('w1024-patches');
  });
});
