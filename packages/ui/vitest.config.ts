import { fileURLToPath } from 'node:url';
import preactPlugin from '@preact/preset-vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [preactPlugin()],
  test: {
    environment: 'happy-dom',
    setupFiles: [fileURLToPath(new URL('../testkit/src/setup.ts', import.meta.url))],
    include: ['src/**/*.test.{ts,tsx}'],
    globals: true,
  },
});
