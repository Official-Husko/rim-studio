#!/usr/bin/env node
// Runner for the Tauri desktop shell (apps/desktop/src-tauri). Usage:
//   node scripts/tauri.mjs dev [-- tauri dev arguments]    live reload: Vite for the UI, the Tauri CLI rebuilds Rust
//                                                           (it watches the shell and every workspace crate it uses)
//   node scripts/tauri.mjs build [-- tauri build arguments] debug build without bundles
//   node scripts/tauri.mjs check                            cargo check and clippy for rimstudio-shell
//   node scripts/tauri.mjs smoke                            headless self test (no window, no display)
// Environment:
//   CARGO_TARGET_DIR        defaults to ~/.cache/rimstudio-target-tauri (never a folder in the repository)
//   RIMSTUDIO_DEV_PORT      port of the Vite dev server for `dev` (default 5173)
//   RIMSTUDIO_DATA_BASE     puts the app's data roots under a folder (smoke uses a temporary one)
//   WEBKIT_DISABLE_DMABUF_RENDERER  set to 1 on Linux unless you set it yourself
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const shellDir = join(root, 'apps', 'desktop', 'src-tauri');
const [mode, ...rest] = process.argv.slice(2);
const extra = rest.filter((a, i) => !(a === '--' && i === 0));

const env = {
  ...process.env,
  CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(homedir(), '.cache', 'rimstudio-target-tauri'),
};
if (process.platform === 'linux' && env.WEBKIT_DISABLE_DMABUF_RENDERER === undefined) {
  // Avoids blank windows on some GPU drivers (WebKitGTK DMA-BUF renderer).
  env.WEBKIT_DISABLE_DMABUF_RENDERER = '1';
}

function run(command, args, options = {}) {
  return new Promise((resolveRun) => {
    // On POSIX the child gets its own process group, so a signal to this script reaches the whole
    // tree (pnpm, the Tauri CLI, Vite and the app) and nothing is left running.
    const grouped = process.platform !== 'win32';
    const child = spawn(command, args, { cwd: root, env, stdio: 'inherit', detached: grouped, ...options });
    const forward = (signal) => {
      try {
        if (grouped && child.pid !== undefined) process.kill(-child.pid, signal);
        else child.kill(signal);
      } catch {
        /* already gone */
      }
    };
    process.on('SIGINT', forward);
    process.on('SIGTERM', forward);
    child.on('exit', (code, signal) => {
      process.off('SIGINT', forward);
      process.off('SIGTERM', forward);
      resolveRun(code ?? (signal ? 1 : 0));
    });
    child.on('error', (error) => {
      console.error(`[tauri] cannot start ${command}: ${error.message}`);
      resolveRun(1);
    });
  });
}

const tauri = (args) => run('pnpm', ['--filter', 'rimstudio-desktop', 'exec', 'tauri', ...args]);

async function dev() {
  const args = ['dev'];
  const port = process.env.RIMSTUDIO_DEV_PORT;
  if (port) {
    if (!/^\d{2,5}$/.test(port)) {
      console.error('[tauri] RIMSTUDIO_DEV_PORT must be a port number.');
      return 2;
    }
    const conf = JSON.parse(readFileSync(join(shellDir, 'tauri.conf.json'), 'utf8'));
    const devCsp = conf.app.security.devCsp.replaceAll('localhost:5173', `localhost:${port}`);
    args.push(
      '--config',
      JSON.stringify({
        build: {
          devUrl: `http://localhost:${port}`,
          beforeDevCommand: `pnpm --filter rimstudio-desktop dev --port ${port}`,
        },
        app: { security: { devCsp } },
      }),
    );
  }
  return tauri([...args, ...extra]);
}

async function build() {
  const release = extra.includes('--release');
  return tauri(['build', ...(release ? [] : ['--debug']), '--no-bundle', ...extra.filter((a) => a !== '--release')]);
}

async function check() {
  const steps = [
    ['cargo', ['check', '-p', 'rimstudio-shell', '--all-targets']],
    ['cargo', ['clippy', '-p', 'rimstudio-shell', '--all-targets', '--no-deps', '--', '-D', 'warnings']],
  ];
  for (const [cmd, args] of steps) {
    const code = await run(cmd, args);
    if (code !== 0) return code;
  }
  return 0;
}

async function smoke() {
  const ownBase = process.env.RIMSTUDIO_DATA_BASE === undefined;
  const base = ownBase ? mkdtempSync(join(tmpdir(), 'rimstudio-smoke-')) : process.env.RIMSTUDIO_DATA_BASE;
  env.RIMSTUDIO_DATA_BASE = base;
  try {
    return await run('cargo', ['run', '-p', 'rimstudio-shell', '--bin', 'rimstudio', '--', '--smoke']);
  } finally {
    if (ownBase) rmSync(base, { recursive: true, force: true });
  }
}

const modes = { dev, build, check, smoke };
if (!modes[mode]) {
  console.error('usage: node scripts/tauri.mjs <dev|build|check|smoke>');
  process.exit(2);
}
process.exit(await modes[mode]());
