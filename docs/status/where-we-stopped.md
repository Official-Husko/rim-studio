# Where we stopped

Status: handoff note | Last updated: 2026-10-10 (the build work was done on 2026-10-05)

This page is for the next working session. It says what exists, what was verified and how, exactly where the work stopped, and what is left, in the order in which it is worth doing. It does not repeat the specification: the requirement groups, measured numbers and gate results live on the [0.1.0 backend status](0.1.0-backend.md) page, the architecture in [the overview](../architecture/overview.md), the file layout of a mod in [mod layout](../features/mod-layout.md).

## 1. In one page

1. **What exists.** The weapons editor of release 0.1.0 is built end to end: ranged and melee weapons, the math, the quiz and the fit meter, vanilla output by default, the optional Combat Extended (CE) patch generator (guns, bows, weapon platforms, under barrel units, custom ammunition), texture and sound import, the weapon wizard that proposes every number from a category, the Patches page (scan, questions, plan, apply, lint), layout fixes with undo, the Mod page (create a mod with the recommended tree, edit About.xml and LoadFolders.xml), a link of a mod into the game's `Mods` folder, a temporary dark UI over all of it, the Tauri desktop shell with live reload and the headless CLI.
2. **State of the repository.** Branch `main`, 20 feature commits on top of `6e7a3fa` (section 3) plus the follow up commits of the final pass. Nothing is pushed and nothing is tagged. The folder is still named as it was (D-099); the GitHub repository is `rim-studio`.
3. **Verified.** Every gate is green on the final tree (section 4): 4,047 Rust tests, 1,245 frontend unit tests, 52 Playwright tests (three full runs in a row, no failure), the Tauri shell check and its headless smoke test, the Windows compile check, the docs, layer and source checks.
4. **Not verified.** A weapon made with the tool has never been loaded in RimWorld itself. Windows and macOS were never run (compile checks only). The desktop shell was driven under WebKitGTK for Setup, the Mod page tabs and the New mod window, not for the weapon wizard, the ammunition windows or the Patches page.
5. **Next step.** The owner makes a test mod end to end (section 2) and starts the game with it. Everything after that is the backlog in section 6, ordered by how much it hurts.

## 2. Try it: from nothing to a mod in the game

The browser build and the desktop shell show the same pages. Start one of them:

| Way | Command | Notes |
|---|---|---|
| Browser | `pnpm install` once, then `pnpm dev` in the repository | Starts the development bridge and Vite; open `http://localhost:5173`. Needs a Rust toolchain; set `CARGO_TARGET_DIR` outside the repository. |
| Desktop window | `pnpm tauri:dev` | Live reload for the UI and for Rust. Needs the Tauri system libraries (WebKitGTK on Linux). Its own target folder, `~/.cache/rimstudio-target-tauri`. |
| VS Code debugger | F5 in the opened repository | The first configuration of `.vscode/launch.json`: Vite in the background, a debug build of the shell, the app under gdb (C/C++ extension). Stop other dev servers on port 5173 first. |
| Owner test copy | `~/rimstudio-test/run.sh` and `~/rimstudio-test/stop.sh` | A copy of the tree with its own target folder; see section 7. |

Then, in the app:

1. **Setup**: Detect, add the folder that holds your own mods (Add folder), Scan. The scan card shows loadable and custom only counts, duplicate package ids and whether Combat Extended is in the library.
2. **Mod**: Create a new mod (name, author, package id, game versions, the optional folders; three steps, the last one lists every file), or open an existing mod folder. Basics edits About.xml, Versions and folders edits LoadFolders.xml, Layout checks and fixes the tree, Test in game links the mod into the game's `Mods` folder.
3. **Weapons**: New weapon wizard (pick a type such as bolt action rifle, SMG or sword, describe rate of fire and calibre, read the numbers and their reasons, name it), or clone a reference weapon. Edit in the editor, read the plan, Apply to the project. Switch Combat Extended on only if you want the patch in its own gated folder.
4. **Patches**: choose a mod, answer the questions of each weapon, read the plan, convert; Lint checks patch files.
5. **Start RimWorld** with the mod enabled and look at the weapon in the game. This is the step nobody has done yet. Report what the game logs say; the checks in this repository cannot know.

## 3. The commits

All on `main`, oldest first, on top of `6e7a3fa chore(desktop): remove the unused weapons placeholder page`. Each Rust commit was built and checked on its own (`cargo check --workspace --all-targets --locked`, the xtask checks, the tests of the crates it touches, regenerated bindings); each frontend commit passed typecheck, the touched tests and lint.

| Commit | Header |
|---|---|
| `e20515f` | feat(shell): add the Tauri desktop shell |
| `96cf29d` | feat(assets): import textures and shot sounds into a weapon design |
| `5a71a5a` | feat(design): convert bows, weapon platforms and more to Combat Extended |
| `5a291e9` | feat(design): add the Combat Extended ammo catalogue and custom ammo |
| `93ffb2f` | feat(toolkit): fix layout problems of a mod with an undo journal |
| `71287b6` | feat(manager): add library facts, weapon facts and a patch file lint |
| `4207f30` | feat(library): link a project into the game's Mods folder |
| `740364d` | feat(design): propose every number of a weapon from an archetype |
| `309f3c4` | feat(toolkit): edit the basics, load folders and versions of a mod |
| `a8a3a33` | feat(designer): import textures and shot sounds in the Weapons editor |
| `1737d4f` | feat(designer): edit Combat Extended bows, platforms and ammunition |
| `9f973a9` | feat(patches): show weapon facts, lint by file and library facts |
| `f06c677` | feat(designer): add the New weapon wizard |
| `c24713a` | feat(project): add the Mod page with basics, folders and versions |
| `3f476ba` | feat(project): fix layout problems from the Layout tab with an undo |
| `0992da2` | feat(project): add the Test in game tab that links a mod into the game |
| `a145a12` | test(e2e): cover the new pages and flows with Playwright specs |
| `5f54cf1` | docs(adr): record the decisions and the evidence behind the new features |
| `0dea565` | docs: specify the new features and the decisions behind them |
| `e73bb80` | docs(status): record the shell, the new features and the results |

The follow up commits of the final pass come after `e73bb80`:

| Commit | Header |
|---|---|
| `0f501c0` | test(design): bound the strength error by the whole number damage step |
| `72aba82` | chore: ignore the build output of the research schema probe |
| `309f6bd` | docs(status): add the where we stopped note and refresh stale state |
| `0ec687b` | feat(ui): add autofocus to TextField and a spoken label to tab badges |
| `4c5f372` | fix(project): keep the Mod page edits safe and the dialogs focused |
| `30a21fa` | docs(status): record the final pass in the handoff note and the gates |
| `9db909c` | docs(status): record the gate run of the final commit and the test copy |
| `1509d66` | docs(status): list the last commits and the stopped watchdog in the note |
| `4490b57` | build(vscode): launch the desktop app under a debugger with F5 |
| `e838a86` | docs(roadmap): draw a roadmap picture from a status file |

`git log --oneline 6e7a3fa..HEAD` is the authoritative list; a commit that only touches this note may come after these.

## 4. What was verified, and how

| Check | Result on the final tree |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 4,047 passed, 0 failed, 64 ignored (real install tests and benchmarks) |
| `cargo run -p xtask -- check-all` | layers, source rules and docs: 0 findings |
| `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` | clean (compile only) |
| `pnpm typecheck`, `pnpm lint`, `pnpm build`, `pnpm --filter rimstudio-ipc-types bindings:check`, the bindings drift test | clean, no lint warnings, 48 golden type files |
| `pnpm test` | testkit 8, ui 133, desktop 1,104 |
| `pnpm e2e` (Playwright, Firefox, the real bridge and the owner's install, temporary data) | 52 tests; three full runs in a row all green (2.9 to 3.2 minutes each) |
| `pnpm tauri:check`, `pnpm tauri:smoke` | clean; the smoke test boots the app and lists 67 commands |
| Desktop shell under WebKitGTK (Xvfb, production build, real install) | Setup (detect, native folder dialog, add folder, scan), Mod hub, New mod window, Basics edit and save, Versions and folders, Files and Layout tabs render and work like the Firefox pages; not covered: Test in game, the wizard, ammunition windows, Patches |

Flaky tests found and fixed in the final pass, all of them test or picker defects and none a product defect:

1. The browser folder picker kept "Choose this folder" enabled while a typed path was still being listed, so a quick click returned the previous folder. The button waits for the listing now (unit test added).
2. `16-viewports-round2.spec.ts` reloaded the page right after a hash navigation, which aborted a lazy import and failed the test on the page error that follows. It loads a fresh page now.
3. Two specs held stale assumptions: the Lint tab lists patch files, and the owner's folder may already be a source in a shared test stack.
4. A randomized test of the archetype solver (`every_ranged_proposal_is_finite_positive_and_lands_near_its_target`) failed once in about thirty runs. Cause: damage is a whole number, so for a damage of 3 or 4 one point is a step of 15 to 17 percent of the strength index and the proposal can miss a reachable target by that much (a sweep of 13,104 proposals found 75 such cases, all at a damage of 3 or 4). The test tolerance now follows the whole number step and a deterministic regression test names the two corners. The solver itself was not changed; see item 6.4.2 for the possible improvement.

## 5. Where the work stopped

The session ended while three pieces of work were in flight. All three are finished or recorded:

1. **The Mod page task** (create a mod with the recommended tree, edit its basics) was stopped before its own report. It was completed by hand and then audited against its original task card: every deliverable exists, is wired into the page, is translated and has unit and end to end tests. The audit found 13 small defects and gaps, listed in section 6.1; four of them (MP-2, MP-3, MP-8, MP-9) were fixed in the final pass.
2. **The final QA task** was also stopped before its report. Its Playwright additions exist (specs 12 to 16, ammo, wizard, mod-basics), the gates were run by hand, the status page was updated. Not repeated: the live reload proof of the shell (a frontend edit and a Rust edit, each reverted); it was proven once before and the mechanism has not changed.
3. **A read only audit run** (Mod page audit, QA audit with three repeated full e2e runs, a desktop shell run, four reviews of 99 screenshots) was cut off by the end of the session. Its notes survived and are summarized in section 6; the raw notes are in the session notes folder (section 7). Two of its planned outputs were never written: a consolidated open items list (this page replaces it) and an operating guide (section 2 and 7 replace it).

Nothing else was running. There is no uncommitted work from the build rounds.

## 6. Open items, in the order worth doing

Severity words: major means wrong or misleading behaviour, minor means visible polish or a small functional gap, nit means tiny. Findings from the screenshot reviews were made from still images; check each in the running app before fixing, because a few may be capture timing.

### 6.1 Mod page defects found by the final audit

| Id | Severity | Finding | Suggested fix |
|---|---|---|---|
| MP-1 | minor | Unsaved edits are lost without a warning when the person closes the mod, opens another or starts a new one (`ProjectPage.tsx`, `ModView.tsx`, `aboutStore.loadAbout`). Reopening the same mod silently restores the old draft, which is inconsistent. | A shared `confirmLeave()` before close, new, open and project switch while `pendingChanges` or `foldersPending` is above zero. |
| MP-2 | fixed | Adding a version folder drops unsaved LoadFolders edits silently (`VersionAddDialog.tsx` calls `loadFolders` without keeping the edits, and the add rewrites the file). | Fixed in `4c5f372`. |
| MP-3 | fixed | A dialog opens with focus on the header Close button, not on the first field (`packages/ui` Dialog picks the first focusable). | Fixed in `4c5f372`. |
| MP-4 | minor | The real bridge e2e does not cover the Preview.png import and removal, ByVersion blocks, a stale refusal on LoadFolders, or Discard on the Versions tab. | Extend `mod-basics.spec.ts`. |
| MP-5 | nit | The test "a bad package id and a collision ... saving is still allowed" never asserts that Save is enabled. | Add the assertion or rename it. |
| MP-6 | nit | No test of keyboard use of the new views (Escape closes the dialogs, focus returns to the opener, Tab order). | Add component tests. |
| MP-7 | nit | The review step of the creation window lists paths only, not the text of the About.xml that will be written. | Return the text from `project_scaffold_preview` and show it in a code view. |
| MP-8 | fixed | The unsaved count on a tab is a bare number ("Basics 1") for a screen reader. | Fixed in `4c5f372`. |
| MP-9 | fixed | `createStore.scheduleCheck` does not invalidate the token of an in-flight check, so Next can be enabled on a stale preview for up to 300 ms. | Fixed in `4c5f372`. |
| MP-10 | nit | Dependency findings are keyed by form row index but blank rows are dropped from the preview, so pointers shift by one after a blank row; Move up and down remounts the row and loses focus. | Map visible index to kept index; key rows by a stable id. |
| MP-11 | nit | A folder edit made while another is in flight is dropped (`applyFoldersOp` returns false when busy) and the add field is cleared before the result. | Queue the operations; clear the field only on success. |
| MP-12 | nit | Picking the same dependency twice by typing the id is ignored without feedback; the new mod window needs scrolling inside the dialog at 1024 by 700. | Show a short note; consider a two column form. |
| MP-13 | nit | The original task card names `project_version_folder_add`; the registry command is `project_version_add`. | None (card only). |

### 6.2 Findings of the screenshot reviews

Reported as major (verify first):

| Id | Where | Finding |
|---|---|---|
| SC-1 | Layout tab, History | After an undo the history still lists the undone fix as "Can be undone" with an active Undo button while the page behind says the layout is in order (`layout-fix-undone`). May be a stale refresh; check that the history reloads after an undo. |
| SC-2 | Setup | Detection warnings are titled with raw codes (`steam.manifest-leftover`, `steam.userdir-version-mismatch`) and a harmless note uses the warning colour. |
| SC-3 | Setup, duplicates | Every copy of a duplicated package id is flagged "The game does not see it" in amber, with no marker for the kept copy and no column headings. |
| SC-4 | Wizard | "Typicality 84 out of 100" is labelled unusual (orange) while the legend says a low score means unusual; the counts of typical, plausible and unusual stats differ between the wizard and the editor. |
| SC-5 | Wizard, editor | Three different timing numbers for one weapon (a shot every 3 s, cooldown 1.1, cycle 5.1 s) with no explanation. |
| SC-6 | Ammunition | After choosing a caliber the "default projectile of the ammo set" select is empty with a hint that contradicts the state. |
| SC-7 | Custom ammunition | The Crafting tab says "Fix 3 errors" while the Review tab says "No problems found" and carries a badge 3. |
| SC-8 | Patches table | The weapon table is wider than its pane at 1440 and at 1024; the Mass column is cut with no visible scroll cue (known, section 9.8 of the status page). Ammo quick pick buttons overflow the right panel and lose the damage figure. |
| SC-9 | Designer output | "Fix the 5 errors in the problems list before writing" while Diagnostics says "No errors": the count is the unanswered Combat Extended questions, and the wording misleads. |
| SC-10 | Platform apply report | After "Wrote 2 files" the dialog reports patch engine errors about another weapon that shares the file, with no hint which weapon or what to do. |
| SC-11 | Weapons at 1024 by 700 | The Output column with the Apply button is not visible without scrolling; check that it is reachable and obvious. |

Minor findings, by theme (the image names are in the session notes):

1. **Raw ids and codes in the UI**: diagnostic ids (`design.duplicate-capacity`, `design.pool-thin`, `ce.cep0nn`, `ce.about-suggestion`) in apply reports and lint; messages that start in lower case and use internal wording; a brand new wizard weapon opens with a warning the modder did not cause (duplicate capacity of the tool barrel).
2. **Tables and trees without headers**: file tree counts such as "19 / 1.7 MiB", Setup duplicate table, Lint repeated explanations per weapon with no grouped count; "1 files" grammar.
3. **Units missing**: damage, range, DPS, cooldown, mass in the wizard side summary; rate of fire labels; work to make (ticks); a damage unit "hp" in the custom ammunition window.
4. **Clipping and overflow**: the XML preview cut at the right edge with no scroll cue; file names broken mid word in output cards and paths; "Tool list of converted weapons" header clipped; the Plan tab file card collapsing to "C...".
5. **State cues**: Next or Save dimmed with no reason (validation pending); a Save button that stays active after a stale refusal; "Updating the plan" shown beside a saved draft; a spinner left in an apparently loaded page; the Layout chip staying amber when only notes remain.
6. **Consistency**: options as checkboxes in one apply dialog and switches in the other; Close versus Done on the final button; content column width differs between pages (Mod stops at about 1,310 px and leaves a dead strip on the right at 1440); the dev-only Gallery entry in the rail of development builds.
7. **Wording**: "Scan depth 1" unexplained; "update mode applies"; "Likely 1.488 to 2.106, wide 1.177 to 2.662"; "D Derived" badge reads as a stray letter; "None / None" for Folder and LoadFolders.xml in the version table.

### 6.3 Test coverage gaps (from the QA audit)

1. `ammo.spec.ts` "fit 1024 by 700" takes screenshots but asserts nothing, although its title and the status page say it checks the fit.
2. `14-bows.spec.ts` scan test does not assert that the platform rifle row is listed as convertible, which the status page states.
3. `13-layout-fix.spec.ts` Lone Wolf test has no axe run and no check that the layout check improves; axe at 1024 runs only on one step; `16-viewports-round2.spec.ts` has no axe.
4. Screenshots are not assertions: visual review happens by hand. There is no screenshot comparison.
5. Reason regexes in the Setup duplicate test use alternations that are permissive.

### 6.4 Product limits and technical debt

1. **Never loaded in the game.** Patch operations rely on XPath predicates verified by the in-engine dry run only. A first real run in RimWorld is the highest value check left.
2. **Whole number damage limits the strength fit** of low damage, fast firing weapons (machine pistols, light SMGs at high rate of fire) to about 17 percent. A second polish step on cooldown and warmup would narrow it; it changes goldens, the wizard fixtures and the docs numbers, so do it deliberately.
3. **Archetype shapes were tuned on 37 vanilla weapons** of one install; work to make is the least predictable number; the fit meter calls outliers unusual, not wrong. An out of sample check on a larger library is open.
4. **Custom ammunition** imports no art or sounds; the rebuild of Combat Extended's own ammunition files leaves 12 missing and 16 different elements, each classified in the task report.
5. **Estimates rest on one install** (17 to 19 converted weapons); thresholds are implementation choices (D-095).
6. **CEP010 and CEP015** are never checked because no caller supplies the Combat Extended type and field tables; the Combat Extended settings of the user's config folder are not read (OD-19).
7. **Not converted**: grenades, launchers, turrets and apparel.
8. **Windows and macOS**: only compile checks; junction creation and `Platform::file_id` are open; the shell's WebView2 behaviour is untested.
9. **Fixtures hold real numbers** from the owner's install (allowed by D-100); the shipped binaries still carry no value table. Re-record the fixtures against the bridge when the DTOs change (`packages/testkit/fixtures`).
10. **Settings card path** on the desktop shell shows `<dataDir>/config/settings.jsonc` while the file is in the config root (`SettingsCard.tsx`, found in the shell run).
11. **The webview remembers the last project** across runs through local storage; a fresh data folder still starts with the old project open and registers it.
12. **Doc drift**: the docs index still says the backend is being implemented and its status line is from 2026-10-04; check it against the status page when the next round starts.

### 6.5 Owner decisions still open

1. The licence text (D-088); nothing depends on it.
2. Review the rating thresholds and plausibility numbers (D-095, the 0.6 to 2.5 window).
3. Whether the temporary UI is kept as the base of the real UI or replaced by the visual design tool's output (`docs/design/design-prompt.md`).
4. Whether the owner's Gewehr 41 and Lone Wolf mod copies in `crates/rimstudio-toolkit/tests/data/layout_fix` (stripped copies: About.xml, patch and definition files, placeholders for textures and sounds) may stay in the repository.

### 6.6 Later milestones (unchanged, see the roadmap)

Apparel editor, the full mod manager (lists, sorting, profiles, validation), community datasets, the Workshop publisher, packaging and the updater, a first signed or unsigned release build (no Windows signing by decision).

## 7. Environment and housekeeping

1. **Build folders.** Always `export CARGO_TARGET_DIR=$HOME/.cache/rimstudio-target` before cargo in the repository (a forgotten export creates `target/` in the repository on the slow drive; delete it). The shell uses `~/.cache/rimstudio-target-tauri`, the owner test copy `~/.cache/rimstudio-target-stable`. The `debug/incremental` folders grow to tens of gigabytes; delete them when space is short. `/tmp` is RAM backed.
2. **Owner test copy.** `~/rimstudio-test` is a git worktree of this repository, detached at `main` as of 2026-10-10, with the untracked `run.sh` and `stop.sh`, its own target folder and its own `node_modules`. A development instance of it was left running on port 5173 (`stop.sh` stops it, `run.sh` starts it). To bring it to a newer commit: stop it, `git -C ~/rimstudio-test checkout -f --detach main`, `pnpm install --frozen-lockfile`, start it. It reads the owner's real RimWorld install and writes only into the development data folder.
3. **Disk watchdog.** A background loop named `disk-watchdog.sh` (in a session folder under `/tmp`) deletes `debug/incremental` and old test executables when `/home` drops under about 17 GB. It was stopped at the end of the session and is not needed while `/home` has room (more than 100 GB were free on 2026-10-10); if you start a long build session on a full disk, write a loop like it again.
4. **Headless desktop runs.** Use your own Xvfb display, `env -u WAYLAND_DISPLAY GDK_BACKEND=x11 DISPLAY=:N`, and a temporary `RIMSTUDIO_DATA_BASE`; without that GTK opens a window on a Wayland desktop and writes into the real data folders.
5. **Session notes.** `~/rimstudio-session-notes/2026-10-05/` (outside the repository, not part of the project) holds the task reports of the build rounds (`reports/`), the audit notes of the final pass (`audit/`, including the full list of screenshot findings), the commit messages and 99 screenshots of the final tree (`shots/final`) and 25 of the desktop shell (`shots/desktop-final`).
6. **Roadmap picture.** `docs/roadmap.svg` shows how far M0 to M7 and the 0.1.0 release have come (about 40 percent of the planned effort on 2026-10-10). It is drawn from `docs/roadmap-status.json` by `pnpm roadmap`; edit the data when a scope item, an exit criterion or a release check changes state, redraw, and commit both with the change (rule 10 of the project instructions). `pnpm roadmap:check` is part of `pnpm check`.
7. **Pending processes.** Nothing from the build rounds runs except the development instance of the test copy. Other browsers and display servers on the machine are not part of this project.

## 8. Read next

1. [0.1.0 backend status](0.1.0-backend.md): requirement groups, measured numbers, gates, sections 8 to 10 for the three UI rounds.
2. [Decision register](../architecture/decision-register.md) and the ADRs 0040 to 0058 for the decisions of the last rounds (shell, layout fixes, imports, ammunition, link, mod basics, archetypes).
3. [Mod layout](../features/mod-layout.md) section 15.9 for the Mod page, [frontend architecture](../architecture/frontend-architecture.md) section 18 for how the UI is built.
4. [Roadmap](../roadmap.md) section 2.1 for the exit criteria of 0.1.0.
