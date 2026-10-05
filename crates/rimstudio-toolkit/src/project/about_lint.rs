//! Findings about the basics of a mod: the values of `About.xml`, the preview image and the neighbours in
//! the scanned library.
//!
//! The rules follow what the game does with the file (`docs/research/rimworld-mod-format-and-corpus.md`,
//! section 1): an id that breaks the format rule is kept but flagged, a dependency without a name or a link is
//! silently dropped by the game, a version that does not match the running game makes the mod show as
//! outdated. Every finding has a stable `about.*` code and, when it concerns one field, a `field` pointer
//! (`/packageId`, `/modDependencies/1/packageId`). Findings never block a save; this module only reads.

use std::collections::BTreeSet;

use camino::Utf8Path;
use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::ids::{PackageId, PackageIdProblem};
use rimstudio_core::mods::ModDependency;
use rimstudio_core::paths::is_official_package_id;
use rimstudio_core::version::{GameVersion, parse_major_minor};
use rimstudio_ipc_types::diagnostic::FIELD_ARG;
use rimstudio_library::index::LibraryIndex;
use rimstudio_xml::about::About;

/// The longest description (characters) that is not flagged; Steam cuts the Workshop description there.
pub const DESCRIPTION_LONG_CHARS: usize = 8000;
/// The largest preview image (bytes) that is not flagged; Steam refuses a larger preview.
pub const PREVIEW_MAX_BYTES_STEAM: u64 = 1_000_000;
/// The recommended preview size.
pub const PREVIEW_SIZE: (u32, u32) = (640, 360);

/// `about.name-missing`
pub const NAME_MISSING: DiagCode = DiagCode::new("about.name-missing");
/// `about.author-missing`
pub const AUTHOR_MISSING: DiagCode = DiagCode::new("about.author-missing");
/// `about.package-id-missing`
pub const PACKAGE_ID_MISSING: DiagCode = DiagCode::new("about.package-id-missing");
/// `about.package-id-format`
pub const PACKAGE_ID_FORMAT: DiagCode = DiagCode::new("about.package-id-format");
/// `about.package-id-case`
pub const PACKAGE_ID_CASE: DiagCode = DiagCode::new("about.package-id-case");
/// `about.package-id-steam-suffix`
pub const PACKAGE_ID_STEAM_SUFFIX: DiagCode = DiagCode::new("about.package-id-steam-suffix");
/// `about.package-id-ludeon`
pub const PACKAGE_ID_LUDEON: DiagCode = DiagCode::new("about.package-id-ludeon");
/// `about.package-id-collision`
pub const PACKAGE_ID_COLLISION: DiagCode = DiagCode::new("about.package-id-collision");
/// `about.package-id-case-conflict`
pub const PACKAGE_ID_CASE_CONFLICT: DiagCode = DiagCode::new("about.package-id-case-conflict");
/// `about.supported-versions-empty`
pub const VERSIONS_EMPTY: DiagCode = DiagCode::new("about.supported-versions-empty");
/// `about.supported-versions-malformed`
pub const VERSIONS_MALFORMED: DiagCode = DiagCode::new("about.supported-versions-malformed");
/// `about.supported-versions-build`
pub const VERSIONS_BUILD: DiagCode = DiagCode::new("about.supported-versions-build");
/// `about.supported-versions-missing-game`
pub const VERSIONS_MISSING_GAME: DiagCode = DiagCode::new("about.supported-versions-missing-game");
/// `about.dependency-package-id-format`
pub const DEPENDENCY_ID_FORMAT: DiagCode = DiagCode::new("about.dependency-package-id-format");
/// `about.dependency-no-display-name`
pub const DEPENDENCY_NO_NAME: DiagCode = DiagCode::new("about.dependency-no-display-name");
/// `about.dependency-no-url`
pub const DEPENDENCY_NO_URL: DiagCode = DiagCode::new("about.dependency-no-url");
/// `about.dependency-url-malformed`
pub const DEPENDENCY_URL_MALFORMED: DiagCode = DiagCode::new("about.dependency-url-malformed");
/// `about.dependency-unknown`
pub const DEPENDENCY_UNKNOWN: DiagCode = DiagCode::new("about.dependency-unknown");
/// `about.self-reference`
pub const SELF_REFERENCE: DiagCode = DiagCode::new("about.self-reference");
/// `about.load-order-contradiction`
pub const LOAD_ORDER_CONTRADICTION: DiagCode = DiagCode::new("about.load-order-contradiction");
/// `about.url-malformed`
pub const URL_MALFORMED: DiagCode = DiagCode::new("about.url-malformed");
/// `about.icon-missing`
pub const ICON_MISSING: DiagCode = DiagCode::new("about.icon-missing");
/// `about.preview-missing`
pub const PREVIEW_MISSING: DiagCode = DiagCode::new("about.preview-missing");
/// `about.preview-invalid`
pub const PREVIEW_INVALID: DiagCode = DiagCode::new("about.preview-invalid");
/// `about.preview-too-large`
pub const PREVIEW_TOO_LARGE: DiagCode = DiagCode::new("about.preview-too-large");
/// `about.preview-dimensions`
pub const PREVIEW_DIMENSIONS: DiagCode = DiagCode::new("about.preview-dimensions");
/// `about.preview-case`
pub const PREVIEW_CASE: DiagCode = DiagCode::new("about.preview-case");
/// `about.description-empty`
pub const DESCRIPTION_EMPTY: DiagCode = DiagCode::new("about.description-empty");
/// `about.description-long`
pub const DESCRIPTION_LONG: DiagCode = DiagCode::new("about.description-long");
/// `about.description-size-tag`
pub const DESCRIPTION_SIZE_TAG: DiagCode = DiagCode::new("about.description-size-tag");
/// `about.unparseable`
pub const UNPARSEABLE: DiagCode = DiagCode::new("about.unparseable");
/// `about.not-utf8`
pub const NOT_UTF8: DiagCode = DiagCode::new("about.not-utf8");
/// `about.too-large`
pub const TOO_LARGE: DiagCode = DiagCode::new("about.too-large");
/// `about.leading-whitespace`
pub const LEADING_WHITESPACE: DiagCode = DiagCode::new("about.leading-whitespace");

/// What the lint knows about the preview image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreviewFacts {
    /// A file exists.
    pub exists: bool,
    /// The file name as it is on disk when it is not exactly `Preview.png`.
    pub other_spelling: Option<String>,
    /// Size in bytes.
    pub bytes: u64,
    /// The file is a PNG with a readable header, with its size in pixels; `None` when it is not.
    pub dimensions: Option<(u32, u32)>,
}

/// Everything besides the About values that the findings depend on.
pub struct LintContext<'a> {
    /// The package ids of mods in the last scan outside the project: `(index, project root)`. `None` when
    /// there was no scan.
    pub library: Option<(&'a LibraryIndex, &'a Utf8Path)>,
    /// The installed game version, when known.
    pub game_version: Option<&'a GameVersion>,
    /// The preview image on disk.
    pub preview: &'a PreviewFacts,
    /// Whether the texture named by `modIconPath` exists in the mod.
    pub icon_exists: &'a dyn Fn(&str) -> bool,
}

fn finding(
    code: DiagCode,
    severity: Severity,
    field: &str,
    message: impl Into<String>,
) -> Diagnostic {
    let d = Diagnostic::new(code, severity, message);
    if field.is_empty() {
        d
    } else {
        d.with_arg(FIELD_ARG, field)
    }
}

fn problem_text(p: PackageIdProblem) -> &'static str {
    match p {
        PackageIdProblem::TooLong => "it is longer than 60 characters",
        PackageIdProblem::BadChar => {
            "it may hold only the letters A to Z, digits and dots (no underscore, hyphen or space)"
        }
        PackageIdProblem::LeadingDot => "it starts with a dot",
        PackageIdProblem::NoDot => "it needs a dot, in the form author.modname",
        PackageIdProblem::EmptySegment => "it has two dots in a row",
        PackageIdProblem::BadEnd => "it must end with a letter or a digit",
    }
}

/// Why `id` breaks the game's package id format rule, `None` when it follows it.
#[must_use]
pub fn package_id_problem(id: &str) -> Option<&'static str> {
    match PackageId::parse(id) {
        Ok(p) => p.check_game_format().err().map(problem_text),
        Err(_) => Some("it is empty or holds a control character"),
    }
}

/// True for a text that can be a web address: `http`, `https` or `steam` (the Workshop link form
/// `steam://url/CommunityFilePage/<id>` that most mods use), a host and no white space. The check is
/// deliberately loose (`https://nope` is what some mods write for "no page", and the game accepts it).
#[must_use]
pub fn looks_like_url(text: &str) -> bool {
    let t = text.trim();
    let Some(rest) = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .or_else(|| t.strip_prefix("steam://"))
    else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !t.chars().any(char::is_whitespace) && !host.is_empty() && !host.starts_with('.')
}

fn same_id(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn contains_id(list: &[String], id: &str) -> bool {
    list.iter().any(|x| same_id(x, id))
}

/// All findings for the values of one About file. `present` lists the root elements of the file (for the
/// difference between a missing and an empty field); `about` is what the reader made of it.
#[must_use]
pub fn lint(about: &About, present: &BTreeSet<String>, ctx: &LintContext<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    lint_identity(about, present, &mut out);
    lint_package_id(about, ctx, &mut out);
    lint_versions(about, ctx, &mut out);
    lint_relations(about, &mut out);
    lint_dependencies(about, ctx, &mut out);
    lint_text(about, ctx, &mut out);
    lint_preview(ctx.preview, &mut out);
    out
}

fn lint_identity(about: &About, present: &BTreeSet<String>, out: &mut Vec<Diagnostic>) {
    if about.name.trim().is_empty() {
        let why = if present.contains("name") {
            "the name is empty"
        } else {
            "the file has no name"
        };
        out.push(finding(
            NAME_MISSING,
            Severity::Warning,
            "/name",
            format!("{why}; the mod list shows the folder name"),
        ));
    }
    let has_author = about
        .author
        .as_deref()
        .is_some_and(|a| !a.trim().is_empty())
        || about.authors.iter().any(|a| !a.trim().is_empty());
    if !has_author {
        out.push(finding(
            AUTHOR_MISSING,
            Severity::Warning,
            "/author",
            "the file names no author",
        ));
    }
}

fn lint_package_id(about: &About, ctx: &LintContext<'_>, out: &mut Vec<Diagnostic>) {
    let id = about.package_id.trim();
    if id.is_empty() {
        out.push(finding(
            PACKAGE_ID_MISSING,
            Severity::Error,
            "/packageId",
            "the packageId is empty; other mods and the load order cannot refer to this mod",
        ));
        return;
    }
    if let Some(why) = package_id_problem(id) {
        out.push(
            finding(
                PACKAGE_ID_FORMAT,
                Severity::Error,
                "/packageId",
                format!("the packageId {id} breaks the game's format rule: {why}"),
            )
            .with_arg("reason", why),
        );
    }
    let lower = id.to_lowercase();
    if lower.ends_with("_steam") || lower.ends_with(".steam") {
        out.push(finding(
            PACKAGE_ID_STEAM_SUFFIX,
            Severity::Warning,
            "/packageId",
            format!("the packageId {id} ends in a steam suffix; the game adds that suffix itself to the Workshop copy of a duplicated id"),
        ));
    }
    if id.chars().any(|c| c.is_ascii_uppercase()) {
        out.push(finding(
            PACKAGE_ID_CASE,
            Severity::Hint,
            "/packageId",
            format!("the packageId {id} has capital letters; the game compares ids ignoring case, so the lower case form {lower} is the usual way to write it"),
        ));
    }
    if lower.contains("ludeon") && !is_official_package_id(&lower) {
        out.push(finding(
            PACKAGE_ID_LUDEON,
            Severity::Warning,
            "/packageId",
            "the game warns about a packageId that contains the word ludeon",
        ));
    }
    lint_collisions(id, ctx, out);
}

/// The findings about one package id (format, hints and the collision with another mod of the last scan),
/// for a caller that has no About file yet (the new mod window).
#[must_use]
pub fn package_id_findings(id: &str, ctx: &LintContext<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let about = About {
        package_id: id.to_owned(),
        ..About::default()
    };
    lint_package_id(&about, ctx, &mut out);
    out
}

fn lint_collisions(id: &str, ctx: &LintContext<'_>, out: &mut Vec<Diagnostic>) {
    let Some((index, root)) = ctx.library else {
        return;
    };
    let mut seen = BTreeSet::new();
    for idx in index.by_package_id(id) {
        let (Some(meta), Some(info)) = (index.get(*idx), index.info(*idx)) else {
            continue;
        };
        let inside = meta.path == root || meta.path.starts_with(root);
        if inside || !info.available || !seen.insert(meta.path.clone()) {
            continue;
        }
        let other = meta.package_id.as_str();
        let (code, how) = if other == id {
            (PACKAGE_ID_COLLISION, "uses the same packageId")
        } else {
            (
                PACKAGE_ID_CASE_CONFLICT,
                "uses the same packageId in other letter case",
            )
        };
        out.push(
            finding(
                code,
                Severity::Warning,
                "/packageId",
                format!(
                    "the mod \"{}\" in {} {how} ({other}); the game treats the two as one mod",
                    meta.name, meta.path
                ),
            )
            .with_arg("otherName", meta.name.clone())
            .with_arg("otherPath", meta.path.as_str())
            .with_arg("otherSource", meta.source.to_string()),
        );
    }
}

fn lint_versions(about: &About, ctx: &LintContext<'_>, out: &mut Vec<Diagnostic>) {
    if about.supported_versions.is_empty() {
        let why = if about.has_supported_versions {
            "the supportedVersions list is empty"
        } else {
            "the file has no supportedVersions list"
        };
        out.push(finding(
            VERSIONS_EMPTY,
            Severity::Warning,
            "/supportedVersions",
            format!("{why}; the game shows the mod as made for no version"),
        ));
        return;
    }
    for (i, entry) in about.supported_versions.iter().enumerate() {
        let field = format!("/supportedVersions/{i}");
        match parse_major_minor(entry, false) {
            None => out.push(finding(
                VERSIONS_MALFORMED,
                Severity::Error,
                &field,
                format!("\"{entry}\" is not a version the game reads; write it as major.minor, for example 1.6"),
            )),
            Some(_) if entry.trim().split('.').count() > 2 => out.push(finding(
                VERSIONS_BUILD,
                Severity::Info,
                &field,
                format!("the game compares only major.minor, so \"{entry}\" counts as the first two numbers"),
            )),
            Some(_) => {}
        }
    }
    if let Some(game) = ctx.game_version
        && !game.supported_matches_game_exact(&about.supported_versions)
    {
        let (major, minor) = game.major_minor();
        out.push(
            finding(
                VERSIONS_MISSING_GAME,
                Severity::Warning,
                "/supportedVersions",
                format!("the installed game is {major}.{minor} and the list does not name it; the game shows the mod as made for another version"),
            )
            .with_arg("gameVersion", format!("{major}.{minor}")),
        );
    }
}

fn lint_relations(about: &About, out: &mut Vec<Diagnostic>) {
    let own = about.package_id.trim();
    let lists: [(&str, &[String]); 5] = [
        ("loadBefore", &about.load_before),
        ("loadAfter", &about.load_after),
        ("forceLoadBefore", &about.force_load_before),
        ("forceLoadAfter", &about.force_load_after),
        ("incompatibleWith", &about.incompatible_with),
    ];
    if !own.is_empty() {
        for (name, list) in lists {
            for (i, id) in list.iter().enumerate() {
                if same_id(id, own) {
                    out.push(finding(
                        SELF_REFERENCE,
                        Severity::Warning,
                        &format!("/{name}/{i}"),
                        format!("{name} names the mod itself ({id})"),
                    ));
                }
            }
        }
    }
    for (i, id) in about.load_before.iter().enumerate() {
        if contains_id(&about.load_after, id) {
            out.push(finding(
                LOAD_ORDER_CONTRADICTION,
                Severity::Warning,
                &format!("/loadBefore/{i}"),
                format!("{id} is in loadBefore and in loadAfter; a mod cannot load both before and after another"),
            ));
        }
    }
    for (i, dep) in about.mod_dependencies.iter().enumerate() {
        if contains_id(&about.load_before, &dep.package_id) {
            out.push(finding(
                LOAD_ORDER_CONTRADICTION,
                Severity::Warning,
                &format!("/modDependencies/{i}/packageId"),
                format!("{} is a dependency and is also in loadBefore; a mod must load after what it requires", dep.package_id),
            ));
        }
        if contains_id(&about.incompatible_with, &dep.package_id) {
            out.push(finding(
                LOAD_ORDER_CONTRADICTION,
                Severity::Warning,
                &format!("/modDependencies/{i}/packageId"),
                format!(
                    "{} is a dependency and is also in incompatibleWith",
                    dep.package_id
                ),
            ));
        }
    }
}

fn dependency_findings(
    dep: &ModDependency,
    pointer: &str,
    own: &str,
    ctx: &LintContext<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let id = dep.package_id.trim();
    if let Some(why) = package_id_problem(id) {
        out.push(
            finding(
                DEPENDENCY_ID_FORMAT,
                Severity::Warning,
                &format!("{pointer}/packageId"),
                format!(
                    "the dependency {id} breaks the packageId format rule: {why}; the game drops it"
                ),
            )
            .with_arg("reason", why),
        );
    }
    if !own.is_empty() && same_id(id, own) {
        out.push(finding(
            SELF_REFERENCE,
            Severity::Warning,
            &format!("{pointer}/packageId"),
            "the mod lists itself as a dependency",
        ));
    }
    let official = is_official_package_id(id);
    if dep.display_name.trim().is_empty() && !official {
        out.push(finding(
            DEPENDENCY_NO_NAME,
            Severity::Warning,
            &format!("{pointer}/displayName"),
            format!("the dependency {id} has no displayName; the game drops it"),
        ));
    }
    let workshop = dep
        .steam_workshop_url
        .as_deref()
        .filter(|u| !u.trim().is_empty());
    let download = dep.download_url.as_deref().filter(|u| !u.trim().is_empty());
    if workshop.is_none() && download.is_none() && !official {
        out.push(finding(
            DEPENDENCY_NO_URL,
            Severity::Warning,
            &format!("{pointer}/steamWorkshopUrl"),
            format!("the dependency {id} has neither a steamWorkshopUrl nor a downloadUrl; the game drops it"),
        ));
    }
    for (tag, url) in [("steamWorkshopUrl", workshop), ("downloadUrl", download)] {
        if let Some(u) = url
            && !looks_like_url(u)
        {
            out.push(finding(
                DEPENDENCY_URL_MALFORMED,
                Severity::Warning,
                &format!("{pointer}/{tag}"),
                format!("\"{u}\" is not a web address (it should start with https://)"),
            ));
        }
    }
    if let Some((index, _)) = ctx.library
        && !official
        && !index.is_empty()
        && !id.is_empty()
        && index.by_package_id(id).is_empty()
        && !dep
            .alternatives
            .iter()
            .any(|a| !index.by_package_id(a).is_empty())
    {
        out.push(finding(
            DEPENDENCY_UNKNOWN,
            Severity::Info,
            &format!("{pointer}/packageId"),
            format!("no mod with the packageId {id} is in the scanned library"),
        ));
    }
}

fn lint_dependencies(about: &About, ctx: &LintContext<'_>, out: &mut Vec<Diagnostic>) {
    let own = about.package_id.trim();
    for (i, dep) in about.mod_dependencies.iter().enumerate() {
        dependency_findings(dep, &format!("/modDependencies/{i}"), own, ctx, out);
    }
    for (version, rel) in &about.by_version {
        for (i, dep) in rel.mod_dependencies.iter().enumerate() {
            dependency_findings(
                dep,
                &format!("/modDependenciesByVersion/{version}/{i}"),
                own,
                ctx,
                out,
            );
        }
    }
}

fn lint_text(about: &About, ctx: &LintContext<'_>, out: &mut Vec<Diagnostic>) {
    if let Some(url) = about.url.as_deref().filter(|u| !u.trim().is_empty())
        && !looks_like_url(url)
    {
        out.push(finding(
            URL_MALFORMED,
            Severity::Warning,
            "/url",
            format!("\"{url}\" is not a web address (it should start with https://)"),
        ));
    }
    if let Some(icon) = about
        .mod_icon_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        && !(ctx.icon_exists)(icon.trim())
    {
        out.push(finding(
            ICON_MISSING,
            Severity::Warning,
            "/modIconPath",
            format!("no texture {icon} was found below Textures in the mod"),
        ));
    }
    let description = about.description.trim();
    if description.is_empty() {
        out.push(finding(
            DESCRIPTION_EMPTY,
            Severity::Warning,
            "/description",
            "the description is empty; it is shown in the mod list and on the Workshop page",
        ));
    } else {
        let chars = description.chars().count();
        if chars > DESCRIPTION_LONG_CHARS {
            out.push(
                finding(
                    DESCRIPTION_LONG,
                    Severity::Warning,
                    "/description",
                    format!("the description is {chars} characters; Steam keeps only {DESCRIPTION_LONG_CHARS}"),
                )
                .with_arg("length", chars.to_string()),
            );
        }
        if description.to_lowercase().contains("<size=") {
            out.push(finding(
                DESCRIPTION_SIZE_TAG,
                Severity::Info,
                "/description",
                "the description has a size tag; the in game mod list shows it as plain text",
            ));
        }
    }
}

pub(crate) fn lint_preview(preview: &PreviewFacts, out: &mut Vec<Diagnostic>) {
    if !preview.exists {
        out.push(finding(
            PREVIEW_MISSING,
            Severity::Warning,
            "",
            "About/Preview.png is missing; the Workshop page needs a preview image",
        ));
        return;
    }
    if let Some(name) = &preview.other_spelling {
        out.push(finding(
            PREVIEW_CASE,
            Severity::Warning,
            "",
            format!(
                "the preview image is named {name}; the game and the uploader look for Preview.png"
            ),
        ));
    }
    match preview.dimensions {
        None => out.push(finding(
            PREVIEW_INVALID,
            Severity::Info,
            "",
            "About/Preview.png is not a PNG image (it may be another format with a .png name; the game still reads most of them)",
        )),
        Some(dims) if dims != PREVIEW_SIZE => {
            // the same picture ratio is only a note: the Workshop page crops anything else
            let widescreen = u64::from(dims.0) * 9 * 100 / 16 / u64::from(dims.1.max(1));
            let severity = if (98..=102).contains(&widescreen) {
                Severity::Info
            } else {
                Severity::Warning
            };
            out.push(
                finding(
                    PREVIEW_DIMENSIONS,
                    severity,
                    "",
                    format!(
                        "the preview image is {} by {} pixels; {} by {} is the size the Workshop page shows without cropping",
                        dims.0, dims.1, PREVIEW_SIZE.0, PREVIEW_SIZE.1
                    ),
                )
                .with_arg("width", dims.0.to_string())
                .with_arg("height", dims.1.to_string()),
            );
        }
        Some(_) => {}
    }
    if preview.bytes > PREVIEW_MAX_BYTES_STEAM {
        out.push(
            finding(
                PREVIEW_TOO_LARGE,
                Severity::Warning,
                "",
                format!(
                    "the preview image is {} bytes; Steam refuses a preview above 1 MB",
                    preview.bytes
                ),
            )
            .with_arg("bytes", preview.bytes.to_string()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_xml::about::read_lenient;

    fn run(xml: &str, preview: &PreviewFacts, game: Option<&GameVersion>) -> Vec<Diagnostic> {
        let read = read_lenient(xml.as_bytes());
        let present = BTreeSet::new();
        let icon = |_: &str| false;
        lint(
            &read.about,
            &present,
            &LintContext {
                library: None,
                game_version: game,
                preview,
                icon_exists: &icon,
            },
        )
    }

    fn codes(d: &[Diagnostic]) -> Vec<&str> {
        d.iter().map(|d| d.code.as_str()).collect()
    }

    fn good_preview() -> PreviewFacts {
        PreviewFacts {
            exists: true,
            other_spelling: None,
            bytes: 100_000,
            dimensions: Some(PREVIEW_SIZE),
        }
    }

    #[test]
    fn a_clean_file_has_no_findings() {
        let xml = "<ModMetaData><name>A</name><author>B</author><packageId>b.a</packageId><supportedVersions><li>1.6</li></supportedVersions><description>Does things.</description></ModMetaData>";
        let game = GameVersion::parse("1.6.4630").unwrap();
        assert_eq!(
            codes(&run(xml, &good_preview(), Some(&game))),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn package_id_rules() {
        assert_eq!(package_id_problem("author.mod"), None);
        assert!(package_id_problem("mod").is_some());
        assert!(package_id_problem("a b.c").is_some());
        assert!(package_id_problem("a_b.c").is_some());
        assert!(package_id_problem(".a.b").is_some());
        assert!(package_id_problem("a..b").is_some());
        assert!(package_id_problem("a.b.").is_some());
        assert!(package_id_problem(&format!("a.{}", "b".repeat(70))).is_some());
        let d = run(
            "<ModMetaData><name>A</name><author>B</author><packageId>Dubwise.Thing.steam</packageId><supportedVersions><li>1.6</li></supportedVersions><description>x</description></ModMetaData>",
            &good_preview(),
            None,
        );
        assert!(codes(&d).contains(&"about.package-id-steam-suffix"));
        assert!(codes(&d).contains(&"about.package-id-case"));
        let d = run(
            "<ModMetaData><name>A</name></ModMetaData>",
            &good_preview(),
            None,
        );
        assert!(codes(&d).contains(&"about.package-id-missing"));
        assert!(codes(&d).contains(&"about.author-missing"));
        assert!(codes(&d).contains(&"about.supported-versions-empty"));
        assert!(codes(&d).contains(&"about.description-empty"));
    }

    #[test]
    fn versions_are_checked_against_the_installed_game() {
        let game = GameVersion::parse("1.6.4630").unwrap();
        let xml = "<ModMetaData><name>A</name><author>B</author><packageId>b.a</packageId><supportedVersions><li>1.5</li><li>v1.6</li><li>1.5.4000</li></supportedVersions><description>x</description></ModMetaData>";
        let d = run(xml, &good_preview(), Some(&game));
        let c = codes(&d);
        assert!(c.contains(&"about.supported-versions-malformed"));
        assert!(c.contains(&"about.supported-versions-build"));
        assert!(c.contains(&"about.supported-versions-missing-game"));
        let miss = d.iter().find(|d| d.code == VERSIONS_MISSING_GAME).unwrap();
        assert_eq!(
            miss.args.get("gameVersion").map(String::as_str),
            Some("1.6")
        );
    }

    #[test]
    fn relations_and_dependencies() {
        let xml = "<ModMetaData><name>A</name><author>B</author><packageId>b.a</packageId><supportedVersions><li>1.6</li></supportedVersions><description>x</description>\
            <loadAfter><li>B.A</li><li>x.y</li></loadAfter><loadBefore><li>x.y</li></loadBefore>\
            <modDependencies><li><packageId>b.a</packageId><displayName>Me</displayName></li><li><packageId>bad id</packageId><displayName>Bad</displayName><downloadUrl>nope</downloadUrl></li></modDependencies></ModMetaData>";
        let d = run(xml, &good_preview(), None);
        let c = codes(&d);
        assert!(c.contains(&"about.self-reference"));
        assert!(c.contains(&"about.load-order-contradiction"));
        assert!(c.contains(&"about.dependency-package-id-format"));
        assert!(c.contains(&"about.dependency-url-malformed"));
        assert!(c.contains(&"about.dependency-no-url"));
        let dep = d.iter().find(|d| d.code == DEPENDENCY_ID_FORMAT).unwrap();
        assert_eq!(
            dep.args.get("field").map(String::as_str),
            Some("/modDependencies/1/packageId")
        );
    }

    #[test]
    fn urls_the_icon_and_the_description() {
        assert!(looks_like_url("https://example.invalid/x"));
        assert!(looks_like_url("http://a.b"));
        assert!(!looks_like_url("example.com"));
        assert!(looks_like_url("https://nope"));
        assert!(looks_like_url("steam://url/CommunityFilePage/2009463077"));
        assert!(!looks_like_url("https://"));
        assert!(!looks_like_url("http:///path"));
        assert!(!looks_like_url("https://a.b/with space"));
        let long = "x".repeat(DESCRIPTION_LONG_CHARS + 1);
        let xml = format!(
            "<ModMetaData><name>A</name><author>B</author><packageId>b.a</packageId><supportedVersions><li>1.6</li></supportedVersions><url>not a url</url><modIconPath>Icon/Gone</modIconPath><description>{long}</description></ModMetaData>"
        );
        let c = codes(&run(&xml, &good_preview(), None))
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(c.contains(&"about.url-malformed".to_owned()));
        assert!(c.contains(&"about.icon-missing".to_owned()));
        assert!(c.contains(&"about.description-long".to_owned()));
    }

    #[test]
    fn the_preview_is_checked() {
        let xml = "<ModMetaData><name>A</name><author>B</author><packageId>b.a</packageId><supportedVersions><li>1.6</li></supportedVersions><description>x</description></ModMetaData>";
        assert!(
            codes(&run(xml, &PreviewFacts::default(), None)).contains(&"about.preview-missing")
        );
        let odd = PreviewFacts {
            exists: true,
            other_spelling: Some("preview.png".into()),
            bytes: 2_000_000,
            dimensions: Some((100, 100)),
        };
        let c = codes(&run(xml, &odd, None))
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for want in [
            "about.preview-case",
            "about.preview-dimensions",
            "about.preview-too-large",
        ] {
            assert!(c.contains(&want.to_owned()), "{want}");
        }
        let bad = PreviewFacts {
            exists: true,
            bytes: 10,
            ..PreviewFacts::default()
        };
        assert!(codes(&run(xml, &bad, None)).contains(&"about.preview-invalid"));
    }

    #[test]
    fn every_code_is_well_formed() {
        for code in [
            NAME_MISSING,
            AUTHOR_MISSING,
            PACKAGE_ID_MISSING,
            PACKAGE_ID_FORMAT,
            PACKAGE_ID_CASE,
            PACKAGE_ID_STEAM_SUFFIX,
            PACKAGE_ID_LUDEON,
            PACKAGE_ID_COLLISION,
            PACKAGE_ID_CASE_CONFLICT,
            VERSIONS_EMPTY,
            VERSIONS_MALFORMED,
            VERSIONS_BUILD,
            VERSIONS_MISSING_GAME,
            DEPENDENCY_ID_FORMAT,
            DEPENDENCY_NO_NAME,
            DEPENDENCY_NO_URL,
            DEPENDENCY_URL_MALFORMED,
            DEPENDENCY_UNKNOWN,
            SELF_REFERENCE,
            LOAD_ORDER_CONTRADICTION,
            URL_MALFORMED,
            ICON_MISSING,
            PREVIEW_MISSING,
            PREVIEW_INVALID,
            PREVIEW_TOO_LARGE,
            PREVIEW_DIMENSIONS,
            PREVIEW_CASE,
            DESCRIPTION_EMPTY,
            DESCRIPTION_LONG,
            DESCRIPTION_SIZE_TAG,
            UNPARSEABLE,
            NOT_UTF8,
            TOO_LARGE,
            LEADING_WHITESPACE,
        ] {
            assert!(code.is_well_formed(), "{}", code.as_str());
        }
    }
}
