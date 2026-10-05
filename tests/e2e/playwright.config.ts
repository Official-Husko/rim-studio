import { defineConfig } from '@playwright/test';

/**
 * End to end tests of the temporary UI against the real bridge. The stack (bridge and Vite on
 * free ports, temporary data) is started by support/global-setup.ts, so the base URL is set per
 * test from the environment it exports.
 */
export default defineConfig({
  testDir: './specs',
  outputDir: './.results',
  globalSetup: './support/global-setup.ts',
  globalTeardown: './support/global-teardown.ts',
  workers: 1,
  fullyParallel: false,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  retries: 0,
  reporter: [['list']],
  use: {
    browserName: 'firefox',
    viewport: { width: 1440, height: 900 },
    trace: 'off',
    actionTimeout: 20_000,
    screenshot: 'only-on-failure',
    video: 'off',
  },
});
