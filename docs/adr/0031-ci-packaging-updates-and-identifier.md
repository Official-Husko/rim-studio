# ADR 0031: CI, packaging, updates and the app identifier

Status: needs-owner | Last updated: 2026-10-04 | Register: D-059, D-049

## Context

The product ships on Windows, macOS and Linux (R1). Notarisation and the updater need owner accounts; the owner declined Windows code signing (D-087). The app identifier fixes data directories and update continuity, and the owner's domain is unknown.

## Decision

CI runs a matrix on Windows, macOS and ubuntu-22.04. Artifacts are an NSIS installer (`currentUser`) plus a portable zip, a notarised dmg per architecture (macOS notarisation stays open), and AppImage plus deb and rpm; releases go through `tauri-action` to GitHub Releases with stable and beta manifests. Install source detection hides the updater where a package manager owns updates. The support floor is macOS 13.3 and WebKitGTK 2.50 or newer; Flatpak and AUR follow the first stable release. Artifacts follow `rimstudio-<version>-<os>-<arch>`. The identifier is the placeholder `app.rimstudio.desktop` until the owner chooses (status of that part: needs-owner); changing it after the first public build moves data directories. Windows builds are unsigned (D-087, decided 2026-10-04): the SmartScreen warning is documented as expected in the release notes and the user guide, and checksums and provenance files carry the trust. The macOS notarisation path is not covered by that decision and stays open.

## Consequences

- The day-one release checklist (12 items) becomes the M7 exit gate.
- MSI and delta updates are not offered.
- Portable mode disables the Windows updater.

## Alternatives rejected

- MSI.
- Azure Artifact Signing or an OV certificate for Windows (declined by the owner).
- Delta updates.
- Choosing an identifier silently.

## Evidence

- [Cross-platform packaging](../research/cross-platform-packaging-research.md) section 9 and implications 1 to 4, 12
- [Cross-platform architecture](../architecture/cross-platform.md)
