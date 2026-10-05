import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { IncomingMessage } from 'node:http';
import preactPlugin from '@preact/preset-vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

const here = (p: string): string => fileURLToPath(new URL(p, import.meta.url));

/** Where the development bridge writes {"port":N,"token":"..."} (docs: As built, the temporary UI). */
const tokenFile =
  process.env.RIMSTUDIO_BRIDGE_TOKEN_FILE ??
  here('../../node_modules/.cache/rimstudio-bridge.json');

interface BridgeInfo {
  port: number;
  token: string;
}

function readBridge(): BridgeInfo | undefined {
  try {
    const parsed: unknown = JSON.parse(readFileSync(tokenFile, 'utf8'));
    if (typeof parsed === 'object' && parsed !== null) {
      const { port, token } = parsed as Record<string, unknown>;
      if (typeof port === 'number' && typeof token === 'string') return { port, token };
    }
  } catch {
    /* no bridge yet */
  }
  return undefined;
}

const fallbackPort = Number(process.env.RIMSTUDIO_BRIDGE_PORT ?? '7878');
const target = `http://127.0.0.1:${readBridge()?.port ?? fallbackPort}`;

/** Proxy entry that injects the bridge token, read fresh from the token file on every request. */
const bridgeProxy = {
  target,
  changeOrigin: true,
  configure(proxy: {
    on: (
      event: 'proxyReq',
      handler: (proxyReq: { setHeader(k: string, v: string): void }, req: IncomingMessage) => void,
    ) => void;
  }): void {
    proxy.on('proxyReq', (proxyReq) => {
      const info = readBridge();
      if (info) proxyReq.setHeader('x-rimstudio-token', info.token);
    });
  },
};

export default defineConfig({
  plugins: [preactPlugin(), tailwindcss()],
  resolve: {
    alias: {
      '~': here('./src'),
      'rimstudio-ui': here('../../packages/ui/src/index.ts'),
      'rimstudio-testkit/setup': here('../../packages/testkit/src/setup.ts'),
      'rimstudio-testkit/mock': here('../../packages/testkit/src/mock.ts'),
      'rimstudio-testkit': here('../../packages/testkit/src/index.ts'),
      'rimstudio-ipc-types': here('../../packages/ipc-types/src/index.ts'),
    },
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    proxy: { '/rpc': bridgeProxy, '/dev': bridgeProxy },
  },
  build: { target: 'es2022', sourcemap: true },
  test: {
    environment: 'happy-dom',
    setupFiles: [here('../../packages/testkit/src/setup.ts')],
    include: ['src/**/*.test.{ts,tsx}'],
    globals: true,
  },
});
