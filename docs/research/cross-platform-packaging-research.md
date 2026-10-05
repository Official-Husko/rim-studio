# RimStudio cross-platform packaging, signing, updating and testing

Scope: how to build, package, sign, update and test RimStudio (Tauri 2, Rust workspace, Vite, Preact, pnpm) on Windows, macOS and Linux, how the two reference managers ship, and which operating system behaviours change the logic of a RimWorld tool. Evidence comes from the Tauri documentation repository (v2 branch, downloaded 2026-10-04 as a tarball and read locally), Tauri source files on raw.githubusercontent.com, crates.io and GitHub release feeds, vendor documentation fetched the same day, the decompiled game code, the real workshop folder on the research machine, and the two reference repositories. No Tauri app was built for this note; performance numbers that are not from the project's own earlier lab are third-party and marked as such.

Status: research note | Last verified: 2026-10-04

Conventions: "(unverified)" marks a claim not checked against a primary source in this session. Version numbers are the latest stable on 2026-10-04 from crates.io or the GitHub releases feed. Paths in the repository are relative to the repository root.

## 1. Build prerequisites

### 1.1 Tauri and tool versions on 2026-10-04

| Item | Latest stable | Source |
| --- | --- | --- |
| `tauri` crate | 2.12.1 (published 2026-10-01) | crates.io API |
| `tauri-build` | 2.7.1 | crates.io API |
| `tauri-cli` | 2.12.1 | crates.io API |
| `tauri-bundler` | 2.10.1 | crates.io API |
| `tauri-plugin-updater` | 2.13.1 (2026-09-30) | crates.io API |
| `tauri-plugin-single-instance` | 2.5.2 | crates.io API |
| `tauri-plugin-process` | 2.4.0 | crates.io API |
| `tauri-plugin-dialog` | 2.8.1 | crates.io API |
| `tauri-driver` | 2.1.0 (2026-09-26) | crates.io API |
| `sysinfo` | 0.39.6 | crates.io API |
| `tauri-apps/tauri-action` | v1.0.0 | GitHub releases feed |

Keep the `tauri` crate, `tauri-cli` and the `@tauri-apps/*` npm packages on the same minor version; the plugin crates version independently.

### 1.2 Linux system packages (from the Tauri prerequisites page)

Tauri 2 needs WebKitGTK 4.1 (not 4.0, which Tauri 1 used). Exact package names on the page `start/prerequisites.mdx`:

| Distribution | Packages |
| --- | --- |
| Debian, Ubuntu | `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` |
| Arch | `webkit2gtk-4.1 base-devel curl wget file openssl libappindicator-gtk3 librsvg xdotool` (the page lists one more line, an appmenu module, that was not captured) |
| Fedora | `webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel` plus the group `c-development` (a further package line, believed to be the xdo development library, was not captured; unverified) |
| openSUSE | `webkit2gtk3-devel libopenssl-devel curl wget file libappindicator3-1 librsvg-devel` plus pattern `devel_basis` |
| Alpine | `build-base webkit2gtk-4.1-dev curl wget file openssl libayatana-appindicator-dev librsvg` and a font package; static libraries needed because of musl |

Notes that matter for RimStudio:

1. `libsoup-3.0` is not in the prerequisite lists: it is a dependency of WebKitGTK 4.1. The Tauri AUR guide lists the runtime dependencies `cairo desktop-file-utils gdk-pixbuf2 glib2 gtk3 hicolor-icon-theme libsoup pango webkit2gtk-4.1` (docs `distribute/aur.mdx`). The Debian name for the development package that provides it (`libsoup-3.0-dev`) is pulled in transitively (unverified).
2. `patchelf` is not in the prerequisite page but appears in the official GitHub Actions example (`libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xdg-utils`) and in the container example (`... libgtk-3-dev ... patchelf libfuse2 file`), docs `distribute/Pipelines/github.mdx`. Treat `patchelf`, `xdg-utils` and `libfuse2` (needed to run AppImage tooling) as CI packages for the Ubuntu runner.
3. The AppImage tool chain downloads `linuxdeploy` and an `AppRun` binary from GitHub at bundle time (tauri-bundler `linux/appimage/linuxdeploy.rs`), so Linux release builds need network access, and a pinned base image should be used (section 2.1).
4. Tauri's tray feature needs `libayatana-appindicator`; RimStudio does not need a tray icon for a mod manager, so the tray feature stays off and that dependency can be dropped from the runtime requirement list (decision for section 9).

### 1.3 Windows

1. Microsoft C++ Build Tools with the "Desktop development with C++" workload (MSVC toolchain `x86_64-pc-windows-msvc`).
2. WebView2 runtime: needed to run and to develop; the docs say it ships with Windows 11 and the Tauri installer ensures it on older systems.
3. ARM64 needs the extra component "MSVC v143 ARM64 build tools" and the Rust target `aarch64-pc-windows-msvc`; the NSIS installer itself stays x86 under emulation while the app is native ARM64 (docs `distribute/windows-installer.mdx`).
4. MSI bundles can only be built on Windows (WiX) and need the VBSCRIPT optional feature; NSIS can be cross-built from Linux or macOS with caveats (same page). RimStudio builds on Windows runners, so this is only relevant to a developer on Arch.

### 1.4 macOS

Xcode (full or Command Line Tools) per the prerequisites page. Build both `aarch64-apple-darwin` and `x86_64-apple-darwin` targets on an Apple Silicon runner or use the separate Intel runner as RimSort does (`RimSort-main/.github/workflows/build.yml` uses `macos-15-intel` and `macos-latest`).

### 1.5 Minimum operating system versions

| OS | What the Tauri docs say | What RimStudio should promise |
| --- | --- | --- |
| Windows | "Windows 7 and later" (prerequisites page); MSI does not work on Windows 7 unless the WebView2 bootstrapper is embedded; NSIS supports `downloadBootstrapper` there | Windows 10 and 11 only. Windows 7 and 8 are not realistic: Tailwind 4 needs a current Chromium and the old WebView2 line for those systems is frozen (unverified). RimWorld itself is the practical audience on Windows 10 or later. |
| macOS | "macOS Catalina (10.15) and later" (prerequisites page); `bundle.macOS.minimumSystemVersion` defaults to 10.13 (`tauri-utils` `config.rs`) | macOS 13.3 or newer, set explicitly, because Tailwind 4 needs Safari 16.4 level WebKit; the Tauri webview table maps Ventura 13.3 to WebKit 615.1.26 and Safari 16.4 (already recorded in `docs/research/webview-and-ipc-performance.md` section 4.1) |
| Linux | any distribution carrying `webkit2gtk-4.1` | A distribution with WebKitGTK 2.50 or newer (current rolling and latest LTS releases, table in the webview note); probe features at runtime and show an "outdated system webview" screen |

Architectures: Windows x86_64 (ARM64 optional, later), macOS aarch64 and x86_64, Linux x86_64 first and aarch64 second (the official workflow includes `ubuntu-22.04-arm`, free only for public repositories, docs `Pipelines/github.mdx`).

## 2. Bundle targets and trade-offs

The bundle types the CLI knows are `deb`, `rpm`, `appimage`, `msi`, `nsis`, `app` and `dmg` (enum `BundleType` in `tauri-utils` `config.rs`). There is no portable or Flatpak target in Tauri itself; those are built outside.

### 2.1 Linux

| Format | Pros | Cons | Verdict |
| --- | --- | --- | --- |
| AppImage | One file, runs on most distributions, supported by the updater (the updater artifact is the AppImage itself plus a `.sig`, or a `.tar.gz` in `v1Compatible` mode, docs `plugin/updater.mdx`); fits Steam Deck desktop mode | Docs say size grows from the 2 to 6 MB range to 70+ MB; glibc rule: build on the oldest base system that still has WebKitGTK 4.1 (Ubuntu 22.04 is the docs' runner); needs FUSE 2 on some systems; bundles GStreamer only if `bundleMediaFramework` is set (leave off) | Primary direct download for Linux, built on `ubuntu-22.04` |
| deb | Small, uses the system WebKitGTK, `depends` and `recommends` configurable in `bundle.linux.deb` | Debian family only; no updater support (the updater docs describe only AppImage on Linux); must be built on an old enough base | Offer on the release page |
| rpm | Same for Fedora and openSUSE; configurable `bundle.linux.rpm` | Same; separate build per base ABI | Offer on the release page |
| Flatpak | Sandboxed, auto-updating through the store, good for Steam Deck Discover | Sandbox hides host folders (below); Tauri's Flathub guide uses runtime `org.gnome.Platform` 47 with `--socket=wayland`, `--socket=fallback-x11`, `--device=dri`, `--share=ipc`; no self-update; DBus-based single instance needs extra permissions (docs `plugin/single-instance.mdx`) | Second wave, after AppImage works. Ship a manifest in the repo, publish to Flathub only when the sandbox model is proven |
| AUR | Reaches Arch users with a tiny PKGBUILD; Tauri docs give three templates (`-bin`, source, git) | Maintained by hand; depends on the user's system WebKitGTK | Publish a `rimstudio-bin` package that repackages the release deb or AppImage after the first stable release (unverified: package name availability) |
| Snap | Documented (`distribute/snapcraft.mdx`) | Confinement, slow start on some systems | Skip |

Flatpak and Steam folders. The Steam Flatpak keeps its data in `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` and cannot see the real home; external libraries must be granted with `flatpak override --user --filesystem=/path com.valvesoftware.Steam` (search results from the Arch wiki and Flathub wiki, 2026-10-04; the existing detection note `docs/research/steam-and-game-detection.md` section 3.2 row 6 records the same probe path). A Flatpak RimStudio therefore needs, at minimum (all unverified until tested in a sandbox):

1. `--filesystem=xdg-data/Steam` and `--filesystem=~/.steam` for native Steam; `--filesystem=~/.var/app/com.valvesoftware.Steam` for Flatpak Steam, which Flatpak may refuse because `~/.var/app` is special (unverified; test).
2. `--filesystem=xdg-config/unity3d` read-write, because RimStudio writes `ModsConfig.xml` there.
3. `--filesystem=/run/media` and `--filesystem=/mnt` for external drives like the owner's custom mod folder (Linux mounts under `/run/media/<user>/<label>`).
4. The sandbox has its own process namespace, so a running RimWorld on the host is probably invisible to `sysinfo` (unverified); the app must treat "cannot see process" as "unknown" and not as "not running".
5. Path remapping: inside the sandbox the host path `/home/<user>/.local/share/Steam` appears at the same string only when granted by `xdg-data`; a path stored in settings by a non-Flatpak build can break. Store paths with a "canonical form" and a "root kind" (see section 9) and re-resolve at start.

### 2.2 Windows

| Choice | Facts | Recommendation |
| --- | --- | --- |
| NSIS (`-setup.exe`) | `installMode` is `currentUser` by default (installs without administrator rights, metadata under HKCU), `perMachine` (Program Files, HKLM, needs administrator) or `both` (always needs administrator, user chooses); can be built on Linux or macOS; supports `downloadBootstrapper` on Windows 7 (`tauri-utils` `NSISInstallerMode`, docs `windows-installer.mdx`) | Primary installer, `currentUser`, so the updater can replace the app without a UAC prompt |
| MSI (WiX v3 in Tauri) | Windows-only build, FIPS option via `TAURI_BUNDLER_WIX_FIPS_COMPLIANT`, VBSCRIPT feature required, per-machine oriented | Offer only if enterprise users ask; skip in v1 |
| Portable zip | Not a Tauri target; build by zipping the release exe plus the marker file (section 2.4) | Offer: modders often want a folder on an external drive |

WebView2 install modes (docs `windows-installer.mdx`, table):

| Mode | Needs internet | Added size | Use |
| --- | --- | --- | --- |
| `downloadBootstrapper` (default) | yes | 0 MB | Default; Windows 10 and 11 already have the runtime |
| `embedBootstrapper` | yes | about 1.8 MB | Better for odd systems |
| `offlineInstaller` | no | about 127 MB | Special offline build only |
| `fixedVersion` | no | about 180 MB | Not worth it |
| `skip` | no | 0 MB | Not recommended |

Choose `embedBootstrapper` for the NSIS build (1.8 MB is cheap insurance against a blocked download) and `skip` for nothing.

### 2.3 macOS

1. Ship a `.dmg` containing the `.app`; the updater uses the `.app.tar.gz` plus signature (updater docs, createUpdaterArtifacts section).
2. Set `minimumSystemVersion` to 13.3 (section 1.5). Build either two thin bundles (the official workflow builds `aarch64-apple-darwin` and `x86_64-apple-darwin` separately) or a universal bundle through `--target universal-apple-darwin` (target name unverified in this session). Two thin bundles halve the download; one universal file is simpler for users. Decision: two files named by architecture, because Apple Silicon is the bulk of current Macs (unverified share) and update manifests have separate `darwin-aarch64` and `darwin-x86_64` keys anyway.
3. RimWorld's Mods folder on macOS. The decompiled code computes Mods as the parent of Unity's data path plus `Mods` (decompiled:Verse/GenFilePaths.cs, `GetOrCreateModsFolder`). Unity documents `Application.dataPath` on macOS players as the player bundle path plus `/Contents` (https://docs.unity3d.com/ScriptReference/Application-dataPath.html, accessed 2026-10-04), so the parent is the app bundle itself: Mods lives at `.../RimWorld/RimWorldMac.app/Mods`, which matches what RimSort's validator expects (recorded in the detection note section 3.4). The official expansions follow the same rule at `RimWorldMac.app/Data`. Consequences:
   - Steam libraries on macOS live in `~/Library/Application Support/Steam`; the game bundle is a normal app inside the library. Reading it needs no special permission for the user's own account in my understanding (unverified).
   - Writing or removing folders inside another app's bundle falls under macOS "App Management" protection introduced in Ventura: an app that modifies another app's bundle and is not signed by the same team is blocked unless the user enables it in Privacy and Security, App Management (lapcatsoftware.com article and Apple developer forum thread, accessed 2026-10-04). A bundle placed by Steam may or may not be covered (unverified).
   - Steam validates and may rewrite game files, and a game update replaces the bundle: anything RimStudio puts in `RimWorldMac.app/Mods` (copies, symlinks) can be wiped or flagged.
   - Link farms (a folder of symlinks to enabled mods) into the bundle's Mods folder are therefore fragile on macOS. The safe design is that RimStudio never writes mod content into the game folder: it manages the active list in `ModsConfig.xml` (in the user config folder, outside the bundle) and points the game at custom folders through that file, as the detection note section 8.6 describes. If a link farm is ever offered, make it an explicit opt-in per OS and handle the App Management refusal with an actionable message.
   - TCC: Documents, Desktop, Downloads, removable volumes and network volumes need per-app consent (Apple Support page "Control access to files and folders on Mac", accessed 2026-10-04). A custom mods folder on an external drive on macOS triggers the removable volume prompt on first scan; the denial surfaces as a permission error and must be reported as such, not as "folder empty".
4. Info.plist can be extended with usage strings in `src-tauri/Info.plist`, merged with the generated file (docs `macos-application-bundle.mdx`); the app does not need camera or contacts keys.

### 2.4 Portable mode marker

Tauri has no portable concept; RimStudio defines one in its own code:

1. If a file named `rimstudio.portable` (name proposal) exists next to the executable, all app-owned data (JSONC settings, JSON caches, datasets) lives in a `data/` folder beside the executable and the OS config and cache folders are not touched.
2. Resolve the data root in one function in the `rimstudio-` core crate before the first read, expose the chosen mode on the diagnostics page.
3. Portable mode disables the updater on Windows (the updater runs the installer, and the app exits when the install step runs, docs `plugin/updater.mdx`) and offers "download new zip" instead.
4. On AppImage the executable directory is a read-only mount; put the marker next to the `.AppImage` file and read the `APPIMAGE` environment variable (set by the AppImage runtime, unverified) to find it, not `current_exe`.

## 3. Signing, trust and updates

### 3.1 Windows

| Fact | Evidence |
| --- | --- |
| Signing is not required to run, but without it a browser-downloaded installer shows a SmartScreen warning | Tauri docs `distribute/Sign/windows.mdx` |
| Since 2024 an EV certificate no longer gives instant SmartScreen reputation: Microsoft removed that special treatment, so EV and OV both build reputation over time | same page |
| The Tauri OV guide applies to certificates issued before 2023-06-01; newer OV and EV certificates are held on hardware or cloud HSMs and signing goes through the issuer's tools | same page |
| Tauri can call a custom sign command, or `signtool` with `certificateThumbprint`, `digestAlgorithm` and `timestampUrl`; the page also documents Azure Key Vault signing through `relic` | same page |
| Azure Artifact Signing (the successor name of Trusted Signing): certificates are renewed daily and valid for 24 hours, signatures are timestamped; generally available in the USA, Canada and Europe for organizations; individual developers limited to the USA and Canada; since April 2026 self-employed individuals can apply without the old three year history requirement; SmartScreen warnings still appear until reputation builds | Microsoft Learn Q&A, devclass.com article and the melatonin.dev write-up found by search, accessed 2026-10-04 (secondary, re-check before applying) |

Recommendation: ship v1 unsigned only as a pre-release with a documented SmartScreen explanation; sign the first public stable release. For an individual outside the USA and Canada, the realistic options are an OV certificate from a commercial authority with cloud signing, or signing through a company. Sign every release with the same identity, because reputation attaches to the signer and the file hash. Unsigned installers also get more antivirus false positives (unverified), which compounds the mass-scan issue in section 6.

### 3.2 macOS

1. A free Apple developer account cannot notarize; the app then stays "not verified" (Tauri docs `Sign/macos.mdx`). A paid account is needed for a `Developer ID Application` certificate (only the Account Holder can create it).
2. Environment variables for CI: `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`; notarization via either `APPLE_ID` plus `APPLE_PASSWORD` or the App Store Connect API trio `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH` (same page). Prefer the API key, which is not tied to a person's password.
3. Without a certificate, use ad-hoc signing: the pipelines page says it can avoid Apple Silicon builds downloaded from GitHub being treated as damaged.
4. Entitlements are applied at signing time (`macos-application-bundle.mdx`). RimStudio needs the hardened runtime and, in my understanding, no extra entitlements because the system WebKit does the JIT work in its own process (unverified). Do not enable the App Sandbox: a sandboxed app cannot read Steam libraries and arbitrary mod folders without security-scoped bookmarks.
5. Notarization and stapling must be part of the release job and tested on a clean macOS account with Gatekeeper enabled.

### 3.3 Tauri updater

| Topic | Verified behaviour (docs `plugin/updater.mdx`) |
| --- | --- |
| Keys | `tauri signer generate` creates a key pair; the public key goes into `plugins.updater.pubkey` (content, not a path); the private key goes into `TAURI_SIGNING_PRIVATE_KEY` and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `.env` files do not work; losing the private key means installed apps can never be updated again. The signature scheme is minisign compatible (unverified here; the docs call the artifact files `.sig`) |
| Artifacts | `bundle.createUpdaterArtifacts: true`; Linux: the AppImage and `.AppImage.sig`; Windows: the installer and its `.sig`; macOS: `.app.tar.gz` and `.sig`; `"v1Compatible"` produces zipped variants for apps migrating from Tauri 1 |
| Endpoints | array of HTTPS URLs; TLS is enforced in release builds; the client tries the next URL only on a non-2xx status; variables `{{current_version}}`, `{{target}}` (linux, windows, darwin), `{{arch}}` (x86_64, i686, aarch64, armv7); no custom variables, but a custom target string is allowed |
| Manifest | static JSON with `version`, and `platforms.<os>-<arch>.url` and `.signature` required; tauri-action generates `latest.json` and uploads it to the release, so `https://github.com/<owner>/<repo>/releases/latest/download/latest.json` works as the endpoint |
| Dynamic server | the server can answer with the update or 204; the version comparison can be overridden |
| Windows | `installMode` `passive` (default, small progress window), the app exits when the installer starts |
| Restart | `relaunch()` from `plugin-process`, or `app.restart()` in Rust; restarting immediately is optional |
| Delta updates | the docs describe only whole-artifact downloads; no delta or differential mechanism is mentioned (searched the updater page and all `distribute/*.mdx`). Treat delta updates as not available. RimStudio's payload is small (a few MB compressed) so full downloads are acceptable |
| Downgrade or rollback | no built-in rollback. Mitigation: keep the previous installers on the release page, publish a corrected `latest.json` that points at a higher version number containing the old code (a "revert release"), and keep user data migrations forward compatible (settings carry a `schemaVersion`) |

Channels. Use one manifest URL per channel: stable `.../releases/latest/download/latest.json` (GitHub's "latest" skips pre-releases) and beta from a fixed tag such as `.../releases/download/channel-beta/latest.json`, a rolling release whose assets CI replaces (design, unverified). The channel is a setting in the app, and the endpoint list is built in Rust at start from it (`UpdaterBuilder` accepts endpoints at runtime, unverified).

Read-only locations:

1. AppImage: the updater replaces the AppImage file, so the file must be writable by the user; if the directory is not writable, or the file is on a read-only mount, show "download manually".
2. deb, rpm, AUR, Flatpak, Snap: the package manager owns updates. The app must detect its install source (environment variable `APPIMAGE` present means AppImage, `FLATPAK_ID` present means Flatpak, the executable under `/usr` means a system package; unverified variable names) and hide "Install update", offering only "Check for news".
3. macOS app in `/Applications` owned by the user updates fine; an app installed by an administrator into a protected path needs a prompt; fall back to the release page.
4. Never run the updater while an import or a write to `ModsConfig.xml` is in progress.

Supply chain: the update key must live only in CI secrets and one offline backup; a second pinned public key for rotation is not supported by the plugin as documented (unverified), so rotation means a bridging release signed by the old key that carries the new `pubkey`.

## 4. CI/CD

### 4.1 Reference managers

| Aspect | RimSort | RimCrow |
| --- | --- | --- |
| Packaging tool | Nuitka standalone build driven by `distribute.py` and a `justfile`; Windows MSI through WiX 7 (`packaging/msi/RimSort.wxs`), Linux AppImage through `packaging/linux/build-appimage.sh` with a desktop file and AppStream metainfo, macOS `.app` optimised by `packaging/optimize_macos_bundle.py` | Nuitka (`pack_nuitka.py`, modes `onefile` and `standalone`, optional Windows splash image) or PyInstaller (`pack_pyinstaller.py`); output is a zip named `<app>-v<version>-<platform tag>.zip` |
| Matrix | macOS Intel (`macos-15-intel`), macOS Arm (`macos-latest`), Ubuntu 22.04, Ubuntu 24.04, Windows latest, all `x86_64` except one macOS leg (`.github/workflows/build.yml`) | No workflow directory in the repository snapshot (no `.github` folder found) |
| Trust | Build provenance attestations for the Windows MSI and the AppImage; no signing step found for Windows or macOS in the workflow (searched for sign, notar, codesign) | None visible |
| Platform support promise | Windows, macOS, Linux | README table: Windows is the main platform and needs WebView2; macOS has basic path support and packaging quality is not promised; Linux is partial, with desktop run, the Steamworks library and packaging not guaranteed (`RimCrow-main/README.md`, platform table) |

Lessons: RimSort's multi-OS matrix with an old Ubuntu leg for glibc compatibility and attestations is the model; RimCrow shows the cost of promising less than the market wants (Linux and macOS users are a large part of the RimWorld audience). RimStudio's Rust and Tauri build has no interpreter to freeze, so its artifacts are smaller and its start faster (numbers in section 8).

### 4.2 Action versions on 2026-10-04 (GitHub releases feeds)

`actions/checkout` v7.0.1, `actions/setup-node` v7.0.0 (the Tauri docs example still shows v6), `pnpm/action-setup` v6.1.0, `Swatinem/rust-cache` v2.9.2, `actions/upload-artifact` v7.0.1, `actions/attest-build-provenance` v4.2.2, `EmbarkStudios/cargo-deny-action` 2.1.1 (cargo-deny 0.20.2), `rustsec/audit-check` v2.0.0, `tauri-apps/tauri-action` v1.0.0. The Tauri pipeline guide uses `dtolnay/rust-toolchain@stable` and `tauri-apps/tauri-action@v1` with `tagName: app-v__VERSION__` and `releaseDraft: true`. Pin third-party actions by commit hash in the real workflow, as RimSort does for its release action.

### 4.3 Matrix

| Leg | Runner | Target args | Output | Notes |
| --- | --- | --- | --- | --- |
| Windows x64 | `windows-latest` | none | NSIS setup exe, zip portable | sign step before upload |
| macOS arm64 | `macos-latest` | `--target aarch64-apple-darwin` | dmg, `.app.tar.gz` | notarize |
| macOS x64 | `macos-15-intel` (RimSort uses it) or `macos-latest` cross target | `--target x86_64-apple-darwin` | dmg | Intel runners are being phased out by GitHub (unverified), so prefer cross compiling on the arm runner |
| Linux x64 | `ubuntu-22.04` | none | AppImage, deb, rpm | oldest base for glibc |
| Linux arm64 | `ubuntu-22.04-arm` | none | AppImage, deb | free for public repositories only |
| Quality gate | `ubuntu-24.04` | none | none | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `cargo deny check`, `pnpm lint`, `pnpm typecheck`, `pnpm vitest run` |

### 4.4 Caching, naming, checksums, gates

1. Cargo: `Swatinem/rust-cache` keyed per target; set `workspaces: ". -> target"` style input only if the Tauri crate is not at the root (input name unverified). pnpm: `pnpm/action-setup` then `actions/setup-node` with `cache: pnpm`.
2. Artifact names: `rimstudio-<version>-<os>-<arch>.<ext>` (for example `rimstudio-0.1.0-windows-x64-setup.exe`); Tauri's default names differ, so rename in a final job, and keep names stable because `latest.json` URLs reference them.
3. Checksums: a final job downloads all artifacts, writes `SHA256SUMS` and attaches it to the release; attestations with `attest-build-provenance` for each installer.
4. `cargo deny` (licences including a deny on GPL and CC BY-NC-SA for dependencies, bans, advisories, sources) and `cargo audit` on each pull request and weekly on schedule. This also serves R11: CI should fail if a dependency licence is incompatible.
5. A repository check that no file under `docs/` or the test fixtures exceeds 1.5 MB and no RimSort, RimCrow or Combat Extended data is present (R11).
6. Version bump: one source of truth. Put the version in `[workspace.package]` of the root `Cargo.toml`, set `"version"` in `tauri.conf.json` to `../package.json` is supported by Tauri (the config accepts a path to a package.json for `version`; unverified in this session), and a small script `scripts/bump-version` (Rust xtask, no new runtime) updates `package.json`, `Cargo.toml` and checks that all three agree; a CI step fails on mismatch. The tag `v<version>` triggers the release workflow.

### 4.5 Workflow outline (not a verified YAML, structure only)

1. Trigger on a tag `v*`; permissions `contents: write`, `id-token: write`, `attestations: write`.
2. Job `gate` as in the matrix.
3. Job `build` with `strategy.matrix.include` of the legs above, `fail-fast: false`; steps: checkout, install Linux packages (section 1.2 plus `patchelf xdg-utils`), pnpm setup, node setup with pnpm cache, Rust toolchain with the macOS targets only on macOS legs, rust-cache, `pnpm install --frozen-lockfile`, then `tauri-apps/tauri-action@v1` with `tagName: v__VERSION__`, `releaseDraft: true`, `args` from the matrix; secrets: `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, the Apple variables and the Windows signing secrets.
4. Job `publish` (needs `build`): checksums, attestations, rename, then flip the draft release to published after a manual approval environment.

The skeleton is deliberately prose: the exact `with:` inputs of `tauri-action` v1 were not re-verified against its README in this session (only the pipeline page in the docs repository was read).

## 5. Cross-platform testing

### 5.1 Layers

| Layer | Tool | Runs on | What it proves |
| --- | --- | --- | --- |
| Rust unit and property tests | `cargo test`, `proptest` (unverified choice) | all OS legs | parsers, rule merging, path logic |
| Fixture tree tests | synthetic trees built in a temp dir by a helper crate | all OS legs | detection and scanning on per-OS layouts |
| Frontend unit | vitest with jsdom or happy-dom | one Linux leg | store logic, formatting, sort helpers |
| Frontend integration | Playwright against the Vite dev server with `mockIPC` from `@tauri-apps/api/mocks` | one Linux leg (Chromium), one macOS leg for WebKit (Playwright ships a WebKit build, unverified) | UI behaviour without Rust |
| End to end | WebdriverIO with `@wdio/tauri-service` | Windows, Linux, macOS | the real app |
| Performance | a Rust bench binary plus the IPC lab script | one runner per OS family | budgets of `webview-and-ipc-performance.md` section 5 |
| Manual matrix | checklist (section 5.5) | real machines, a Steam Deck | everything the CI cannot |

### 5.2 Synthetic Steam layouts per OS

Build trees from code at test time, not from checked in copies of real data (R11 and size). A `fixtures` builder takes an OS profile and a root and creates:

| Profile | Layout to synthesize |
| --- | --- |
| Linux native | `~/.local/share/Steam/steamapps/{libraryfolders.vdf, appmanifest_294100.acf, common/RimWorld/{Data/Core, Mods}, workshop/content/294100/<id>}`, `~/.steam/steam` symlink, `~/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config/ModsConfig.xml` |
| Linux Flatpak Steam | the same under `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` and a second library on a mount |
| Linux with Proton | prefix `compatdata/294100/pfx/drive_c/users/steamuser/AppData/LocalLow/...` |
| Windows | `C:\Program Files (x86)\Steam\...` and `D:\SteamLibrary` expressed with a path abstraction so Linux CI can test the logic; real Windows leg runs the same tests against a temp dir |
| macOS | `~/Library/Application Support/Steam/steamapps/common/RimWorld/RimWorldMac.app/{Data,Mods}` and `~/Library/Application Support/RimWorld/Config` |
| Edge cases | lowercase `about/About.xml` variants, a mod with BOM, a self-referencing symlink, a long path (over 240 characters), a Windows reserved name folder (`con`, only creatable on Linux), NFD and NFC names, a duplicate package id across roots, an offline custom folder |

The detection note (`docs/research/steam-and-game-detection.md` section 7.4) already plans this fixture builder; this note adds the per-OS variants above and requires the builder to run on all three CI OS legs.

### 5.3 End to end status

| Platform | Status (Tauri docs `develop/Tests/WebDriver`) |
| --- | --- |
| Overview | The recommended route is WebdriverIO with `@wdio/tauri-service`, which works on Windows, Linux and macOS; by default it runs an embedded WebDriver server inside the app, so no external driver is needed on any platform, and that is how macOS is supported |
| `tauri-driver` (2.1.0) | Wraps the native driver on Windows (Edge WebDriver) and Linux (`WebKitWebDriver`, package name `webkit2gtk-driver` on Debian, unverified); the docs state there is no official macOS driver because WKWebView has none; CrabNebula's fork supports macOS but needs a paid key |
| CI | WebDriver tests on Linux run under a fake display (`xvfb`), and GitHub Actions also supports Windows runs |
| Mocking | `mockIPC` intercepts `invoke`; call `clearMocks()` after each test (docs `develop/Tests/mocking.mdx`) |

Decision: use mocked IPC Playwright tests for UI logic (fast, every pull request), and a small WebdriverIO smoke suite (start, open settings, add a custom folder, scan a fixture tree, quit) on a nightly schedule for each OS. An embedded WebDriver server inside the release binary is a security surface: compile it only into a `e2e` Cargo feature, never into releases.

### 5.4 Performance regression tests

1. Rust: a `cargo bench` or a custom binary that scans a generated tree of 700 mods and 300,000 files (the real workshop folder has 306,394 files, measured in section 6.1) and records wall time and peak RSS in JSON; a CI step compares against a stored baseline with a tolerance of 25 percent, warning first.
2. Frontend: the snapshot and list render tests from the webview note, run with the same budgets.
3. Startup: a script launches the release binary with an environment variable that makes it exit after the first frame event and records the time; per OS baselines.
4. CI runners are noisy: compare ratios against a reference microbenchmark run in the same job, not absolute numbers.

### 5.5 Manual test matrix

| Area | Windows 10 | Windows 11 | macOS 13 | macOS latest | Ubuntu LTS | Fedora | Arch | Steam Deck |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Install and first launch | x | x | x | x | x (deb, AppImage) | x (rpm) | x (AUR, AppImage) | x (Flatpak or AppImage) |
| Auto detect game and workshop | x | x | x | x | x | x | x | x |
| External drive custom folder | x | x | x (TCC prompt) | x | x | x | x | x (SD card) |
| Scan 700 mods, memory and time | x | | | x | x | | | x |
| Update from previous release | x | | | x | x (AppImage) | | | x |
| Running game warning | x | | x | | x | | | x |
| Uninstall leaves user data | x | | x | | x | | | x |

## 6. Operating system behaviours that affect logic

### 6.1 Measured on this machine (workshop content, 691 mod folders found at depth 2)

| Fact | Result | Command basis |
| --- | --- | --- |
| Folder named `About` | 691 of 691 are exactly `About` (no case variants) | find with `-iname about` at depth 2 |
| File name inside `About` | 686 `About.xml`, 5 `about.xml` | find at depth 3 |
| `About` folders inside version subfolders | 6 | find at depth 3 |
| Files in the workshop content tree | 306,394 | find with `-printf` |
| Longest absolute path | 233 characters (under the Steam path `/home/<user>/.steam/steam/steamapps/workshop/content/294100/`, 60 characters of prefix); none over 240 | awk over file list |

How the game itself behaves (decompiled): the loader resolves `About.xml` case insensitively but uses the exact folder name `About` for the folder, and the preview and icon paths use `About`, `Preview.png` and `ModIcon.png` as literal strings (decompiled:Verse/ModMetaData.cs, `ResolveCaseInsensitiveFilePath` use; constants `AboutFolderName`, `PreviewImagePath`, `ModIconImagePath`). Consequences: on Linux RimStudio must resolve `About.xml` case insensitively (5 of 691 real mods need it) but should not invent leniency for the folder name, because the game would not find `about/` either; flag it as a mod error ("won't load on Linux and macOS case sensitive volumes") on all OS.

### 6.2 Catalogue

| Topic | Behaviour | Handling in RimStudio |
| --- | --- | --- |
| Case sensitivity | Linux ext4 and btrfs: sensitive; Windows NTFS and macOS APFS: insensitive by default (APFS can be case sensitive); exFAT and FAT: insensitive | Probe the volume with a temp file in a writable folder; compare paths with case folding only on insensitive volumes; resolve `About.xml` through a directory listing and a case insensitive match |
| Windows path length | MAX_PATH is 260 characters; the 32,767 character limit needs the `\\?\` prefix or Windows 10 1607 plus the `LongPathsEnabled` registry value plus a `longPathAware` manifest element; relative paths stay limited (Microsoft Learn, accessed 2026-10-04); directory creation needs the path under MAX_PATH minus 12 | Add the `longPathAware` manifest entry to the Windows build (Tauri allows a custom manifest, mechanism unverified) and use extended prefix paths internally; warn when a path passes 240 characters |
| Windows reserved names | `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, trailing dots and spaces (Microsoft naming conventions, unverified in this session) | Validate names when creating mod folders, projects and exports; the item designer must not use them for file names |
| Symlinks and junctions | Windows symlinks need either Developer Mode or elevation; junctions (directories) do not (unverified); macOS and Linux symlinks are free | Avoid symlink dependence on Windows; if a link farm is offered, use junctions on Windows, with a copy fallback; never follow links outside configured roots while scanning; track device and inode to stop loops |
| File locking | Windows can refuse deletes of files in use; the game does not hold mod files open after loading (unverified); `ModsConfig.xml` is rewritten by the game when mods change in game or at exit | Check for a running game before writing `ModsConfig.xml`; write atomically (temp file in the same folder, then rename); keep a one-step backup |
| Running game detection | `sysinfo` 0.39.6 lists processes by name; names differ per OS (`RimWorldWin64.exe`, `RimWorldLinux`, `RimWorld` inside `RimWorldMac.app`; exact names unverified, derive them from the install: the executable beside the data folder named `<exe>_Data`) | Match on the executable path under the detected install folder, not only the name; refresh only the process list (not all system data) at action time and every few seconds while a write dialog is open; in a Flatpak sandbox treat "cannot see" as unknown |
| Antivirus on mass scans | Real time scanners inspect every file open; 300,000 file scans can be slowed and unsigned binaries are scanned harder (unverified) | Read only what is needed (About.xml, files of interest), batch the work, never open every Defs file at start; provide a documented exclusion tip; measure scans with Windows Defender on (manual test) |
| Removable and network drives | Mount points change; stalled mounts block `stat` forever | Already designed in the detection note section 8.5: offline state, `volumeHint`, deadline thread for existence checks; on Linux `/run/media/<user>/<label>` (the owner's `Projects` folder lives on a btrfs volume there, mount type verified by `stat -f`) |
| macOS TCC | Documents, Desktop, Downloads, removable and network volumes ask for consent per app (Apple Support); Full Disk Access is separate | On a permission error show which folder and how to grant; do not retry in a loop; the first scan of an external drive may block on the prompt: show a "waiting for permission" state |
| macOS Unicode | HFS+ stored decomposed names; APFS preserves the bytes as given but compares in a normalization insensitive way (unverified) | Normalize to NFC only for comparison keys and display, never when writing paths back; compare by `(device, inode)` where needed |
| inotify limits | On this machine `max_user_watches` is 524288 and `max_user_instances` 1024 (read from `/proc/sys/fs/inotify`); defaults on older systems are lower (unverified) | Watch only the root directories and rely on rescans on focus plus directory mtimes; never one watch per mod subfolder (a 691 mod tree has far more directories than the default budget on some systems) |
| Timestamp granularity | NTFS stores 100 ns UTC; FAT stores write time with a 2 second resolution in local time, create time 10 ms, access time one day; FAT times shift by an hour across daylight saving changes in some APIs (Microsoft Learn "File Times", accessed 2026-10-04). ext4 and btrfs keep nanoseconds; exFAT 10 ms (unverified) | The cache key is `(path, size, mtime in whole seconds rounded down to even seconds, file id)`, compared for equality, never "newer than"; on FAT-family volumes also store a content hash for small files such as About.xml and treat an mtime difference of exactly 3600 seconds as "same after daylight saving" |
| Rust path handling | Paths may not be valid UTF-8 on Linux | Use `OsString` or `camino` with lossy display; reject non UTF-8 paths at the IPC edge with a clear error (unverified policy) |
| Unity data folder differences | Linux data folder `RimWorldLinux_Data`, macOS app bundle `Contents`, Windows `RimWorldWin64_Data`; the `savedatafolder` command line argument overrides the user data folder (decompiled:Verse/GenFilePaths.cs) | Detection keeps the argument in mind when the user launches the game through RimStudio, and passes it only if the user configured a custom save folder |

## 7. Steam Deck and Linux gaming

1. Screen: 7 inch touch screen, 1280 x 800, 60 Hz, LCD (Valve tech specs page, accessed 2026-10-04; the OLED model has the same resolution to my knowledge, unverified). The default RimStudio layout must work at 1280 x 800 with a 100 percent scale, which means a collapsible side panel and a mod list that stays usable at 1280 wide.
2. Touch targets: aim for at least 40 CSS pixels for primary controls in the "compact touch" density, and respect the WCAG 2.2 minimum of 24 pixels everywhere (WCAG number from memory, unverified). The webview note already requires keyboard commands for every drag operation, which also serves a controller mapped as keyboard.
3. Desktop mode vs game mode: desktop mode runs a KDE Plasma session where Tauri works like on any Linux desktop; in game mode, a non-Steam shortcut runs the app inside gamescope, which has no window decorations and may break custom titlebars and dialogs (unverified). Provide a `--fullscreen` flag and keep native dialogs usable; do not rely on a window manager.
4. Install: the SteamOS root is read-only and not suitable for deb or rpm; Flatpak through Discover or an AppImage in the home folder are the two routes (SteamOS details unverified here). The updater works for the AppImage.
5. Steam on the Deck is the native Linux client; libraries may be on the SD card at `/run/media/mmcblk0p1` (path unverified), which the Flatpak needs `--filesystem=/run/media` for. SD card filesystems are often ext4 or exFAT: the timestamp rules in section 6 apply.
6. RimWorld has a native Linux build; Proton is only used if forced (the detection note records an empty `compatdata/294100` folder on this machine, so folder existence is not a Proton signal). If Proton is active the user config folder moves into the prefix (detection note section 3.8).
7. WebKitGTK graphics issues (blank window under some Wayland and NVIDIA combinations) are handled by the safe graphics flag and crash marker of the webview note section 4.3; the Deck's AMD GPU is not the usual trigger (unverified).

## 8. Published numbers: size, start, memory

Third-party numbers are weak: each is a single run or a vendor blog. The only primary figures are from the Tauri docs and the project's own earlier lab.

| Metric | Number | Source and quality |
| --- | --- | --- |
| deb package size range | 2 to 6 MB | Tauri docs `distribute/appimage.mdx` (a range, not a measurement) |
| AppImage size | 70+ MB | same page |
| Windows WebView2 offline installer added | about 127 MB; fixed runtime about 180 MB; bootstrapper about 1.8 MB | Tauri docs `windows-installer.mdx` |
| Hello world Tauri bundle | 3.2 MB | rustify.rs and similar blogs found by search, 2026-10-04; method not stated, treat as indicative |
| Scaffolded demo app bundle | 8.6 MB | gethopp.app blog, single run on a MacBook Pro, the author disclaims rigor |
| Idle memory | 42 MB (blog claim); about 172 MB with six windows open (gethopp, N=1) | secondary |
| Cold start | 380 ms (blog claim); gethopp says negligible difference from Electron | secondary and conflicting |
| Project lab, Linux, WebKitGTK 2.52.6 | Rust side ready after 0.35 s; webview DOMContentLoaded 1.94 s and first animation frame 2.45 s after process start; 347 MB proportional set size for the webview after a heavy benchmark | `docs/research/webview-and-ipc-performance.md` section 4.3 and 5 (own lab, cold, release build) |

Interpretation: the webview process creation, not Rust, dominates cold start (about 1.6 s of the 2 s on the research machine), and WebView2 and WKWebView numbers on other OS are not measured. Windows and macOS numbers must be collected on real machines in the first release cycle (open question). The startup techniques follow from the gap:

1. Create the main window hidden (`visible: false` in the window config) and show it on the first frame event from the frontend, so the user never sees a white window. Keep the background colour of the webview and window set to the theme colour (token) to avoid a flash.
2. Paint the shell (chrome, skeleton rows) with no data dependency; budget 1.5 s on the research machine class (webview note).
3. Persist a snapshot of the last mod list as JSON in the cache folder, load it in Rust before the window shows, send it as the first snapshot, then run the scan and send deltas (design from webview note section 2.4). The snapshot is keyed by the cache key of section 6.2.
4. Defer all scanning, dataset refreshes and update checks until after first paint; run them on a low priority worker pool with a cap on threads so that the UI stays responsive on a Steam Deck.
5. A native splash is possible but adds a second window; the hidden window plus skeleton is simpler. Revisit only if measured start exceeds budget on Windows.
6. Keep initial JavaScript under 300 KB gzip with lazy chunks for heavy tools (webview note budget 8), and avoid isolation pattern overhead until measured.
7. Release profile: `lto = "thin"` or `"fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` where compatible (standard Cargo options; the size gain is unmeasured here), and Tauri's `opt-level` default left alone until a measurement says otherwise.

## 9. Decision table, CI matrix, risk register, release checklist

### 9.1 Decision table

| Target | Format | Recommended | Reason |
| --- | --- | --- | --- |
| Windows 10 and 11 x64 | NSIS setup exe, `currentUser`, `embedBootstrapper` | yes, primary | No UAC prompt, updater friendly, small |
| Windows | Portable zip with marker file | yes, secondary | External drive users; updater disabled in this mode |
| Windows | MSI | no (later on request) | Windows-only build, per-machine, no updater benefit |
| Windows ARM64 | NSIS | later | Needs extra tools and a runner; demand unknown |
| macOS arm64 | dmg plus `.app.tar.gz` for updater, signed and notarized | yes, primary | Most Macs; notarization removes Gatekeeper blocks |
| macOS x64 | dmg | yes | Older Macs; cross compile on the arm runner |
| macOS | Universal single file | optional | Simpler for users, bigger download |
| Linux x64 | AppImage built on Ubuntu 22.04 | yes, primary | One file, updater works, runs on the Steam Deck |
| Linux x64 | deb and rpm | yes, secondary | Native packages for system WebKitGTK; no self update |
| Linux | Flatpak | second wave | Sandbox needs filesystem grants and has process visibility limits |
| Linux | AUR `-bin` | after first stable | Arch is the owner's platform and a large modder segment |
| Linux arm64 | AppImage and deb | later | Public repository arm runner is free |
| Update channel | GitHub Releases `latest.json`, minisign style key | yes | Free hosting, tauri-action generates it |
| Updates in Flatpak, deb, rpm, AUR | package manager | yes | Hide the in-app installer there |

### 9.2 CI matrix summary

See section 4.3 for the table; the gate job runs on `ubuntu-24.04`, builds run on `windows-latest`, `macos-latest` (two targets) and `ubuntu-22.04` (plus `ubuntu-22.04-arm`), and the end to end smoke suite runs nightly on all three OS.

### 9.3 Risk register

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | Loss of the updater private key | low | severe (no more updates for installed apps) | offline backup, secret manager, documented rotation bridge release |
| 2 | SmartScreen warning drives users away | high at start | medium | sign from the first stable, keep one identity, document the warning, offer the portable zip and a checksum |
| 3 | macOS notarization or App Management blocks | medium | high on macOS | never write into the game bundle by default; clear permission messages; clean account test |
| 4 | System WebKitGTK too old or buggy on Linux | medium | medium | feature probes, safe graphics flag, document the support floor |
| 5 | AppImage glibc break | medium | medium | build on Ubuntu 22.04, test on an older distribution in CI (container leg) |
| 6 | Flatpak sandbox hides Steam folders or the game process | high if shipped | medium | explicit grants, detection fallbacks, treat unknown process state as unknown |
| 7 | Mass scan slowed by antivirus on Windows | medium | medium | read minimal files, batch, cache keyed on stat, exclusion tip |
| 8 | Stat cache wrong on FAT family drives | medium | medium | even second rounding, hash for small files, equality compare |
| 9 | Removable drive path changes or goes offline | high | low to medium | `volumeHint`, offline state, never delete active mods silently (the game itself drops uninstalled mods from the active list, per the project context) |
| 10 | Unsigned end to end driver shipped in releases | low | high | compile e2e server only with a feature, test the release binary has none |
| 11 | Licence contamination through dependencies or fixtures | low | high | cargo-deny licence gate, fixture builder instead of copies |
| 12 | Version drift between Cargo.toml, package.json, tauri.conf.json | medium | medium | xtask check in CI |
| 13 | GitHub runner changes (Intel macOS retirement) | medium | low | cross compile on arm, pin runner labels where it matters |
| 14 | Windows long paths in deep mod trees | low (max 233 here) | medium | long path manifest, extended prefix, warn at 240 |

### 9.4 Day-one release checklist

1. Version agrees in `Cargo.toml`, `package.json` and `tauri.conf.json`; tag created.
2. Gate job green: format, clippy, tests, `cargo deny`, `cargo audit`, lint, vitest, Playwright mocked suite.
3. All matrix legs built; artifacts renamed to the `rimstudio-<version>-<os>-<arch>` scheme; `SHA256SUMS` attached; provenance attestations created.
4. Windows installer signed and timestamped; verified with `signtool verify` on a clean VM.
5. macOS app signed, notarized and stapled; opened on a clean account with Gatekeeper on and the external drive prompt exercised.
6. AppImage tested on the oldest supported distribution and on the Steam Deck desktop mode; deb and rpm installed on Ubuntu LTS and Fedora.
7. `latest.json` generated, signatures valid, updater tested from the previous release on each OS (and on AppImage from a read-only location to see the fallback message).
8. Updater public key in the config equals the one stored in the secret backup; the backup is verified.
9. Smoke suite passed on the three OS; first launch shows the shell before data; cold start numbers recorded per OS.
10. Detection tested on the manual matrix: native Steam, Flatpak Steam, a second library, a custom folder on an external drive, Windows long path, macOS permission denial.
11. Release notes list the support floor (Windows 10, macOS 13.3, WebKitGTK version), known SmartScreen behaviour and the portable mode.
12. Draft release reviewed, then published; rollback plan (previous installers kept, revert release procedure) written down.

## Implications for RimStudio

1. Target Windows 10 and 11, macOS 13.3 or newer and Linux with a WebKitGTK 2.50 or newer; set `bundle.macOS.minimumSystemVersion` explicitly to 13.3 because the Tauri default is 10.13; a runtime check shows an "outdated webview" screen on failure.
2. Ship NSIS (`currentUser`, `embedBootstrapper`), a portable zip, a signed and notarized macOS dmg per architecture, and a Linux AppImage built on `ubuntu-22.04` plus deb and rpm; Flatpak and AUR come after the first stable release.
3. Use GitHub Releases as the update channel with `tauri-action` generating `latest.json`; stable and beta channels use different manifest URLs chosen at runtime; no delta updates are assumed.
4. Detect the install source (AppImage, Flatpak, system package, portable) at start and hide the in-app installer where a package manager or a read-only location owns updates; test this with a unit test on the detection function and a manual test per format.
5. Never write mod content into the game folder by default; manage mods through `ModsConfig.xml` and custom folders. On macOS the Mods folder is inside `RimWorldMac.app` (decompiled rule plus Unity `dataPath` documentation), so any write there is opt-in and must handle the App Management refusal.
6. Implement the portable marker file `rimstudio.portable` in the core crate with one data-root resolver, and make the updater inactive in portable mode on Windows.
7. Resolve `About.xml` case insensitively (5 of 691 real workshop mods use lowercase), require the exact `About` folder name, and flag deviations as errors that apply to case sensitive systems.
8. The stat-based cache key is equality on `(path, size, mtime rounded to even seconds, file id)` with a content hash fallback on FAT-family volumes, and it must be covered by a test that simulates 2 second and 1 hour shifts.
9. Check for a running RimWorld (path based match through `sysinfo`) before writing `ModsConfig.xml` or any mod folder; write atomically; in a sandbox report "unknown".
10. The Windows build declares long path awareness and uses extended prefix paths internally; paths over 240 characters produce a warning.
11. Startup order: hidden window, painted shell, cached last-list snapshot, deferred scan; a CI check records time to first frame per OS and warns when it exceeds the budget.
12. CI: matrix of section 4.3, `cargo deny` with licence rules that serve R11, `cargo audit`, an xtask version check, action versions pinned by hash and a release job that writes `SHA256SUMS` and attestations.
13. The fixture builder generates per-OS Steam layouts (native, Flatpak, Proton, Windows, macOS, edge cases) at test time and runs on all three OS legs; no real Steam or game data is checked in.
14. End to end tests: Playwright with `mockIPC` on every pull request; a WebdriverIO smoke suite nightly on all OS; any embedded WebDriver server is behind an `e2e` Cargo feature that the release build verifiably lacks.
15. Compact, touch friendly density and a 1280 x 800 layout are acceptance criteria for the Steam Deck.

## Open questions

1. Exact start time, idle memory and installed size of the real RimStudio shell on WebView2 and WKWebView: no first-party published numbers exist and the project's lab ran only on Linux; collect them in the first release cycle.
2. Does the Flatpak sandbox allow `--filesystem=~/.var/app/com.valvesoftware.Steam`, and can `sysinfo` see host processes there? Needs a real Flatpak test.
3. Is `--target universal-apple-darwin` still the right flag in `tauri-cli` 2.12, and does `tauri-action` v1 handle it in the manifest? Verify against the `tauri-action` README.
4. Which signing path will the owner use on Windows: Azure Artifact Signing (country and entity eligibility must be checked), an OV certificate with cloud signing, or none for the first pre-release?
5. Does Steam rewrite or flag files that are added to `RimWorldMac.app/Mods`, and does macOS App Management block a user-level app from creating folders there? Needs a Mac.
6. Does `tauri.conf.json` accept a `package.json` path for `version`, and can the Windows manifest get `longPathAware` through the Tauri build configuration? Verify in the config schema.
7. Which environment variables do AppImage and Flatpak set for source detection (`APPIMAGE`, `FLATPAK_ID`)? Confirm in their documentation.
8. What are the exact process names of the RimWorld executable on Windows and macOS builds of version 1.6? Confirm on real installs.
9. Can the updater plugin accept channel-specific endpoints at runtime and a second public key for rotation? Check the plugin API documentation on docs.rs.
10. How much do antivirus products slow a 300,000 file scan on Windows? Measure with Defender enabled on a Windows machine.
