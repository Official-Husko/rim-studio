# RimStudio cross-platform strategy

This document fixes how RimStudio behaves and is built, tested and shipped on Windows, macOS and Linux (native, Flatpak, Snap, Steam Deck and Proton). It covers the OS behaviour matrix, the rule that confines platform code to one crate and how that code is tested without the real OS, the webview floor and startup probe, Linux graphics safe mode, packaging, updates, signing and CI, Steam Deck acceptance criteria, the in-game link-farm test plan and the manual test matrix. It ends with an explicit statement of what was verified on the Linux research machine and what is still unverified on Windows and macOS. It refines requirements R1 (cross-platform), R3 (detection), R4 (custom folders) and R10 (JSON only), and it relies on the crate layout in [overview](overview.md), [workspace layout](workspace-layout.md), [crate catalog](crate-catalog.md) and the [decision register](decision-register.md) (decisions D-009, D-010, D-024, D-039, D-040, D-041, D-059, D-070).

Status: draft | Last updated: 2026-10-04

Evidence lives in [cross-platform packaging research](../research/cross-platform-packaging-research.md), [steam and game detection](../research/steam-and-game-detection.md), [rimworld mod format and corpus](../research/rimworld-mod-format-and-corpus.md) and [webview and IPC performance](../research/webview-and-ipc-performance.md). Statements marked "(unverified)" were not confirmed by execution and are listed again in section 12.

## 1. Principles

1. One code base, one behaviour. Differences between operating systems are data (a table row, a port implementation) and never an `if cfg!(windows)` scattered through feature code (invariant I-11, platform quarantine).
2. Detect, never assume. Every location (Steam root, install, user config, Mods folder, Player.log) is a candidate list with evidence and confidence in a `DetectionReport`; the user can override each one and overrides are never discarded (D-038).
3. Fail visibly and reversibly. Every OS refusal (permissions, read-only volume, sandbox, locked file) becomes a `Diagnostic` with a code in the `deploy.*`, `scan.*` or `steam.*` areas and a fix hint. Nothing retries in a loop and nothing silently falls back to a riskier mechanism.
4. The game folder is fenced. The only writes under the install or the game's config folder are owned link-farm entries and `ModsConfig.xml` after a backup (invariant I-05, D-040). This rule is strongest on macOS, where the Mods folder lives inside the application bundle.
5. Degrade to read-only. When a capability is unknown (sandboxed process list, no link privilege), the app keeps working in browse, sort and plan mode and disables only the action that needs the capability.

## 2. OS behaviour matrix

The matrix lists what differs per environment. Columns are the nine environments that matter; rows are the behaviours that change code or UX. "same" means identical to the previous column.

### 2.1 Locations

| Topic | Windows 10 and 11 | macOS 13.3+ | Linux native | Flatpak Steam | Snap Steam | Steam Deck | Proton (Windows build of the game) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Steam root probe order | HKCU `SteamPath`, HKLM WOW6432Node `InstallPath`, HKLM `InstallPath`, `C:\Program Files (x86)\Steam`, `C:\Program Files\Steam` | `~/Library/Application Support/Steam` | `$XDG_DATA_HOME/Steam`, `~/.steam/steam`, `~/.steam/root`, `~/.steam/debian-installation` | `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` and the two `.steam` variants | `~/snap/steam/common/.local/share/Steam` | as native for user `deck` (unverified) | same Steam root as native Linux |
| Root validity test | `steam.exe` or `steamapps\` exists | `steamapps/` or `config/libraryfolders.vdf` | same as macOS | same | same | same | same |
| Game install | `steamapps\common\RimWorld`, executable `RimWorldWin64.exe` | `.../RimWorld/RimWorldMac.app` (glob `*.app`, never a fixed case) | `.../RimWorld`, executable `RimWorldLinux`, data `RimWorldLinux_Data` | same layout inside the Flatpak home | same | same, library may be on the SD card | `RimWorldWin64.exe` beside or instead of the Linux binary (unverified) |
| Install accepted when | `Data/Core/About/About.xml` exists (all OS) | same | same | same | same | same | same |
| User config folder | `%USERPROFILE%\AppData\LocalLow\Ludeon Studios\RimWorld by Ludeon Studios`, resolved with the `FOLDERID_LocalAppDataLow` known folder, not by string building | `~/Library/Application Support/RimWorld` (two competitors agree; not executed on a Mac) | `~/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios` (verified), `XDG_CONFIG_HOME` honoured (unverified) | both `~/.var/app/com.valvesoftware.Steam/.config/unity3d/...` and `.../config/unity3d/...` probed; which one the game uses is open | probed under the snap home (unverified) | as native | `<root>/steamapps/compatdata/294100/pfx/drive_c/users/steamuser/AppData/LocalLow/Ludeon Studios/RimWorld by Ludeon Studios`, accepted only when that path exists |
| Player.log | beside `Config\` | beside `Config/` (two listed locations, both to be probed) | beside `Config/` (verified, with `Player-prev.log`) | same as the chosen config folder | same | same | inside the prefix config folder |
| Default among several config candidates | the candidate whose `Config/ModsConfig.xml` is newest; the user can change it; `-savedatafolder` wins when RimStudio itself launches the game | same | same | same | same | same | same |
| Mods folder | `<install>\Mods` | inside `RimWorldMac.app` (App Management may refuse writes) | `<install>/Mods` | `<install>/Mods`, reachable by the game only for granted paths | same | same | same as Windows install folder |
| Workshop content | `<library>\steamapps\workshop\content\294100`, scanned in every library | same | same | same | same | same, SD card library supported | same |

### 2.2 Mechanisms and hazards

| Topic | Windows | macOS | Linux (all variants) | Handling |
| --- | --- | --- | --- | --- |
| Link farm mechanism (D-039) | junction for local volumes (no privilege needed, unverified), directory symlink only when Developer Mode or the privilege is present, never relative | directory symlink, absolute target | directory symlink, absolute target | `LinkBackend` port in `rimstudio-core`, implemented in `rimstudio-platform`; planner and manifest are pure code in `rimstudio-library::deploy` |
| Link fallbacks | copy when the target is a UNC path or links fail; copy carries `.rimstudio.json` with source id, mtime, file count and hash | copy when the bundle refuses writes is useless because the game scans only its own folder, so show the permission instruction | Flatpak: check the target prefix against the known grants (`/mnt`, `/media`, `/run/media`), else copy or tell the user to grant access; read-only or root-owned Mods folder: explain | fallback is chosen per mod and recorded in the plan the user sees before applying |
| Running game detection (D-041) | `sysinfo` process list matched on the executable path under the detected install | same | same; plus Steam `AppRunning` flag (state flag 64) from the appmanifest | in a sandbox where the process list is blind the result is `unknown`, which warns and does not block |
| Case sensitivity | NTFS and exFAT insensitive | APFS insensitive by default, may be sensitive | ext4 and btrfs sensitive | the `About` folder name must match exactly; `About.xml` is resolved by a case-insensitive directory-listing match (5 of 691 real Workshop mods use lowercase); case folding for path comparison only on volumes probed as insensitive; case-only filename collisions are reported |
| Path length | MAX_PATH 260; extended `\\?\` prefix used internally, `longPathAware` manifest entry requested (mechanism unverified); warn above 240 characters | n/a | n/a | `rimstudio-platform` strips the prefix for display (via a small helper, not by string surgery in features) |
| Reserved names | `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, trailing dots and spaces | n/a | n/a | validated in scaffolder, project store, designer and export file names on every OS so a mod made on Linux does not break on Windows |
| File locking | delete of an in-use file may fail | rare | rare | atomic writes (temp file in the same folder, rename); check for the running game before `ModsConfig.xml`; one backup per write |
| Antivirus | real-time scanners slow mass opens; unsigned binaries are scanned harder (unverified) | Gatekeeper scan on first run | none by default | read only what is needed (About.xml first, Defs lazily), no full-tree open at start, documented exclusion tip, manual test with Defender on |
| Consent prompts | none | TCC prompts for Documents, Desktop, Downloads, removable and network volumes; Full Disk Access is separate | portal prompts only inside sandboxes | a permission error is reported as "permission denied for folder X, how to grant", never as "folder empty"; the first scan of an external drive shows a "waiting for permission" state |
| Removable and network drives | drive letters can change; UNC paths | volumes under `/Volumes` | `/run/media/<user>/<label>` changes with user and label | custom folder keeps `volumeHint`, shows `offline`, keeps cached mods as unavailable, blocks activation, never retargets automatically; existence checks run on a worker with a deadline because a stalled mount blocks `stat` forever |
| Unicode normalisation | NFC typical | HFS+ stored NFD; APFS preserves bytes but compares insensitively (unverified) | bytes as given | NFC only for comparison keys and display, never when writing a path back; identity by `(device, inode)` or file id where needed |
| Non-UTF-8 paths | not applicable (UTF-16) | not applicable | possible | stored as `OsString`; rejected at the IPC edge with a clear error (unverified policy) |
| Timestamp granularity | NTFS 100 ns | APFS ns | ext4, btrfs ns | see section 2.3 |
| File watching | `ReadDirectoryChangesW`, no event limit concern | FSEvents | inotify `max_user_watches` (524288 here, lower on older systems) | watch roots and metadata files only, debounced, poll fallback on ENOSPC, recursive only for the active project (D-028); network drives deliver no events so `watch` is advisory and a rescan command plus focus rescan exist |
| Symlink loops | junction loops possible | possible | possible | scanner never follows links below a root unless the target is inside a configured root and tracks visited `(device, inode)` or file id |

### 2.3 Timestamp granularity and the cache key

The cache key per file is `(relative path, size, mtime, file id)` compared for equality only, never "newer than" (D-023). Volumes differ in resolution: NTFS 100 ns, ext4 and btrfs nanoseconds, exFAT 10 ms (unverified), FAT-family 2 seconds with local-time storage that can shift by one hour across daylight saving changes in some APIs. Rules:

1. On volumes detected as FAT-family, mtime is rounded down to even seconds before comparison and a blake3 content hash of small files (About.xml, LoadFolders.xml, ModsConfig.xml) is stored as a second key.
2. A difference of exactly 3600 seconds with equal size and equal hash is treated as unchanged.
3. The key is computed by `rimstudio-io` from stat data it receives through `FsProbe`; the volume class (`Fat`, `Exfat`, `Ntfs`, `Posix`) comes from `rimstudio-platform`.
4. Test: a property test simulates 2 second and 1 hour shifts and asserts no false invalidation and no missed change for edits that change size or content.

Caches live only in the cache root and are always deletable; a wrong key costs a rescan, never data.

### 2.4 Flatpak, Snap, Steam Deck and Proton notes

1. Flatpak Steam persists the whole home and grants `/mnt`, `/media` and `/run/media` (Flathub manifest read 2026-10-04). Libraries on other drives work; a custom folder under `~/Documents` may be invisible to the game process. RimStudio itself, when shipped as a Flatpak (after the first stable release), needs `--filesystem=~/.var/app/com.valvesoftware.Steam` and the same media grants; whether `sysinfo` sees host processes there is open.
2. Snap Steam uses the snap home; detection probes it and reports `unverified` confidence when found only through the snap path.
3. Steam Deck is native Linux (SteamOS, Arch based). Libraries on the SD card appear as extra `libraryfolders.vdf` entries, typically under `/run/media/...` (unverified). SD card file systems are often ext4 or exFAT, so the timestamp rules apply.
4. Proton is a secondary path: the existence of `compatdata/294100` alone is not a signal (an empty folder exists on the research machine); only the `pfx/.../LocalLow/...` path counts.
5. The Windows build of RimWorld under Proton keeps junction concerns away: the game then sees the Linux filesystem through Wine, and symlinks created natively are followed by Wine (unverified); the in-game test plan in section 9 therefore includes a Proton row.

## 3. Platform-code policy

### 3.1 Quarantine

`cfg(target_os)`, `cfg(windows)`, `cfg(unix)` and OS-specific dependencies are allowed only in:

| Location | What may live there |
| --- | --- |
| `crates/rimstudio-platform/src/{windows,macos,linux,unix}` | every implementation of a core port: registry probe, link backend (junction or symlink), process probe, launcher, credential store, sandbox probe, install-source probe, locale, volume class |
| `apps/desktop/src-tauri/src/window` | custom titlebar, window chrome, per-OS drag handling |
| `crates/rimstudio-steam-helper/src/lib_path.rs` | where Valve's native library is searched per OS |
| xtask packaging code | bundle and signing steps |

Everything else asks a port (trait objects in `rimstudio-core::ports`, injected through `AppContext`, decision D-009) or calls `Os::current()` when only a data decision is needed (for example, picking a candidate list). `cargo xtask check-cfg` greps the workspace and fails on any other occurrence; this is the enforcement of invariant I-11.

### 3.2 Ports

The ports relevant to this document are `RegistryProbe` (Windows registry reads, a stub elsewhere), `FsProbe` (stat, read_dir, exists with deadline, volume class), `EnvProbe` (environment variables, home and known folders), `ProcessProbe`, `LinkBackend`, `Launcher`, `SandboxProbe`, `InstallSourceProbe` and the umbrella `DetectEnv` consumed by `rimstudio-steam`. `rimstudio-steam` and `rimstudio-library::deploy` never import `std::os::*`.

### 3.3 How platform code is tested

1. Decision logic is pure and runs everywhere. Candidate ordering, VDF parsing, library enumeration, config-folder selection, link planning, cache-key comparison and the install-source decision are tested on Linux CI against `DetectEnv` fixtures built by `rimstudio-testing`: fake registry hives, fake home trees, fake process lists and a `RecordingFs` that records every write.
2. Fixture trees are generated at test time per OS (research fixture ids L1 to P1): native Linux, Flatpak, Snap, Proton, Windows with and without HKCU, macOS, plus the edge cases (lowercase `about/About.xml`, BOM, self-referencing symlink, path over 240 characters, a folder named `con` creatable only on Linux, NFD and NFC names, duplicate package id across roots, an offline custom folder). No real Steam or game data is committed (R11).
3. Thin OS implementations are exercised on their own leg. `rimstudio-platform` has `#[cfg(windows)]`, `#[cfg(target_os = "macos")]` and `#[cfg(target_os = "linux")]` integration tests that create real junctions, symlinks and registry values in temp locations; they run on the Windows, macOS and Linux CI legs respectively and nowhere else.
4. Golden outputs are JSON and must be identical at 1 and 8 threads and across the three OS legs, except for fields that are OS paths, which fixtures normalise.
5. Real-install tests are `#[ignore]` and enabled by an environment variable; they exist to compare fixtures against reality on the owner's machines.

## 4. Webview floor and startup feature probe

### 4.1 Minimum versions

| OS | Promise | Why |
| --- | --- | --- |
| Windows | Windows 10 and 11, x86_64 (ARM64 later) | WebView2 evergreen; Tailwind 4 needs a current Chromium; Windows 7 and 8 are not offered |
| macOS | 13.3 or newer, set explicitly as `bundle.macOS.minimumSystemVersion` (the Tauri default is 10.13) | Tailwind 4 needs Safari 16.4 level WebKit, which Ventura 13.3 ships |
| Linux | a distribution with WebKitGTK 2.50 or newer carrying `webkit2gtk-4.1` | current rolling and latest LTS releases; older packages are probed, not assumed |

### 4.2 Probe

`main.tsx` runs a synchronous feature probe before mounting: CSS `@layer`, `@property`, `color-mix()`, container queries and `:has()` through `CSS.supports`, plus a few JavaScript features the bundle relies on. On failure the app mounts a static "system webview too old" page that names the detected engine version (read through a tiny Rust command `app_webview_info`, the only command allowed before the probe passes), the minimum, and how to update (WebView2 runtime, macOS update, distribution package). The page is plain HTML and CSS with no framework so that it renders on the oldest engine. The probe result is also shown on the diagnostics page.

### 4.3 Startup order

Hidden window, painted shell, cached last-list snapshot, deferred scan (D-044 and the scan design). CI records time to first frame per OS and warns when it exceeds the budget in `xtask/budgets.jsonc`; the real numbers for WebView2 and WKWebView are not yet known and are collected in the first release cycle (section 12).

## 5. Linux graphics safe mode

WebKitGTK blank or garbled windows occur under some Wayland and GPU combinations. The policy is user controlled, never unconditional:

1. A JSONC launch file (`launch.jsonc` in the config root, created on demand, read before webview creation) may set graphics environment variables such as the WebKit compositing and DMABUF switches. Comments survive edits (CST edits, D-025).
2. `--safe-graphics` applies the documented software fallback set for that run.
3. A start marker (`webview-start.json` in the logs root, distinct from the session `crash-marker.json` of the error handling document) is written before webview creation and removed after the first painted frame. If the marker exists at the next start, a small native dialog (no webview needed) offers "start in safe graphics mode" and remembers the choice in the launch file.
4. The diagnostics page shows the webview engine version, session type (Wayland or X11) and the graphics variables in effect. DevTools exist only in a diagnostics build (Cargo feature).
5. The exact variable names are taken from the WebKitGTK documentation at implementation time and are not copied from this document (unverified here).

## 6. Packaging decisions

| Target | Artifact | Notes |
| --- | --- | --- |
| Windows x64 | NSIS installer, `installMode` per user (`currentUser`), `embedBootstrapper` for WebView2; portable zip | MSI not offered; portable zip uses `rimstudio.portable` |
| macOS arm64 and x64 | notarized, stapled dmg per architecture; `.app.tar.gz` for the updater | cross compile x64 on the arm runner rather than rely on Intel runners (being phased out, unverified) |
| Linux x64 | AppImage built on `ubuntu-22.04` (oldest glibc base), deb, rpm | Flatpak and AUR after the first stable release; arm64 second (`ubuntu-22.04-arm`, free for public repositories only) |
| Linux arm64 | AppImage, deb | later |
| Headless | `rimstudio-cli` archives per OS | same `rimstudio-app` handlers (D-007) |
| Sidecar | `rimstudio-steam-helper` binary in `binaries/` (git-ignored) | present only in releases that ship publishing; the app starts without it (invariant I-15) |

### 6.1 Portable mode

A file named `rimstudio.portable` next to the executable moves config, data, cache and logs under `./data`. The data-root resolver is in `rimstudio-io` (DataRoots) and receives the marker decision from `EnvProbe`; `rimstudio-core` never discovers directories (D-024). In portable mode the updater is inactive on Windows (the portable zip is replaced manually), and the data roots show their paths in Settings so users know what to copy. On macOS and Linux package formats the marker is ignored when the executable sits in a read-only location, and the app says so.

### 6.2 Install-source detection and updater

`InstallSourceProbe` (implemented in `rimstudio-platform`) returns one of `appimage`, `flatpak`, `snap`, `system-package`, `macos-app`, `windows-installer`, `portable` or `unknown`, from `APPIMAGE`, `FLATPAK_ID`, `SNAP`, the executable path (under `/usr` means a system package) and the marker file. The environment variable names are taken from the packaging research and flagged unverified until checked in each format's documentation. The decision function is pure and unit tested with fixtures; each format also has a manual check.

Updater rules:

1. GitHub Releases with `tauri-action` producing `latest.json`; stable and beta use different manifest URLs, built in Rust at start from the channel setting (runtime endpoint support is unverified).
2. AppImage updates only when the file is user-writable; deb, rpm, AUR, Flatpak and Snap hide "Install update" and show "Check for news" only.
3. macOS in `/Applications` owned by the user updates in place; protected installs fall back to the release page.
4. The updater never runs while an import, a deploy or a `ModsConfig.xml` write is in progress (a job-registry check).
5. No delta updates are assumed; payloads are a few MB.
6. No built-in rollback: keep previous installers on the release page, ship a "revert release" with a higher version number if needed, and keep data migrations forward compatible (D-026).
7. The updater key lives only in CI secrets plus one offline backup. Rotation is a bridging release signed by the old key that carries the new public key (a second pinned key is not supported as documented, unverified).

### 6.3 Signing

| OS | Path | Status |
| --- | --- | --- |
| macOS | Apple Developer ID, hardened runtime, notarization and stapling in the release job; opened on a clean account with Gatekeeper on as a release gate | required for a usable release |
| Windows | No code signing (D-087): builds are unsigned and the SmartScreen warning is documented as expected in the release notes and the user guide | decided |
| Linux | `SHA256SUMS` and build attestations; no distribution signing in v1 | automatic |
| Updater | Tauri signer key pair (minisign compatible, unverified) | owner holds the private key |

## 7. CI matrix

| Leg | Runner | Output | Gate content |
| --- | --- | --- | --- |
| Quality | `ubuntu-24.04` | none | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest` on the workspace, `cargo deny check`, `cargo xtask check-layers check-cfg check-deps check-licences check-docs check-tools check-size`, bindings and schema drift check, `pnpm lint`, `pnpm typecheck`, `pnpm vitest run`, Playwright with `mockIPC` |
| Windows x64 | `windows-latest` | NSIS, portable zip | platform integration tests (junction, registry); no sign step (D-087) |
| macOS arm64 | `macos-latest`, target `aarch64-apple-darwin` | dmg, `.app.tar.gz` | platform integration tests (symlink), notarize |
| macOS x64 | cross target on the arm runner | dmg | build and smoke only |
| Linux x64 | `ubuntu-22.04` | AppImage, deb, rpm | platform integration tests (symlink, inotify fallback), package smoke |
| Linux arm64 | `ubuntu-22.04-arm` | AppImage, deb | build only, public repository only |
| Nightly | all three OS | none | WebdriverIO smoke suite; the embedded WebDriver server sits behind an `e2e` Cargo feature that `xtask package` verifies is absent from release builds |

Further rules: third-party actions are pinned by commit hash; the release job writes `SHA256SUMS` and attestations; `cargo xtask check-tools` verifies the toolchain pins (Rust 1.96 channel, rust-version 1.95, Node 26, pnpm 11); the fixture builder generates per-OS Steam layouts on all three legs; `xtask bench-budgets` fails on a 2x regression; a time-to-first-frame record per OS warns above budget. Windows CI also builds with long paths on and runs the path-length fixture.

## 8. Steam Deck acceptance criteria

1. The default layout works at 1280 x 800, 100 percent scale, with a collapsible side panel and a mod list that stays usable at 1280 px wide.
2. A "compact touch" density gives primary controls at least 40 CSS pixels; no control is below 24 pixels in any density.
3. Every drag operation has a keyboard equivalent (invariant of D-054), which also serves a controller mapped to keyboard.
4. Desktop mode (KDE Plasma) behaves like any Linux desktop. In game mode (gamescope, no decorations) the app starts with `--fullscreen`, uses native dialogs, and does not depend on a window manager or a custom titlebar.
5. Install routes: Flatpak through Discover or an AppImage in the home folder; deb and rpm are not offered on the read-only root. The AppImage updater works.
6. A library on the SD card is detected, its filesystem class is probed, and an offline SD card shows the offline state rather than errors.
7. Safe graphics mode is reachable without a keyboard (the crash marker dialog).
8. Scan of about 700 mods stays inside the research budgets on the Deck within a factor chosen after the first measurement (the Deck number is not yet known).

## 9. In-game link-farm test plan (spike S-03)

The link farm ships only after this plan passes on each OS (D-039). The Mono directory test done on the research machine proved enumeration and `About.xml` reads through links but did not run the game's own runtime, so it is encouraging and not conclusive.

Preparation: a clean test mod folder `RimStudioLinkTest` outside the install with an `About/About.xml`, a Defs file adding a visible def (fictional content), a Textures file, an Assemblies-free design, and a patch; an active-list backup of `ModsConfig.xml`.

| Step | Procedure | Expected result |
| --- | --- | --- |
| 1 | Create a link (symlink on Linux and macOS, junction on Windows; also a symlink with Developer Mode on) from `<install>/Mods/RimStudioLinkTest__rsXXXX` to the test folder through RimStudio's dry-run then apply | the plan shows the link, the manifest records it |
| 2 | Start the game from RimStudio with the mod active | the mod appears in the Mods screen, loads without error, def, texture and patch effects visible in game, Player.log has no path error |
| 3 | Edit a file in the real folder, restart | the edit is live through the link |
| 4 | Relative path, spaces, `&` and brackets in the link name | all load |
| 5 | Steam "verify integrity of game files" with links present | record whether links survive; either result is acceptable if the next apply restores them |
| 6 | Steam update of the game (simulated by deleting a manifest file) | same |
| 7 | Take the external drive offline, launch via RimStudio | pre-launch check blocks and lists the ids; with the check bypassed manually, record that the game deactivates the id (confirms the premise) |
| 8 | Deactivate the mod, apply | link removed with unlink only; the real folder is intact (file count and hash compared before and after) |
| 9 | Delete the link by hand, apply again | the manifest detects the missing link and restores or reports it |
| 10 | Antivirus on (Windows Defender, macOS XProtect) | no quarantine of linked content; record scan time |
| 11 | Per-OS extras below | see below |

Per-OS extras:

1. Windows: link to a second local volume; link to a UNC path must fall back to copy and say so; a junction whose drive letter changed shows as broken and is never retargeted; recursive-delete behaviour of the Rust unlink on a junction is tested on a directory containing a sentinel file.
2. macOS: Mods inside `RimWorldMac.app`; creating the link as a normal user, with App Management denied, then granted; Steam behaviour on files added to the bundle; a quarantined (translocated) app.
3. Linux native: external ext4 and exFAT target; Mods folder read-only.
4. Flatpak Steam: target under `/run/media` (expected to work), under `~/Documents` (expected to fail), with and without an added `--filesystem` override.
5. Steam Deck: SD card target in desktop mode and game mode.
6. Proton: game started through Proton with a natively created symlink.

Exit criteria: the mod loads in all rows marked "must work" (steps 1 to 4 and 8 on every OS), the copy fallback is exercised on each OS at least once, and the results table is committed under `docs/research/data/` (as JSON) with the OS build numbers.

## 10. Manual test matrix

Run for every release candidate; the first four columns are blocking.

| Area | Win 10 | Win 11 | macOS 13 | macOS latest | Ubuntu LTS | Fedora | Arch | Steam Deck |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Install and first launch (formats: NSIS and zip; dmg; deb and AppImage; rpm; AUR and AppImage; Flatpak or AppImage) | x | x | x | x | x | x | x | x |
| Auto detect game and Workshop | x | x | x | x | x | x | x | x |
| External drive custom folder (TCC prompt on macOS, SD card on Deck) | x | x | x | x | x | x | x | x |
| Scan 700 mods, memory and time | x | | | x | x | | | x |
| Update from the previous release | x | | | x | x (AppImage) | | | x |
| Running game warning | x | | x | | x | | | x |
| Uninstall leaves user data | x | | x | | x | | | x |
| Link farm plan from section 9 | x | | x | | x | | | x |
| Webview probe (old engine shows the notice) | x | | x | | x | | | |
| Safe graphics mode | | | | | x | | x | x |
| Windows long path and reserved name fixtures | x | | | | | | | |
| Unsigned or signed install warnings recorded | x | | x | | | | | |

## 11. Risk register

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Game does not follow links on some OS | link farm unusable there | S-03 gate; copy fallback is always available |
| Steam verify removes links | missing mods at launch | ownership manifest restores; pre-launch check |
| macOS refuses writes in the bundle | no custom folders on macOS | explicit instruction flow; document as a known limit |
| Unsigned Windows build | SmartScreen friction | accepted (D-087); documented as expected in the release notes and user guide |
| Old system webview | broken UI | startup probe notice |
| WebKitGTK graphics bugs | blank window | safe graphics and crash marker |
| Antivirus slows scans | poor first run | lazy reads, cache, exclusion tip |
| Transitive XML users in Tauri tree | boundary rule noise | cargo-deny wrappers (spike S-09) |

## 12. Verified versus unverified

Verified on the Linux Arch research machine (Rust 1.96, Node 26): Linux Steam layout and symlinks, the user config folder and Player.log location, workshop scan of 691 mod folders and the two-phase scan numbers, VDF parser behaviour on 28 real fixtures, inotify limits, Mono directory enumeration through symlinks, loops and dangling links, btrfs removable volume layout, tooling versions.

Unverified, needing a real machine (tracked as release-cycle tasks):

| Item | Needs |
| --- | --- |
| Windows registry keys (HKCU `SteamPath`), per-user Steam installs, junction privileges and UNC behaviour, `longPathAware` mechanism in the Tauri build, Defender scan cost | Windows 10 and 11 |
| macOS user data path, Player.log locations, App Management behaviour inside `RimWorldMac.app`, Steam reaction to added files, TCC behaviour, `--target universal-apple-darwin` handling, notarization | a Mac |
| Flatpak and Snap config folder choice, process visibility in sandboxes, grants | Flatpak and Snap installs |
| Steam Deck paths, gamescope behaviour, SD card mount path | a Deck |
| Exact RimWorld process names per OS, Proton layout | real installs |
| WebView2 and WKWebView start time, memory, IPC ratios | Windows and macOS lab runs |
| Env variable names for install-source detection, WebKitGTK graphics variables | format documentation |
| Game loading assets through links | S-03 on each OS |

## 13. Owner decisions

1. macOS notarisation path; the Windows question is closed by D-087 (unsigned).
2. App identifier (placeholder `app.rimstudio.desktop`), D-049, fixes data directories on every OS and update continuity.
3. Whether the first release ships macOS custom-folder support given the bundle write risk, or marks it experimental until a Mac test passes.
4. Access to Windows and macOS machines (or CI-only evidence) for sections 9 and 10.
