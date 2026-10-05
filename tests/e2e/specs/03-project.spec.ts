import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { pickFolderPath } from '../support/ui.ts';

test.describe.configure({ mode: 'serial' });

test.describe('project flow', () => {
  test('create a new mod in a temporary folder and see its tree', async ({ page, env, shot }) => {
    const parent = join(env.tmp, 'work');
    mkdirSync(parent, { recursive: true });
    await gotoRoute(page, env, '/project');
    await page.getByRole('button', { name: 'Create a new mod' }).click();
    const dialog = page.getByRole('dialog', { name: 'New mod' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('textbox', { name: /Create in/ }).fill(parent);
    await dialog.getByRole('textbox', { name: /Mod name/ }).fill('QA Test Mod');
    await dialog.getByRole('textbox', { name: 'Author' }).fill('qa');
    await expect(dialog.getByRole('textbox', { name: /Package id/ })).toHaveValue('qa.qatestmod');
    await shot('project-new-dialog');
    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    await dialog.getByRole('button', { name: 'Next', exact: true }).click();
    await dialog.getByRole('button', { name: 'Create mod' }).click();
    await expect(dialog).toBeHidden();

    const folder = join(parent, 'QA Test Mod');
    expect(existsSync(join(folder, 'About', 'About.xml'))).toBe(true);
    expect(readFileSync(join(folder, 'About', 'About.xml'), 'utf8')).toContain('qa.qatestmod');
    // the new mod opens on its Basics tab
    await expect(page.getByRole('tab', { name: 'Basics', selected: true })).toBeVisible();
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree')).toBeVisible();
    await expect(page.getByRole('treeitem', { name: /About/ }).first()).toBeVisible();
    await shot('project-created');
    // the top bar follows the project
    await expect(page.getByRole('group', { name: 'Project' }).getByRole('combobox')).toHaveValue(folder);
  });

  test('open a mod through the folder browser and read a file', async ({ page, env, shot }) => {
    await gotoRoute(page, env, '/project');
    await page.getByRole('button', { name: /Open another|Choose a mod folder/ }).first().click();
    await pickFolderPath(page, join(env.tmp, 'work', 'QA Test Mod'));
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree')).toBeVisible();
    await page.getByRole('treeitem', { name: /About\.xml/ }).first().click();
    await expect(page.getByText('qa.qatestmod').first()).toBeVisible();
    await shot('project-file');
  });
});
