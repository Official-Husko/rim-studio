import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** Opens the Weapons page on a project folder through the project prompt. */
export async function openWeaponsOn(page: Page, projectPath: string): Promise<void> {
  const prompt = page.getByRole('region', { name: 'Drafts' });
  await expect(prompt).toBeVisible();
  const field = prompt.getByRole('textbox', { name: 'Project folder' });
  if (await field.isVisible()) {
    await field.fill(projectPath);
    await prompt.getByRole('button', { name: 'Open project' }).click();
  }
  await expect(page.getByRole('button', { name: 'New ranged' })).toBeVisible();
}

/** Waits for the reference list, filters by name and starts a clone. */
export async function cloneReference(page: Page, search: string, defName: string): Promise<void> {
  const list = page.getByRole('list', { name: 'Reference weapons' });
  await expect(list.getByRole('listitem').first()).toBeVisible({ timeout: 45_000 });
  await page.getByPlaceholder('Search by name').fill(search);
  await expect(page.getByText('1 of 20 weapons')).toBeVisible();
  await list.getByRole('button', { name: 'Clone' }).first().click();
  const dialog = page.getByRole('dialog', { name: /^Clone / });
  await dialog.getByRole('textbox', { name: 'Def name' }).fill(defName);
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('heading', { level: 2, name: /copy$/ }).first()).toBeVisible();
}

/** The numeric text of a readout tile such as DPS. */
export async function readout(page: Page, label: string): Promise<number> {
  const tile = page
    .getByRole('region', { name: 'Live readouts' })
    .locator('div')
    .filter({ hasText: new RegExp(`^${label}\\s*[0-9]`) })
    .last();
  const text = (await tile.innerText()).replace(label, '');
  const match = /-?[0-9][0-9,]*\.?[0-9]*/.exec(text);
  if (!match) throw new Error(`no number in readout ${label}: ${text}`);
  return Number(match[0].replace(/,/g, ''));
}
