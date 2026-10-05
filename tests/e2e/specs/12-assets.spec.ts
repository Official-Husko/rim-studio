import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { axeViolations, describe } from '../support/axe.ts';
import { cloneReference, openWeaponsOn } from '../support/designer.ts';
import { REAL } from '../support/env.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

const DEF = 'Gun_QaAssets';
const LONE = join(REAL.customDir, '[OH] The Lone Wolf Weapon Package');
const PNG = join(LONE, 'Textures/Things/Weapon/Ranged/Rifle/TLWWP_Eagle_Carbine.png');
const WAV = join(LONE, 'Sounds/Weapons/TLWWP_AK_47_Shot.wav');

/** Chooses a file in the bridge file browser: the folder in the path box, then the row, then Choose. */
async function pickFilePath(page: Page, file: string): Promise<void> {
  const dialog = page.getByRole('dialog', { name: 'Choose a file' });
  await expect(dialog).toBeVisible();
  const field = dialog.getByRole('textbox', { name: 'Path' });
  await expect(field).not.toHaveValue('');
  const slash = file.lastIndexOf('/');
  await field.fill(file.slice(0, slash));
  await field.press('Enter');
  await dialog.getByRole('row', { name: new RegExp(file.slice(slash + 1).replace('.', '\\.')) }).click();
  await dialog.getByRole('button', { name: 'Choose this file' }).click();
  await expect(dialog).toBeHidden();
}

test.describe('weapons: texture and sound imports', () => {
  test('import a PNG and a clip, see the plan, apply and find the copies on disk', async ({ page, env, shot }) => {
    const work = join(env.tmp, 'work', 'Assets Mod');
    mkdirSync(join(work, 'About'), { recursive: true });
    writeFileSync(
      join(work, 'About', 'About.xml'),
      '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData><name>Assets Mod</name><author>qa</author>' +
        '<packageId>qa.assets</packageId><supportedVersions><li>1.6</li></supportedVersions>' +
        '<description>qa</description></ModMetaData>\n',
    );
    // the owner's files are only read: the copies come from a temporary folder
    const art = join(env.tmp, 'art');
    mkdirSync(art, { recursive: true });
    copyFileSync(PNG, join(art, 'carbine.png'));
    copyFileSync(WAV, join(art, 'bang.wav'));

    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await cloneReference(page, 'bolt', DEF);

    const texture = page.getByRole('region', { name: 'Texture', exact: true });
    await texture.getByRole('button', { name: 'Import PNG' }).first().click();
    await pickFilePath(page, join(art, 'carbine.png'));
    await expect(texture.getByAltText('Thumbnail of carbine.png')).toBeVisible();
    await expect(texture.getByText('512 by 512 pixels')).toBeVisible();

    const sounds = page.getByRole('region', { name: 'Sounds', exact: true });
    await sounds.getByRole('radio', { name: 'Own clips' }).click();
    await sounds.getByRole('button', { name: 'Add clip' }).click();
    await pickFilePath(page, join(art, 'bang.wav'));
    await expect(sounds.getByRole('list', { name: 'Clips of the shot sound' })).toContainText('bang.wav');
    await expect(sounds.getByText('WAV sound')).toBeVisible();

    const output = page.getByRole('region', { name: 'Output', exact: true });
    const files = output.getByRole('list', { name: 'Files of the plan' });
    await expect(files).toContainText('Texture');
    await expect(files).toContainText('Sound clip');
    await expect(files).toContainText('Sound definitions');
    await expect(sounds.getByText('What will be written')).toBeVisible();
    await shot('assets-imported');
    expect(describe(await axeViolations(page)), 'axe assets').toBe('');

    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await expect(dialog.getByRole('list', { name: 'Files to write' })).toContainText('bang.wav');
    await expect(dialog.getByText(/of these files are copied from your computer/)).toBeVisible();
    await shot('assets-apply-review');
    expect(describe(await axeViolations(page)), 'axe assets apply review').toBe('');
    await dialog.getByRole('button', { name: /^Write 4 files/ }).click();
    await expect(dialog.getByText('Wrote 4 files')).toBeVisible();
    await shot('assets-apply-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    const png = join(work, 'Textures', 'Things', 'Item', 'Equipment', 'WeaponRanged', `${DEF}.png`);
    expect(existsSync(png)).toBe(true);
    expect(readFileSync(png).equals(readFileSync(PNG))).toBe(true);
    const clip = join(work, 'Sounds', 'Weapons', `${DEF}_Shot`, 'bang.wav');
    expect(readFileSync(clip).equals(readFileSync(WAV))).toBe(true);
    const sound = readFileSync(join(work, 'Defs', 'SoundDefs', 'World_Oneshots_Weapons.xml'), 'utf8');
    expect(sound).toContain(`Weapons/${DEF}_Shot/bang`);
    expect(readFileSync(join(work, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial', `${DEF}.xml`), 'utf8')).toContain(
      `${DEF}_Shot`,
    );

    await page.getByRole('link', { name: 'Mod' }).click();
    await page.getByRole('tab', { name: 'Files' }).click();
    const tree = page.getByRole('tree');
    await expect(tree.getByRole('treeitem', { name: /^Textures/ }).first()).toBeVisible();
    await expect(tree.getByRole('treeitem', { name: /^Sounds/ }).first()).toBeVisible();
    await shot('assets-project-tree');
  });
});
