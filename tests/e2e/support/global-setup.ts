import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeEnv } from './env.ts';
import { started } from './state.ts';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');

function freePort(): Promise<number> {
  return new Promise((ok, fail) => {
    const server = createServer();
    server.once('error', fail);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      const port = typeof address === 'object' && address ? address.port : 0;
      server.close(() => ok(port));
    });
  });
}

async function until<T>(what: string, probe: () => Promise<T | undefined> | T | undefined, ms = 60_000): Promise<T> {
  const end = Date.now() + ms;
  for (;;) {
    try {
      const value = await probe();
      if (value !== undefined) return value;
    } catch {
      /* not yet */
    }
    if (Date.now() > end) throw new Error(`Timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 250));
  }
}

/** Starts the bridge from a private copy of the binary and Vite on free ports. */
export default async function globalSetup(): Promise<void> {
  const tmp = mkdtempSync(join(tmpdir(), 'rimstudio-e2e-'));
  const shots = process.env.RIMSTUDIO_E2E_SHOTS ?? join(tmp, 'shots');
  mkdirSync(shots, { recursive: true });
  mkdirSync(join(tmp, 'data'), { recursive: true });

  const targetDir = process.env.CARGO_TARGET_DIR ?? join(process.env.HOME ?? tmp, '.cache', 'rimstudio-target');
  let bin = process.env.RIMSTUDIO_E2E_BRIDGE_BIN;
  if (!bin) {
    const built = spawnSync('cargo', ['build', '-p', 'rimstudio-devserver'], {
      cwd: root,
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
      stdio: 'inherit',
    });
    if (built.status !== 0) throw new Error('cargo build -p rimstudio-devserver failed');
    bin = join(tmp, 'bridge');
    copyFileSync(join(targetDir, 'debug', 'rimstudio-devserver'), bin);
    spawnSync('chmod', ['+x', bin]);
  }

  const vitePort = await freePort();
  const tokenFile = join(tmp, 'bridge.json');
  const children: ChildProcess[] = [];
  const bridge = spawn(
    bin,
    ['--port', '0', '--token-file', tokenFile, '--data-dir', join(tmp, 'data'), '--allow-origin', `http://127.0.0.1:${vitePort}`],
    { cwd: root, stdio: ['pipe', 'pipe', 'pipe'] },
  );
  children.push(bridge);
  const log: string[] = [];
  bridge.stdout?.on('data', (d: Buffer) => log.push(String(d)));
  bridge.stderr?.on('data', (d: Buffer) => log.push(String(d)));
  const info = await until('the bridge token file', () => {
    if (!existsSync(tokenFile)) return undefined;
    const parsed = JSON.parse(readFileSync(tokenFile, 'utf8')) as { port?: number; token?: string };
    return typeof parsed.port === 'number' && typeof parsed.token === 'string' ? { port: parsed.port, token: parsed.token } : undefined;
  });

  const vite = spawn('pnpm', ['exec', 'vite', '--port', String(vitePort), '--host', '127.0.0.1', '--strictPort'], {
    cwd: join(root, 'apps/desktop'),
    env: { ...process.env, RIMSTUDIO_BRIDGE_TOKEN_FILE: tokenFile },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  children.push(vite);
  vite.stdout?.on('data', (d: Buffer) => log.push(String(d)));
  vite.stderr?.on('data', (d: Buffer) => log.push(String(d)));
  const baseUrl = `http://127.0.0.1:${vitePort}`;
  await until('vite', async () => ((await fetch(baseUrl)).ok ? true : undefined));
  const bridgeUrl = `http://127.0.0.1:${info.port}`;
  await until('the bridge', async () => ((await fetch(`${bridgeUrl}/dev/health`)).ok ? true : undefined));
  // Vite proxied calls reach the bridge with the token injected.
  await until('the proxy', async () => ((await fetch(`${baseUrl}/dev/info`)).ok ? true : undefined));

  writeFileSync(join(tmp, 'stack.log'), log.join(''));
  writeEnv({ baseUrl, bridgeUrl, token: info.token, tmp, shots });
  // Keep the handles referenced by the process but do not block exit on them.
  for (const child of children) child.unref();
  started.push(...children);
}
