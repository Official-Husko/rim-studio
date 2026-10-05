import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { axeViolations, describe } from '../support/axe.ts';
import { REAL } from '../support/env.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehr, copyLoneWolf, snapshot } from '../support/mods.ts';
import { createRequest, rpc } from '../support/rpc.ts';
import { rememberProject } from '../support/session.ts';

test.describe.configure({ mode: 'serial' });

const read = (path: string): string => readFileSync(path, 'utf8');

test.describe('mod basics: create a mod with the recommended structure and edit its basics', () => {
  let work = '';
  let mod = '';

  test('create the mod in the window, check every step, and land on Basics', async ({ page, env, shot }) => {
    work = join(env.tmp, 'work-basics');
    mkdirSync(work, { recursive: true });
    await gotoRoute(page, env, '/project');
    await page.getByRole('button', { name: 'Create a new mod' }).click();
    const dialog = page.getByRole('dialog', { name: 'New mod' });
    await dialog.getByRole('textbox', { name: /Create in/ }).fill(work);
    await dialog.getByRole('textbox', { name: /Mod name/ }).fill('Basics Pack');
    await dialog.getByRole('textbox', { name: 'Author' }).fill('Tester');

    // a bad package id is refused by the backend while typing
    const id = dialog.getByRole('textbox', { name: /Package id/ });
    await id.fill('nodots');
    await expect(dialog.getByText(/breaks the game's format rule/)).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Next', exact: true })).toBeDisabled();
    await id.fill('tester.basicspack');
    await expect(dialog.getByText(/breaks the game's format rule/)).toBeHidden();
    await shot('basics-new-1-identity');

    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(dialog.getByRole('list', { name: 'Structure of the new mod' })).toBeVisible();
    await expect(dialog.getByText('The About file. The game and the Workshop read it.')).toBeVisible();
    await dialog.getByRole('checkbox', { name: 'README.md' }).check();
    await dialog.getByRole('checkbox', { name: 'Languages' }).check();
    await expect(dialog.getByText('Text keys your mod uses.')).toBeVisible();
    await shot('basics-new-2-structure');

    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    const files = dialog.getByRole('list', { name: 'Files and folders that will be created' });
    await expect(files).toContainText('About/About.xml');
    await expect(files).toContainText('README.md');
    await expect(files).toContainText('Languages/English/Keyed/');
    await shot('basics-new-3-review');
    await dialog.getByRole('button', { name: 'Create mod' }).click();
    await expect(dialog).toBeHidden();

    mod = join(work, 'Basics Pack');
    expect(existsSync(join(mod, 'About', 'About.xml'))).toBe(true);
    expect(existsSync(join(mod, 'README.md'))).toBe(true);
    expect(existsSync(join(mod, 'Languages', 'English', 'Keyed'))).toBe(true);
    expect(existsSync(join(mod, 'Assemblies'))).toBe(false);
    await expect(page.getByRole('tab', { name: 'Basics', selected: true })).toBeVisible();
    await expect(page.getByRole('textbox', { name: /Package id/ })).toHaveValue('tester.basicspack');
    await shot('basics-opened');
  });

  test('a change to one field leaves every other byte of the file as it was', async ({ page, env, shot }) => {
    const file = join(mod, 'About', 'About.xml');
    const before = read(file);
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('textbox', { name: /Mod name/ }).fill('Basics Pack Two');
    await expect(page.getByText('1 unsaved change')).toBeVisible();
    await page.getByRole('button', { name: 'Review changes' }).click();
    const review = page.getByRole('dialog', { name: 'Review changes to About.xml' });
    await expect(review.getByRole('region', { name: 'Changes to About.xml' })).toContainText('Basics Pack Two');
    await shot('basics-review');
    await review.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('No unsaved changes')).toBeVisible();
    await expect(page.getByText(/^Saved\. The old file is in /)).toBeVisible();
    expect(read(file)).toBe(before.replace('<name>Basics Pack</name>', '<name>Basics Pack Two</name>'));
    // the header follows the saved name
    await expect(page.getByRole('heading', { level: 1, name: 'Basics Pack Two' })).toBeVisible();
  });

  test('edit every section, save, and read the result back from the disk', async ({ page, env, shot }) => {
    await rpc(env, 'library_scan', {});
    const file = join(mod, 'About', 'About.xml');
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('textbox', { name: /Mod name/ }).fill('Basics Pack');
    await page.getByRole('textbox', { name: 'Short name' }).fill('Basics');
    await page.getByRole('textbox', { name: 'Mod version' }).fill('1.2.0');
    const authors = page.getByRole('textbox', { name: 'Several authors' });
    await authors.fill('Second Person');
    await authors.press('Enter');
    await page.getByRole('textbox', { name: 'Description' }).fill('Line one.\nLine two.');
    await expect(page.getByText('19 characters')).toBeVisible();
    await page.getByRole('checkbox', { name: '1.5' }).check();

    // dependencies: Combat Extended with one click, then one from the library search
    await page.getByRole('button', { name: 'Add Combat Extended' }).click();
    const ce = page.getByRole('group', { name: 'Dependency Combat Extended' });
    await expect(ce.getByRole('textbox', { name: /Package id/ })).toHaveValue('CETeam.CombatExtended');
    await page.getByRole('button', { name: 'Add from library' }).click();
    await page.getByRole('searchbox', { name: 'Search the library for a dependency' }).fill('harmony');
    await page.getByRole('button', { name: /^Add Harmony/ }).first().click();
    await page.getByRole('button', { name: 'Add from library' }).click();
    await expect(page.getByRole('group', { name: /^Dependency / })).toHaveCount(2);

    // chip lists
    const incompatible = page.getByRole('textbox', { name: 'Incompatible with' });
    await incompatible.fill('some.badmod');
    await incompatible.press('Enter');
    await page.getByRole('textbox', { name: 'Mod icon path' }).fill('Things/Item/Icon');
    await expect(page.getByText(/unsaved changes$/)).toBeVisible({ timeout: 20_000 });
    await shot('basics-edited');

    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('No unsaved changes')).toBeVisible();
    const text = read(file);
    expect(text).toContain('<name>Basics Pack</name>');
    expect(text).toContain('<shortName>Basics</shortName>');
    expect(text).toContain('<modVersion>1.2.0</modVersion>');
    expect(text).toContain('Second Person');
    expect(text).toContain('Line one.');
    expect(text).toContain('<li>1.5</li>');
    expect(text).toContain('<packageId>CETeam.CombatExtended</packageId>');
    expect(text).toContain('<li>some.badmod</li>');
    expect(text).toContain('<modIconPath>Things/Item/Icon</modIconPath>');
    // the file is still read by the backend as the same mod
    const about = await rpc<{ packageId: { value: string }; supportedVersions: { items: string[] } }>(
      env,
      'project_about_get',
      { projectId: await projectIdOf(env, mod), includePreviewImage: false },
    );
    expect(about.packageId.value).toBe('tester.basicspack');
    expect(about.supportedVersions.items).toEqual(['1.5', '1.6']);
  });

  test('a bad package id and a collision with a library mod show findings, and saving is still allowed', async ({ page, env, shot }) => {
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    const id = page.getByRole('textbox', { name: /Package id/ }).first();
    await id.fill('Bad Id');
    await expect(page.getByText(/breaks the game's format rule/)).toBeVisible();
    await shot('basics-bad-id');
    await id.fill('CETeam.CombatExtended');
    await expect(page.getByText(/uses the same packageId/)).toBeVisible();
    await expect(page.getByText(/Combat Extended/).first()).toBeVisible();
    await shot('basics-collision');
    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(id).toHaveValue('tester.basicspack');
  });

  test('a file changed on disk is refused, explained, and saves after a reload', async ({ page, env, shot }) => {
    const file = join(mod, 'About', 'About.xml');
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('textbox', { name: /Mod name/ }).fill('Edited In The Page');
    await expect(page.getByText('1 unsaved change')).toBeVisible();
    // the preview has settled, then someone edits the file by hand
    await expect(page.getByRole('button', { name: 'Save' })).toBeEnabled();
    appendFileSync(file, '<!-- edited by hand -->\n');
    const before = read(file);
    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('About.xml changed on disk')).toBeVisible();
    expect(read(file)).toBe(before);
    await shot('basics-stale');
    await page.getByRole('button', { name: 'Reload' }).click();
    await expect(page.getByText('About.xml changed on disk')).toBeHidden();
    await expect(page.getByText('1 unsaved change')).toBeVisible();
    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('No unsaved changes')).toBeVisible();
    const text = read(file);
    expect(text).toContain('<name>Edited In The Page</name>');
    expect(text).toContain('<!-- edited by hand -->');
  });

  test('copies of the owner mods: edit the basics and change nothing else', async ({ page, env, shot }) => {
    // the originals are in the library, so the copies collide with them (a finding, never a block)
    // an earlier spec of the same stack may have added the folder already: a second add is refused
    const sources = await rpc<{ sources: { path: string }[] }>(env, 'sources_list', {});
    if (!sources.sources.some((source) => source.path === REAL.customDir)) {
      await rpc(env, 'sources_add_folder', { path: REAL.customDir });
    }
    await rpc(env, 'library_scan', {});
    const gewehr = copyGewehr(join(env.tmp, 'work-owner'));
    const lone = copyLoneWolf(join(env.tmp, 'work-owner'));
    for (const [folder, edit] of [
      [gewehr, { field: /Mod name/, from: "Huskos's Gewehr 41", to: "Huskos's Gewehr 41 (test copy)" }],
      [lone, { field: /Mod name/, from: 'The Lone Wolf Weapon Package', to: 'The Lone Wolf Weapon Package (test copy)' }],
    ] as const) {
      const file = join(folder, 'About', 'About.xml');
      const before = read(file);
      const others = snapshot(folder);
      await page.goto('about:blank');
      await rememberProject(page, folder);
      await gotoRoute(page, env, '/project');
      const name = page.getByRole('textbox', { name: edit.field });
      await expect(name).toHaveValue(edit.from);
      await name.fill(edit.to);
      await expect(page.getByText('1 unsaved change')).toBeVisible();
      await page.getByRole('button', { name: 'Save' }).click();
      await expect(page.getByText('No unsaved changes')).toBeVisible();
      expect(read(file)).toBe(before.replace(edit.from, edit.to));
      const after = snapshot(folder);
      for (const key of Object.keys(others)) {
        if (key === 'About/About.xml') continue;
        expect(after[key], key).toBe(others[key]);
      }
    }
    await shot('basics-owner-copy');
  });

  test('versions and folders: edit a LoadFolders.xml, add a version folder, create the file for a flat mod', async ({ page, env, shot }) => {
    const versioned = join(env.tmp, 'work-folders', 'Versioned');
    mkdirSync(join(env.tmp, 'work-folders'), { recursive: true });
    await rpc(env, 'project_create', createRequest(versioned, 'Versioned', 'qa.versioned', {
      versionedFolders: true,
      cePatchFolder: true,
      supportedVersions: ['1.5', '1.6'],
    }));
    await rememberProject(page, versioned);
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Versions and folders' }).click();
    const list = page.getByRole('list', { name: 'Folders for 1.6' });
    await expect(list.getByRole('textbox', { name: 'Folder path 1.6', exact: true })).toBeVisible();
    await expect(list.getByText('Only if any is active: ceteam.combatextended')).toBeVisible();
    const field = page.getByRole('textbox', { name: 'Add a folder to 1.6' });
    await field.fill('1.6/Extra');
    await field.press('Enter');
    await expect(page.getByText('1 unsaved change')).toBeVisible();
    await expect(page.getByText('Missing')).toBeVisible();
    await page.getByRole('button', { name: 'Review changes' }).click();
    const review = page.getByRole('dialog', { name: 'Review changes to LoadFolders.xml' });
    await expect(review.getByRole('region', { name: 'Changes to LoadFolders.xml' })).toContainText('1.6/Extra');
    await shot('folders-review');
    await review.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('No unsaved changes')).toBeVisible();
    expect(read(join(versioned, 'LoadFolders.xml'))).toContain('<li>1.6/Extra</li>');
    await shot('folders-edited');
    expect(describe(await axeViolations(page)), 'axe folders').toBe('');

    // a version folder with its standard folders and its block
    await page.getByRole('button', { name: 'Add a version folder' }).click();
    const dialog = page.getByRole('dialog', { name: 'Add a version folder' });
    await dialog.getByRole('textbox', { name: 'Game version' }).fill('1.7');
    await expect(dialog.getByRole('region', { name: 'What will be created' })).toBeVisible();
    await dialog.getByRole('button', { name: 'Create the folder' }).click();
    await expect(dialog).toBeHidden();
    expect(existsSync(join(versioned, '1.7', 'Defs'))).toBe(true);
    expect(read(join(versioned, 'LoadFolders.xml'))).toContain('v1.7');

    // a flat mod has no LoadFolders.xml: it is created only when asked
    const flat = join(env.tmp, 'work-folders', 'Flat');
    await rpc(env, 'project_create', createRequest(flat, 'Flat', 'qa.flat'));
    await page.goto('about:blank');
    await rememberProject(page, flat);
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Versions and folders' }).click();
    await expect(page.getByText('This mod has no LoadFolders.xml')).toBeVisible();
    await page.getByRole('button', { name: 'Create LoadFolders.xml' }).click();
    await page.getByRole('button', { name: 'Add a block' }).click();
    await expect(page.getByText('1 unsaved change')).toBeVisible();
    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('No unsaved changes')).toBeVisible();
    expect(read(join(flat, 'LoadFolders.xml'))).toContain('<v1.6>');
  });

  test('the new views pass axe, at 1440 by 900 and at 1024 by 700', async ({ page, env, shot }) => {
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await expect(page.getByRole('textbox', { name: /Mod name/ })).toBeVisible();
    expect(describe(await axeViolations(page)), 'axe basics').toBe('');
    await page.getByRole('button', { name: 'Advanced' }).click();
    expect(describe(await axeViolations(page)), 'axe advanced').toBe('');
    await shot('basics-1440');
    await page.getByRole('button', { name: 'New mod' }).first().click();
    const dialog = page.getByRole('dialog', { name: 'New mod' });
    expect(describe(await axeViolations(page)), 'axe create 1').toBe('');
    await dialog.getByRole('textbox', { name: /Create in/ }).fill(join(env.tmp, 'work-basics'));
    await dialog.getByRole('textbox', { name: /Mod name/ }).fill('Axe Pack');
    await dialog.getByRole('textbox', { name: 'Author' }).fill('Tester');
    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    expect(describe(await axeViolations(page)), 'axe create 2').toBe('');
    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    expect(describe(await axeViolations(page)), 'axe create 3').toBe('');
    await dialog.getByRole('button', { name: 'Cancel' }).click();

    await page.setViewportSize({ width: 1024, height: 700 });
    await expect(page.getByRole('textbox', { name: /Mod name/ })).toBeVisible();
    const sideways = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
    expect(sideways).toBe(false);
    await shot('basics-1024');
    await page.getByRole('tab', { name: 'Versions and folders' }).click();
    await shot('folders-1024');
    await page.getByRole('button', { name: 'New mod' }).first().click();
    await shot('create-1024');
  });

  test('the hub offers both cards when no mod is open', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/project');
    await expect(page.getByRole('region', { name: 'Create a new mod' })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Open a mod' })).toBeVisible();
    expect(describe(await axeViolations(page)), 'axe hub').toBe('');
    await shot('hub-1440');
    writeFileSync(join(env.tmp, 'basics-done.txt'), 'ok');
  });
});

async function projectIdOf(env: Parameters<typeof rpc>[0], path: string): Promise<string> {
  const opened = await rpc<{ projectId: string }>(env, 'project_open', { path });
  return opened.projectId;
}
