import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { axeViolations, describe } from '../support/axe.ts';
import { cloneReference, openWeaponsOn } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

test.describe.configure({ mode: 'serial' });

const DEF = 'Gun_QaAmmo';

/** Every file under a folder, relative to it. */
function walk(root: string, dir = ''): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const rel = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(root, rel));
    else out.push(rel);
  }
  return out;
}

test.describe('weapons: Combat Extended ammunition', () => {
  test('browse every ammo set, choose one, then make custom ammo with two types and apply', async ({
    page,
    env,
    shot,
  }) => {
    const work = join(env.tmp, 'work', 'Ammo Mod');
    mkdirSync(join(work, 'About'), { recursive: true });
    writeFileSync(
      join(work, 'About', 'About.xml'),
      '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData><name>Ammo Mod</name><author>qa</author>' +
        '<packageId>qa.ammomod</packageId><supportedVersions><li>1.6</li></supportedVersions>' +
        '<description>qa</description></ModMetaData>\n',
    );
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await cloneReference(page, 'bolt', DEF);

    const ce = page.getByRole('region', { name: 'Combat Extended', exact: true });
    await ce.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' }).click();
    await expect(ce.getByRole('button', { name: 'Browse all ammo' })).toBeVisible({
      timeout: 60_000,
    });
    await expect(ce.getByRole('group', { name: 'Quick picks, best fit first' })).toBeVisible();

    // The browser lists every ammo set of the install.
    await ce.getByRole('button', { name: 'Browse all ammo' }).click();
    const browser = page.getByRole('dialog', { name: 'Combat Extended ammunition' });
    const list = browser.getByRole('listbox', { name: 'Ammo sets' });
    await expect(browser.getByText(/^[0-9]+ ammo sets$/)).toBeVisible({ timeout: 120_000 });
    await expect(list.getByRole('option').first()).toBeVisible();
    await expect
      .poll(async () => Number(await list.getByRole('option').first().getAttribute('aria-setsize')))
      .toBeGreaterThanOrEqual(300);
    const size = Number(await list.getByRole('option').first().getAttribute('aria-setsize'));
    expect(size, 'ammo sets listed').toBeGreaterThanOrEqual(300);
    // only the rows in view are in the page, so the list stays light
    expect(await list.getByRole('option').count()).toBeLessThan(60);
    await expect(browser.getByRole('button', { name: 'Create custom ammo' })).toBeVisible();
    await shot('ammo-browser');
    expect(describe(await axeViolations(page)), 'axe ammo browser').toBe('');

    // Filter by words and by ammo class, then choose a set.
    await browser.getByRole('searchbox', { name: 'Search ammo sets' }).fill('303');
    await expect(browser.getByText(/^[0-9]+ of [0-9]+ ammo sets$/)).toBeVisible({
      timeout: 30_000,
    });
    const filtered = Number(await list.getByRole('option').first().getAttribute('aria-setsize'));
    expect(filtered).toBeGreaterThan(0);
    expect(filtered).toBeLessThan(size);
    await browser.getByRole('combobox', { name: 'Ammo class' }).selectOption({ index: 1 });
    await shot('ammo-browser-filtered');
    await browser.getByRole('combobox', { name: 'Ammo class' }).selectOption({ index: 0 });
    await browser.getByRole('searchbox', { name: 'Search ammo sets' }).fill('zzzz-no-such-ammo');
    await expect(browser.getByText('No ammo set matches')).toBeVisible({ timeout: 30_000 });
    await browser.getByRole('button', { name: 'Clear the filters' }).click();
    await browser.getByRole('searchbox', { name: 'Search ammo sets' }).fill('303British');
    await expect(list.getByRole('option').first()).toContainText('.303 British', {
      timeout: 30_000,
    });
    await list.getByRole('option').first().click();
    await browser.getByRole('button', { name: 'Select this set' }).click();
    await expect(browser).toBeHidden();
    await expect(ce.locator('[data-ammo-chosen]')).toContainText('AmmoSet_303British');
    await shot('ammo-chosen');

    // Answer the other questions so the plan can list the files.
    await ce.getByRole('radio', { name: 'All derived' }).check();
    const tag = ce.getByRole('combobox', { name: /Which weapon class tag/ });
    await tag.selectOption({
      label: (await tag.locator('option').allTextContents()).find((t) => /CE_AI_SR/.test(t)) ?? '',
    });
    await ce.getByRole('spinbutton', { name: 'CE magazine size' }).fill('5');
    await ce.getByRole('spinbutton', { name: 'CE shot spread' }).fill('0.2');
    for (const tool of ['stock', 'barrel']) {
      const field = ce.getByRole('spinbutton', { name: `Blunt penetration of the tool ${tool}` });
      if ((await field.count()) > 0 && (await field.inputValue()) === '') await field.fill('2');
    }
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeHidden({
      timeout: 60_000,
    });

    // Create custom ammunition with two types.
    await ce.getByRole('button', { name: 'Create custom ammo' }).click();
    const win = page.getByRole('dialog', { name: 'Create custom ammunition' });
    await expect(win).toBeVisible();
    await win.getByRole('textbox', { name: 'Name' }).fill('QaSix');
    await win.getByRole('textbox', { name: 'Caliber', exact: true }).fill('6mm QA');
    await win.getByRole('tab', { name: /^Ammo types/ }).click();
    await win.getByRole('button', { name: 'Add an ammo type' }).click();
    const classSelect = win.getByRole('combobox', { name: 'Ammo class' });
    await expect(classSelect.locator('option')).not.toHaveCount(1, { timeout: 120_000 });
    await classSelect.selectOption('FullMetalJacket');
    // the numbers start from the nearest of the user's own ammunition
    await expect(win.getByRole('spinbutton', { name: 'Damage', exact: true })).not.toHaveValue('', {
      timeout: 60_000,
    });
    await win.getByRole('spinbutton', { name: 'Damage', exact: true }).fill('11');
    await win.getByRole('spinbutton', { name: 'Speed' }).fill('140');
    await win.getByRole('button', { name: 'Add an ammo type' }).click();
    await win.getByRole('combobox', { name: 'Ammo class' }).selectOption('ArmorPiercing');
    await expect(win.getByRole('spinbutton', { name: 'Sharp penetration' })).not.toHaveValue('', {
      timeout: 60_000,
    });
    await win.getByRole('spinbutton', { name: 'Damage', exact: true }).fill('8');
    await win.getByRole('spinbutton', { name: 'Speed' }).fill('140');
    await shot('ammo-custom-types');
    await win.getByRole('tab', { name: 'Crafting' }).click();
    await expect(win.getByRole('group', { name: 'Ingredients of one craft' })).toBeVisible();
    await shot('ammo-custom-recipe');
    await win.getByRole('tab', { name: /^Review/ }).click();
    await expect(win.getByText('No problems found.')).toBeVisible({ timeout: 60_000 });
    await expect(win.getByRole('list', { name: 'Files that will be written' })).toContainText(
      'Compat/CombatExtended/Defs/Ammo/QaSix.xml',
    );
    await shot('ammo-custom-review');
    expect(describe(await axeViolations(page)), 'axe custom ammo').toBe('');
    await win.getByRole('button', { name: 'Save custom ammo' }).click();
    await expect(win).toBeHidden();
    await expect(ce.locator('[data-ammo-custom]')).toContainText('QaSix');

    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(
      output.getByRole('button', { name: /^Preview Compat\/CombatExtended\/Defs\/Ammo\// }),
    ).toBeVisible({ timeout: 60_000 });
    await shot('ammo-plan');
    await expect(output.getByRole('button', { name: 'Apply to project' })).toBeEnabled({
      timeout: 60_000,
    });
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await dialog.getByRole('button', { name: /^Write [0-9]+ files?/ }).click();
    await expect(dialog.getByText(/Wrote [0-9]+ files?/)).toBeVisible({ timeout: 60_000 });

    const files = walk(work);
    const defs = files.find((f) => f.startsWith(join('Compat', 'CombatExtended', 'Defs', 'Ammo')));
    expect(defs, `files: ${files.join(', ')}`).toBeDefined();
    const ammo = readFileSync(join(work, defs ?? ''), 'utf8');
    for (const name of [
      'AmmoSet_QaSix',
      'Ammo_QaSix_FMJ',
      'Bullet_QaSix_FMJ',
      'Ammo_QaSix_AP',
      'Bullet_QaSix_AP',
    ]) {
      expect(ammo, name).toContain(name);
    }
    expect(ammo).toContain('<damageAmountBase>11</damageAmountBase>');
    const patch = files.find((f) => f.startsWith(join('Compat', 'CombatExtended', 'Patches')));
    expect(readFileSync(join(work, patch ?? ''), 'utf8')).toContain('AmmoSet_QaSix');
    // the weapon itself stays a plain vanilla definition
    const vanilla = readFileSync(
      join(work, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial', `${DEF}.xml`),
      'utf8',
    );
    expect(vanilla).not.toContain('CombatExtended');
    expect(existsSync(join(work, 'LoadFolders.xml'))).toBe(true);
  });
});

test.describe('weapons: Combat Extended ammunition at a small window', () => {
  test.use({ viewport: { width: 1024, height: 700 } });

  test('the browser and the custom ammo window fit 1024 by 700', async ({ page, env, shot }) => {
    const work = join(env.tmp, 'work', 'Ammo Small');
    mkdirSync(join(work, 'About'), { recursive: true });
    writeFileSync(
      join(work, 'About', 'About.xml'),
      '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData><name>Ammo Small</name><author>qa</author>' +
        '<packageId>qa.ammosmall</packageId><supportedVersions><li>1.6</li></supportedVersions>' +
        '<description>qa</description></ModMetaData>\n',
    );
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await cloneReference(page, 'bolt', 'Gun_QaSmall');
    const ce = page.getByRole('region', { name: 'Combat Extended', exact: true });
    await ce.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' }).click();
    await ce.getByRole('button', { name: 'Browse all ammo' }).click();
    const browser = page.getByRole('dialog', { name: 'Combat Extended ammunition' });
    await expect(browser.getByText(/^[0-9]+ ammo sets$/)).toBeVisible({ timeout: 120_000 });
    await shot('ammo-small-browser');
    await browser.getByRole('button', { name: 'Create custom ammo' }).click();
    const win = page.getByRole('dialog', { name: 'Create custom ammunition' });
    await expect(win).toBeVisible();
    await win.getByRole('tab', { name: /^Ammo types/ }).click();
    await win.getByRole('button', { name: 'Add an ammo type' }).click();
    await shot('ammo-small-custom');
  });
});
