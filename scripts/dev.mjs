#!/usr/bin/env node
// Development runner: starts the bridge (rimstudio-devserver), waits for its token file, then
// starts Vite. Arguments after the script name are passed to the bridge, for example
//   pnpm dev -- --data-dir /tmp/rs-dev --allow-origin http://localhost:5173
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const bridgeArgs = process.argv.slice(2).filter((a) => a !== '--');
const tokenFile =
  process.env.RIMSTUDIO_BRIDGE_TOKEN_FILE ??
  bridgeArgs.find((a) => a.startsWith('--token-file='))?.slice('--token-file='.length) ??
  (bridgeArgs.includes('--token-file') ? bridgeArgs[bridgeArgs.indexOf('--token-file') + 1] : undefined) ??
  join(root, 'node_modules', '.cache', 'rimstudio-bridge.json');
// Both halves must agree on the file: the bridge writes it and the Vite proxy reads it.
if (!bridgeArgs.some((a) => a === '--token-file' || a.startsWith('--token-file='))) {
  bridgeArgs.push('--token-file', tokenFile);
}
const env = {
  ...process.env,
  CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(homedir(), '.cache', 'rimstudio-target'),
  RIMSTUDIO_BRIDGE_TOKEN_FILE: tokenFile,
};

const children = [];
let stopping = false;

function stop(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const child of children) child.kill('SIGTERM');
  setTimeout(() => process.exit(code), 300).unref();
}
process.on('SIGINT', () => stop(0));
process.on('SIGTERM', () => stop(0));

function start(label, command, args, options = {}) {
  const child = spawn(command, args, { cwd: root, env, stdio: 'inherit', ...options });
  children.push(child);
  child.on('exit', (code, signal) => {
    if (stopping) return;
    console.error(`[dev] ${label} exited (${signal ?? code}); stopping.`);
    stop(code ?? 1);
  });
  return child;
}

function readToken() {
  try {
    const parsed = JSON.parse(readFileSync(tokenFile, 'utf8'));
    if (typeof parsed.port === 'number' && typeof parsed.token === 'string') return parsed;
  } catch {
    /* not written yet */
  }
  return undefined;
}

async function healthy(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/dev/health`);
    return res.ok;
  } catch {
    return false;
  }
}

async function waitForBridge(timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const info = readToken();
    if (info && (await healthy(info.port))) return info;
    await new Promise((r) => setTimeout(r, 250));
  }
  return undefined;
}

if (existsSync(tokenFile)) rmSync(tokenFile, { force: true });
console.log('[dev] starting the bridge: cargo run -p rimstudio-devserver');
start('bridge', 'cargo', ['run', '-p', 'rimstudio-devserver', '--', ...bridgeArgs]);

const info = await waitForBridge(10 * 60 * 1000);
if (!info) {
  console.error('[dev] the bridge did not come up; see the cargo output above.');
  stop(1);
} else {
  console.log(`[dev] bridge ready on 127.0.0.1:${info.port}; starting vite`);
  start('vite', 'pnpm', ['--filter', 'rimstudio-desktop', 'dev']);
}
