# RimStudio

A cross-platform desktop app for RimWorld modders: a fast mod manager plus a modding toolkit (def explorer, patch tester, item designer for weapons and apparel with optional Combat Extended patches, Steam Workshop publisher). The Rust workspace holds the backend; the frontend is a Preact, TypeScript and Tailwind app. The documentation index is [docs/README.md](docs/README.md).

Status: the 0.1.0 backend slice is built ([status](docs/status/0.1.0-backend.md)); the desktop shell does not exist yet. A temporary browser UI, served by Vite and backed by a development bridge, lets you try the designer against your own install.

## Quick start

Requirements: a Rust toolchain, Node 24 or newer and pnpm 11.

```sh
pnpm install                 # once, and after dependency changes
pnpm dev                     # bridge (cargo) + Vite on http://localhost:5173
```

What `pnpm dev` does:

1. Starts the development bridge, `cargo run -p rimstudio-devserver`, with its own data folder (`$HOME/.local/share/rimstudio-dev`), so it never mixes with the CLI folders. `CARGO_TARGET_DIR` defaults to `$HOME/.cache/rimstudio-target` when unset.
2. Waits for the bridge to write its token file (`node_modules/.cache/rimstudio-bridge.json`, with the port and the token).
3. Starts Vite, which proxies `/rpc` and `/dev` to the bridge and adds the `x-rimstudio-token` header to every request.

Arguments after `--` go to the bridge, for example `pnpm dev -- --data-dir /tmp/rs-dev --port 0`. The bridge finds your game through the usual detection; the environment variables `RIMSTUDIO_GAME_DIR`, `RIMSTUDIO_WORKSHOP_DIR` and `RIMSTUDIO_CE_DIR` point it at a specific install, workshop folder and Combat Extended copy.

Without the bridge, `pnpm dev:ui` starts only Vite. The app then shows recorded fixtures (the chip in the top bar reads "Mock data") and the component gallery works fully: `pnpm gallery` opens it.

## Commands

| Command                             | What it does                                                |
| ----------------------------------- | ----------------------------------------------------------- |
| `pnpm dev`                          | Bridge and Vite together                                    |
| `pnpm dev:ui`                       | Vite only (mock data)                                       |
| `pnpm gallery`                      | Vite, opened on the component gallery                       |
| `pnpm typecheck`                    | `tsc` in every package (strict, `noUncheckedIndexedAccess`) |
| `pnpm test`                         | Vitest in every package (happy-dom)                         |
| `pnpm lint`                         | oxlint with the import boundary rules of `.oxlintrc.json`   |
| `pnpm e2e`                          | Playwright (Firefox) against the real bridge and your install |
| `pnpm build`                        | Type check and production build of the desktop app          |
| `pnpm format`                       | Prettier over the frontend sources                          |
| `cargo test -p rimstudio-devserver` | The bridge tests (Rust side)                                |

Rust commands set `CARGO_TARGET_DIR` first when your default target folder is small, for example `export CARGO_TARGET_DIR=$HOME/.cache/rimstudio-target`.

## What runs where

| Part              | Folder                       | Notes                                                                                 |
| ----------------- | ---------------------------- | ------------------------------------------------------------------------------------- |
| Backend crates    | `crates/`                    | Business rules, file formats, the command registry (`rimstudio-app`)                  |
| Bridge            | `crates/rimstudio-devserver` | Development only, never packaged; loopback, token and origin checked                  |
| Desktop app       | `apps/desktop`               | Vite, Preact; features in `src/features`, shell in `src/app`, IPC in `src/shared/ipc` |
| Component library | `packages/ui`                | `rimstudio-ui`, presentational components and the icon set                            |
| Test kit          | `packages/testkit`           | Mock transport, fixtures, render helpers; never shipped                               |
| Generated types   | `packages/ipc-types`         | Written by the backend bindings test                                                  |

The webview receives JSON only. It has no XML library and no file access; the folder browser in the web build lists folders through the bridge. Details are in the section "As built: the temporary UI" of [docs/architecture/frontend-architecture.md](docs/architecture/frontend-architecture.md).
