import type { Page } from '@playwright/test';

/** Remembers a project as the last one, as the browser does after a session. */
export async function rememberProject(page: Page, path: string): Promise<void> {
  await page.addInitScript((value) => {
    try {
      window.localStorage.setItem('rimstudio.project.current', value);
    } catch {
      /* storage is a convenience */
    }
  }, path);
}
