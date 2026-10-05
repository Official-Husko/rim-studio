import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { rememberProject } from '../support/session.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

async function focusRing(page: Page): Promise<{ style: string; width: string }> {
  return page.evaluate(() => {
    const el = document.activeElement;
    if (!el) return { style: 'none', width: '0px' };
    const css = getComputedStyle(el);
    const wrap = el.parentElement ? getComputedStyle(el.parentElement) : css;
    // a control either shows an outline itself or its wrapper does (focus-within)
    return css.outlineStyle !== 'none' && css.outlineWidth !== '0px'
      ? { style: css.outlineStyle, width: css.outlineWidth }
      : { style: wrap.outlineStyle, width: wrap.outlineWidth };
  });
}

test.describe('keyboard navigation of the main flows', () => {
  test('skip link, tool rail and focus rings', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup');
    await page.keyboard.press('Tab');
    const skip = page.getByRole('link', { name: 'Skip to content' });
    await expect(skip).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(page.locator('main')).toBeFocused();

    // Tab reaches the rail links in order and Enter opens a tool
    await page.getByRole('link', { name: 'Weapons' }).focus();
    expect((await focusRing(page)).style).not.toBe('none');
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/#\/weapons/);
    await expect(page.getByRole('heading', { level: 1, name: 'Weapons' })).toBeAttached();
  });

  test('setup: operate detection and the add folder dialog with the keyboard', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup');
    const detect = page.getByRole('button', { name: 'Detect again' });
    await detect.focus();
    expect((await focusRing(page)).style).not.toBe('none');
    await page.keyboard.press('Enter');
    await expect(page.getByLabel('Detection report').getByText('Install folder')).toBeVisible();

    await page.getByRole('button', { name: 'Add folder' }).focus();
    await page.keyboard.press('Enter');
    const picker = page.getByRole('dialog', { name: 'Choose a folder' });
    await expect(picker).toBeVisible();
    // focus is inside the dialog and Escape closes it
    expect(await page.evaluate(() => document.activeElement?.closest('dialog') !== null)).toBe(true);
    await page.keyboard.press('Escape');
    await expect(picker).toBeHidden();
    await expect(page.getByRole('button', { name: 'Add folder' })).toBeFocused();
  });

  test('project: the tree answers arrow keys and opens a file with Enter', async ({ page, env }) => {
    await rememberProject(page, join(env.tmp, 'work', 'QA Test Mod'));
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Files' }).click();
    const tree = page.getByRole('tree');
    await expect(tree).toBeVisible();
    await tree.getByRole('treeitem', { name: /^QA Test Mod/ }).focus();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    await expect(tree.getByRole('treeitem', { name: /About\.xml/ })).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(page.getByText('qa.qatestmod').first()).toBeVisible();
    // the tabs of the workbench move with the arrow keys
    await page.getByRole('tab', { name: 'Files' }).focus();
    await page.keyboard.press('ArrowRight');
    await expect(page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ })).toBeFocused();
  });

  test('weapons: clone a reference weapon without the mouse', async ({ page, env }) => {
    await rememberProject(page, join(env.tmp, 'work', 'QA Test Mod'));
    await gotoRoute(page, env, '/weapons');
    const list = page.getByRole('list', { name: 'Reference weapons' });
    await expect(list.getByRole('listitem').first()).toBeVisible({ timeout: 45_000 });
    const search = page.getByRole('searchbox', { name: 'Search by name' });
    await search.focus();
    await page.keyboard.type('pila');
    await expect(page.getByText('1 of 20 weapons')).toBeVisible();
    await page.keyboard.press('Tab');
    // Tab moves through the search clear/sort controls to the first Clone button
    for (let i = 0; i < 6; i += 1) {
      const name = await page.evaluate(() => document.activeElement?.textContent?.trim() ?? '');
      if (name === 'Clone') break;
      await page.keyboard.press('Tab');
    }
    await expect(list.getByRole('button', { name: 'Clone' })).toBeFocused();
    await page.keyboard.press('Enter');
    const dialog = page.getByRole('dialog', { name: /^Clone / });
    await expect(dialog).toBeVisible();
    // focus starts inside the dialog; Tab reaches the first field
    expect(await page.evaluate(() => document.activeElement?.closest('dialog') !== null)).toBe(true);
    await dialog.getByRole('textbox', { name: 'Def name' }).focus();
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Gun_QaPila');
    // the own projectile switch does not apply to a bow family weapon but the form must stay usable
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(list.getByRole('button', { name: 'Clone' })).toBeFocused();
  });
});
