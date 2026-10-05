import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** Types a folder path into the open folder browser and chooses it. */
export async function pickFolderPath(page: Page, path: string): Promise<void> {
  const dialog = page.getByRole('dialog', { name: 'Choose a folder' });
  await expect(dialog).toBeVisible();
  const field = dialog.getByRole('textbox', { name: 'Path' });
  await expect(field).not.toHaveValue('');
  await field.fill(path);
  await field.press('Enter');
  await expect(field).toHaveValue(path);
  await dialog.getByRole('button', { name: 'Choose this folder' }).click();
}

/** Opens a mod folder on the Project page through the folder browser. */
export async function openProjectFolder(page: Page, path: string): Promise<void> {
  await page.getByRole('button', { name: /Choose a mod folder|Open another/ }).first().click();
  await pickFolderPath(page, path);
}
