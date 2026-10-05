import { join } from 'node:path';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { axeViolations, describe } from '../support/axe.ts';
import { copyGewehrWithPatch } from '../support/mods.ts';
import { rememberProject } from '../support/session.ts';
import { REAL } from '../support/env.ts';
import { pickFolderPath } from '../support/ui.ts';

test.describe.configure({ mode: 'serial' });

test.describe('setup: counts, duplicate groups and the Combat Extended card', () => {
  test('scan the library and read the new facts', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/setup');
    await page.getByRole('button', { name: 'Detect again' }).click();
    await expect(page.getByLabel('Detection report').getByText('Install folder')).toBeVisible();
    const folders = page.getByRole('list', { name: 'Mod folders' });
    if (!(await folders.getByText(REAL.customDir).first().isVisible().catch(() => false))) {
      await page.getByRole('button', { name: 'Add folder' }).click();
      await pickFolderPath(page, REAL.customDir);
      await page.getByRole('dialog', { name: 'Add a mod folder' }).getByRole('button', { name: 'Add folder' }).click();
    }
    await page.getByRole('button', { name: 'Scan', exact: true }).click();
    await expect(page.getByText('Mods found').first()).toBeVisible({ timeout: 90_000 });
    const text = await page.locator('body').innerText();
    const count = (label: string): number => {
      const m = new RegExp(`${label}\\s+([0-9][0-9,]*)`).exec(text);
      if (!m) throw new Error(`no count for ${label}`);
      return Number((m[1] ?? '').replace(/,/g, ''));
    };
    const mods = count('Mods found');
    const loadable = count('Loadable');
    const customOnly = count('Custom only');
    // every mod is either loadable or exists only in one of the owner's own folders
    expect(mods).toBeGreaterThan(0);
    expect(customOnly).toBeGreaterThan(0);
    expect(loadable + customOnly).toBe(mods);
    await expect(page.getByText(/mods? exist only in your own folders?/)).toBeVisible();
    await expect(page.getByText(/need a link or a copy|cannot load a mod that sits only/)).toBeVisible();
    // per source: the owner folder has no loadable mod of its own, every mod is custom only
    const scanned = page.getByRole('table').first();
    await expect(scanned).toContainText('Your folder');

    // duplicate groups: the package id, why, the copy that is kept and the copies that are skipped
    const duplicates = page.getByRole('table', { name: 'Duplicate package ids' });
    await expect(duplicates).toBeVisible();
    await expect(page.getByText(/^[0-9]+ package ids are used by more than one mod; [0-9]+ copies are skipped\./)).toBeVisible();
    await expect(duplicates.getByRole('row').nth(1)).toBeVisible();
    await expect(duplicates).toContainText(/The game (loads|rejects|does not see) it/);
    await expect(duplicates).toContainText(/Its About file is newer|comes first by folder path|higher priority wins|only copy the game can see|matches the game version|pinned/);

    // the Combat Extended card reads the library itself
    const ce = page.getByRole('region', { name: 'Combat Extended' });
    await expect(ce).toContainText('In your library');
    await expect(ce).toContainText(/Version\s*[0-9]+\.[0-9]+/);
    await expect(ce).toContainText(/CETeam\.CombatExtended/i);
    await expect(ce).toContainText('Workshop');
    expect(describe(await axeViolations(page)), 'axe setup facts').toBe('');
    await shot('data-setup');
  });

  test('Patches table: vanilla numbers, tags, sorting and filters on a copy of Gewehr 41', async ({ page, env, shot }) => {
    const mod = copyGewehrWithPatch(join(env.tmp, 'work-data'));
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/patches');
    const table = page.getByRole('grid', { name: 'Weapons of the mod' });
    await expect(table.getByText('OH_G41m').first()).toBeVisible({ timeout: 60_000 });
    for (const name of ['Weapon', 'Status', 'Open', 'Tags', 'Damage', 'Range', 'Cooldown', 'Warm-up', 'Mass']) {
      await expect(table.getByRole('columnheader', { name: new RegExp(`^${name}`, 'i') })).toBeVisible();
    }
    // the weapons of the mod already carry a conversion, so every row reads as converted
    await expect(table.getByRole('row')).toHaveCount(6);
    await expect(page.getByText('5 already CE')).toBeVisible();
    // numbers come from the vanilla definition: Gewehr 41 (m) has range 55, the sniper variants 68
    const row = table.getByRole('row', { name: /OH_G41m\b/ });
    await expect(row).toContainText('55');
    await expect(row).toContainText('Gun');
    await expect(page.getByText('Showing 5 of 5')).toBeVisible();

    // sort by range, both ways: the first row follows
    const first = async () => /OH_[A-Za-z0-9_]+/.exec(await table.getByRole('row').nth(1).innerText())?.[0] ?? '';
    await table.getByRole('columnheader', { name: /^Range/i }).click();
    const ascending = await first();
    await table.getByRole('columnheader', { name: /^Range/i }).click();
    const descending = await first();
    expect(ascending).not.toBe(descending);
    expect(ascending).toBe('OH_G41m');
    expect(descending).toMatch(/OH_G41w_(25r_)?sniper/);

    // filter by tag or class
    const tag = page.getByLabel('Tag or class');
    await tag.selectOption({ label: 'SniperRifle' });
    await expect(page.getByText('Showing 2 of 5')).toBeVisible();
    await expect(table.getByRole('row')).toHaveCount(3);
    await shot('data-patches-filtered');
    await tag.selectOption({ index: 0 });
    await expect(page.getByText('Showing 5 of 5')).toBeVisible();
    // the status filter offers the statuses that appear in the list and combines with the tag filter
    const status = page.getByLabel('Status');
    await expect(status.locator('option')).toHaveText(['Any', 'Already CE']);
    await status.selectOption({ label: 'Already CE' });
    await tag.selectOption({ label: 'SniperRifle' });
    await expect(page.getByText('Showing 2 of 5')).toBeVisible();
    await status.selectOption({ index: 0 });
    await tag.selectOption({ index: 0 });
    await expect(page.getByText('Showing 5 of 5')).toBeVisible();
    await shot('data-patches-table');
    expect(describe(await axeViolations(page)), 'axe patches table').toBe('');

    // the detail pane lists the tags and classes as chips
    await row.click();
    await expect(page.getByText('Tags and classes')).toBeVisible();
  });

  test('Lint tab: findings by file with rule ids, details and the rules that did not run', async ({ page, env, shot }) => {
    const mod = copyGewehrWithPatch(join(env.tmp, 'work-data-lint'));
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/patches');
    await page.getByRole('radio', { name: 'Lint' }).click();
    await expect(page.getByText(/^Files checked: 1\. Errors: [0-9]+\. Warnings: [0-9]+/)).toBeVisible({ timeout: 90_000 });
    await expect(page.getByText('Patches/ce_patch.xml').first()).toBeVisible();
    // the unguarded Combat Extended class in a patch outside the gated folder is an error of the rules
    await expect(page.getByText('CEP004').first()).toBeVisible();
    await expect(page.getByText('ce.cep004-ce-class-ungated').first()).toBeVisible();
    await expect(page.getByText(/Operation 1/).first()).toBeVisible();
    const details = page.getByText('Explanation and location').first();
    await details.click();
    await expect(page.getByText(/^Element:/).first()).toBeVisible();
    await expect(page.getByText(/rules? did not run/)).toBeVisible();
    await shot('data-lint');
    expect(describe(await axeViolations(page)), 'axe lint').toBe('');
  });
});
