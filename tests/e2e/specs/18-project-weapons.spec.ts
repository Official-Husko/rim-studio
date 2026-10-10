import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { openWeaponsOn } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehr } from '../support/mods.ts';
import { walk } from '../support/patches.ts';

test.describe.configure({ mode: 'serial' });

const FILE = join('Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial.xml');
const WEAPON = 'Gewehr 41 (m)';

test.describe('weapons: the weapons of the loaded mod', () => {
  test('list them, read the details, load one, change a number and apply it in place', async ({
    page,
    env,
    shot,
  }) => {
    const work = join(env.tmp, 'work-project-weapons');
    mkdirSync(work, { recursive: true });
    const mod = copyGewehr(work);
    const before = readFileSync(join(mod, FILE), 'utf8');
    const filesBefore = walk(mod);

    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, mod);
    const list = page.getByRole('list', { name: "This mod's weapons" });
    // the rows are the direct children; the number chips inside a row are list items too
    await expect(list.locator(':scope > li')).toHaveCount(5, { timeout: 60_000 });
    await expect(list.getByText(WEAPON).first()).toBeVisible();
    await shot('weapons-project-list');

    // the details of one weapon
    await page.getByRole('button', { name: `Details of ${WEAPON}` }).click();
    await expect(page.getByText(FILE.replace(/\\/g, '/')).first()).toBeVisible();
    await shot('weapons-project-details');

    // load it: it is edited where it stands
    await page.getByRole('button', { name: `Load ${WEAPON} into the editor` }).click();
    await expect(page.getByRole('heading', { level: 2, name: WEAPON }).first()).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText('This weapon is edited where it stands')).toBeVisible();
    await shot('weapons-project-loaded');

    const output = page.getByRole('region', { name: 'Output', exact: true });
    // nothing was changed yet, so the plan has nothing to write
    const damage = page.getByRole('spinbutton', { name: 'Damage', exact: true });
    await expect(damage).toHaveValue('26');
    await damage.fill('31');
    await damage.blur();
    await expect(output.getByRole('button', { name: 'Apply to project' })).toBeEnabled({ timeout: 30_000 });
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await expect(dialog.getByRole('list', { name: 'Files to write' })).toContainText('RangedIndustrial.xml');
    await shot('weapons-project-apply-review');
    await dialog.getByRole('button', { name: /^Write 1 file/ }).click();
    await expect(dialog.getByText('Wrote 1 file')).toBeVisible({ timeout: 60_000 });
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    // on disk: the same file, one changed value, no second definition and no new file
    const after = readFileSync(join(mod, FILE), 'utf8');
    const changed = before
      .split('\n')
      .map((line, i) => [line, after.split('\n')[i]])
      .filter(([a, b]) => a !== b);
    expect(after.split('\n')).toHaveLength(before.split('\n').length);
    expect(changed).toHaveLength(1);
    expect(changed[0]?.[1]).toContain('<damageAmountBase>31</damageAmountBase>');
    expect(after.match(/<defName>OH_G41m<\/defName>/g)).toHaveLength(1);
    expect(walk(mod)).toEqual(filesBefore);
  });
});
