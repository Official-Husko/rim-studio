# ADR 0016: RemoteDataset pipeline and runtime-only community datasets

Status: proposed | Last updated: 2026-10-04 | Register: D-032, D-034, D-035

## Context

R6 requires the community sorting rules and related datasets (SteamDB, Use This Instead, version lists) with auto-fetching, and R11 forbids bundling unlicensed data. The data repositories carry no licence file. SteamDB is large; the slim index loads in 25 ms with 18 MB RSS.

## Decision

`rimstudio-datasets` holds one generic `RemoteDataset` pipeline, the only HTTP client in the product (reqwest), driven by data-defined descriptors for the five datasets: conditional GET storing the ETag together with the encoding used, size and compression-ratio caps, download to a temp file then rotate `current` to `previous`, quarantine of collapsed data and the last good copy always kept. The raw URL is primary; the jsDelivr mirror is used only for files under 20 MB. SteamDB is stored as a compressed raw artifact plus a locally built slim JSON index. Nothing is bundled or mirrored, including the MIT-licensed Use This Instead and No Version Warning (status proposed for those two), and replacements match by workshop id first. Asking the RimSort maintainers for a licence is an owner action.

## Consequences

- A first offline run shows no replacements until one fetch has happened; a seed can be added later.
- One state JSON per dataset records etag, time and checks.
- A hosted compact mirror stays disabled until permission exists.

## Alternatives rejected

- Bundle the datasets: licence and staleness.
- Host a RimStudio mirror: licence.
- A git clone as source; bespoke code per dataset.

## Evidence

- [Rules fetch and merge design](../research/rules-fetch-and-merge-design.md) implications 1 to 5
- [Community datasets analysis](../research/community-datasets-analysis.md) implications 5, 6 and 8
- [Community datasets feature spec](../features/community-datasets.md)
