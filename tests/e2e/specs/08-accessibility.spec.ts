import { join } from 'node:path';
import { axeViolations, describe } from '../support/axe.ts';
import { cloneReference } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehr } from '../support/mods.ts';
import { rememberProject } from '../support/session.ts';

test.describe.configure({ mode: 'serial' });

/** Pages with real content: axe must find nothing on any of them. */
test.describe('axe on pages with content', () => {
  test('setup after detection and a scan', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup?scan=run');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByText('Mods found').first()).toBeVisible({ timeout: 90_000 });
    expect(describe(await axeViolations(page)), 'axe').toBe('');
  });

  test('setup with the add folder dialog open', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup');
    await page.getByRole('button', { name: 'Add folder' }).click();
    await expect(page.getByRole('dialog', { name: 'Choose a folder' })).toBeVisible();
    await expect(page.getByRole('textbox', { name: 'Path' })).not.toHaveValue('');
    expect(describe(await axeViolations(page)), 'axe').toBe('');
  });

  test('project with its tree, layout tab and the new mod dialog', async ({ page, env }) => {
    await rememberProject(page, join(env.tmp, 'work', 'QA Test Mod'));
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree')).toBeVisible();
    expect(describe(await axeViolations(page)), 'axe tree').toBe('');
    await page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ }).click();
    expect(describe(await axeViolations(page)), 'axe layout').toBe('');
    await page.getByRole('button', { name: 'Layout guide' }).click();
    expect(describe(await axeViolations(page)), 'axe guide').toBe('');
    await page.getByRole('button', { name: 'New mod' }).first().click();
    await expect(page.getByRole('dialog', { name: 'New mod' })).toBeVisible();
    expect(describe(await axeViolations(page)), 'axe dialog').toBe('');
  });

  test('weapons with a draft open, the plan and the Combat Extended panel', async ({ page, env }) => {
    await rememberProject(page, join(env.tmp, 'work', 'QA Test Mod'));
    await gotoRoute(page, env, '/weapons');
    await expect(page.getByRole('button', { name: 'New ranged' })).toBeVisible();
    await cloneReference(page, 'pila', 'Gun_QaAxe');
    await expect(page.getByRole('heading', { level: 2, name: /copy$/ }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: /^Preview / }).first()).toBeVisible({ timeout: 30_000 });
    expect(describe(await axeViolations(page)), 'axe weapons').toBe('');
    await page.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' }).click();
    await expect(page.getByRole('heading', { name: /answers? still needed/ })).toBeVisible({ timeout: 30_000 });
    expect(describe(await axeViolations(page)), 'axe weapons with Combat Extended').toBe('');
  });

  test('patches with a scanned mod and its questions', async ({ page, env }) => {
    const mod = copyGewehr(join(env.tmp, 'work-axe'));
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/patches');
    await expect(page.getByRole('grid', { name: 'Weapons of the mod' }).getByRole('row')).toHaveCount(6, { timeout: 60_000 });
    expect(describe(await axeViolations(page)), 'axe patches').toBe('');
    await page.getByRole('tab', { name: 'Definition' }).click();
    await expect(page.getByRole('region').or(page.locator('pre, code')).first()).toBeVisible();
    expect(describe(await axeViolations(page)), 'axe definition').toBe('');
  });
});
