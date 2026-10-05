//! Game versions and the game's version matching rules.
//!
//! RimWorld reports its version as `1.6.4871 rev598`: major, minor, build and revision. Mods declare
//! the versions they support as `major.minor` strings and the game compares only major and minor
//! (`supported_matches`). Version keys in `ByVersion` elements and `LoadFolders.xml` blocks are
//! lower cased with one leading `v` removed ([`normalize_version_key`]).

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CoreError, VersionFault};

/// A game version with the original text kept for display.
///
/// Equality, ordering and hashing use the four numeric parts only, never `raw`, so `1.6.4871 rev598`
/// and `v1.6.4871 rev 598` are the same version. A missing build or revision orders before any
/// present one.
#[derive(Debug, Clone)]
pub struct GameVersion {
    /// The text this version was parsed from (trimmed), or the canonical text when built by
    /// [`GameVersion::new`].
    pub raw: String,
    /// The major number, `1` for RimWorld 1.x.
    pub major: u32,
    /// The minor number, `6` for RimWorld 1.6.
    pub minor: u32,
    /// The build number, absent in a short version such as `1.6`.
    pub build: Option<u32>,
    /// The revision, absent unless written as `revN`.
    pub rev: Option<u32>,
}

/// The numeric parts of a dotted version, as parsed.
struct Dotted {
    parts: Vec<u32>,
}

fn parse_u32(text: &str) -> Option<u32> {
    let t = text.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<u32>().ok()
}

fn parse_dotted(text: &str, max_parts: usize) -> Result<Dotted, VersionFault> {
    let mut parts = Vec::new();
    for piece in text.split('.') {
        parts.push(parse_u32(piece).ok_or(VersionFault::BadNumber)?);
    }
    if parts.len() < 2 {
        return Err(VersionFault::TooFewParts);
    }
    if parts.len() > max_parts {
        return Err(VersionFault::TooManyParts);
    }
    Ok(Dotted { parts })
}

/// Parses `rev598`, `rev 598`, `r598` or `598` style revision text. `None` when it is not one.
fn parse_rev(text: &str) -> Option<u32> {
    let t = text.trim().trim_matches(|c| c == '(' || c == ')').trim();
    let lower = t.to_ascii_lowercase();
    let digits = lower
        .strip_prefix("rev")
        .or_else(|| lower.strip_prefix('r'))
        .unwrap_or(&lower);
    parse_u32(digits.trim_start_matches(['.', ':', ' ']))
}

impl GameVersion {
    /// Builds a version from its numeric parts; `raw` becomes the canonical text.
    pub fn new(major: u32, minor: u32, build: Option<u32>, rev: Option<u32>) -> Self {
        let mut v = GameVersion {
            raw: String::new(),
            major,
            minor,
            build,
            rev,
        };
        v.raw = v.canonical();
        v
    }

    /// Parses a game version, tolerating the usual variants.
    ///
    /// Accepted: `1.6.4871 rev598` (the game's own form), `1.6.4871`, `1.6`, a leading `v` or `V`,
    /// a byte order mark, surrounding whitespace and newlines (as in `Version.txt`), `rev 598`,
    /// `(rev598)`, and the four number form `1.6.4871.598`. Rejected: fewer than two numeric parts,
    /// non numeric parts and unknown trailing text.
    ///
    /// # Errors
    /// [`CoreError::InvalidVersion`] with the reason.
    pub fn parse(input: &str) -> Result<GameVersion, CoreError> {
        let fail = |fault| CoreError::invalid_version(input, fault);
        let trimmed = input.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            return Err(fail(VersionFault::Empty));
        }
        let text = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
        let (numeric, rest) = match text.find(char::is_whitespace) {
            Some(i) => (&text[..i], text[i..].trim()),
            None => (text, ""),
        };
        let dotted = parse_dotted(numeric, 4).map_err(fail)?;
        let mut rev = None;
        if !rest.is_empty() {
            rev = Some(parse_rev(rest).ok_or_else(|| fail(VersionFault::UnexpectedSuffix))?);
        }
        let build = dotted.parts.get(2).copied();
        if let Some(fourth) = dotted.parts.get(3).copied() {
            if rev.is_some() {
                return Err(fail(VersionFault::TooManyParts));
            }
            rev = Some(fourth);
        }
        Ok(GameVersion {
            raw: trimmed.to_owned(),
            major: dotted.parts[0],
            minor: dotted.parts[1],
            build,
            rev,
        })
    }

    /// The canonical text: `major.minor`, then `.build` when known, then ` revN` when known.
    pub fn canonical(&self) -> String {
        let mut s = format!("{}.{}", self.major, self.minor);
        if let Some(b) = self.build {
            s.push_str(&format!(".{b}"));
        }
        if let Some(r) = self.rev {
            s.push_str(&format!(" rev{r}"));
        }
        s
    }

    /// The `major.minor` text, for example `1.6`, used for version folders and `ByVersion` keys.
    pub fn short(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// The `major.minor.build` text (build `0` when unknown), the game's `CurrentVersionString`.
    pub fn with_build(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.build.unwrap_or(0))
    }

    /// The major and minor numbers as a pair.
    pub fn major_minor(&self) -> (u32, u32) {
        (self.major, self.minor)
    }

    /// True when `other` has the same major and minor number (build and revision are ignored).
    pub fn same_major_minor(&self, other: &GameVersion) -> bool {
        self.major_minor() == other.major_minor()
    }

    /// The game's compatibility rule for one `supportedVersions` entry: exact `major.minor`
    /// equality, ignoring build and revision, after removing one leading `v` or `V`.
    ///
    /// Entries with fewer than two numeric parts never match. Parts beyond the second are ignored,
    /// so `1.6.4871` matches game 1.6.
    pub fn supports_entry(&self, entry: &str) -> bool {
        parse_major_minor(entry, true) == Some(self.major_minor())
    }

    /// True when any entry of `supported` matches ([`GameVersion::supports_entry`]).
    pub fn supported_matches<S: AsRef<str>>(&self, supported: &[S]) -> bool {
        supported.iter().any(|s| self.supports_entry(s.as_ref()))
    }

    /// Like [`GameVersion::supported_matches`] but exactly as the game parses entries: a leading
    /// `v` is not stripped, so `v1.6` never matches. Useful to warn about entries that the game
    /// itself rejects.
    pub fn supported_matches_game_exact<S: AsRef<str>>(&self, supported: &[S]) -> bool {
        supported
            .iter()
            .any(|s| parse_major_minor(s.as_ref(), false) == Some(self.major_minor()))
    }

    /// True when some entry is newer than this version in major and minor ("made for a newer
    /// version") and none is compatible.
    pub fn made_for_newer<S: AsRef<str>>(&self, supported: &[S]) -> bool {
        !self.supported_matches(supported)
            && supported
                .iter()
                .filter_map(|s| parse_major_minor(s.as_ref(), true))
                .any(|mm| mm > self.major_minor())
    }

    /// The key tuple the game compares when it checks whether a `LoadFolders` version key is not
    /// newer than the running version. A key never carries a revision, so a missing revision is
    /// `-1` and orders before any real revision; an unknown build counts as `0`.
    pub(crate) fn game_order_key(&self) -> (u32, u32, u32, i64) {
        (
            self.major,
            self.minor,
            self.build.unwrap_or(0),
            self.rev.map_or(-1, i64::from),
        )
    }
}

/// Parses the major and minor numbers of a dotted text the way the game does for
/// `supportedVersions`: split on `.`, at least two parts, the first two non negative integers, extra
/// parts ignored. With `strip_v` one leading `v` or `V` is removed first.
pub fn parse_major_minor(text: &str, strip_v: bool) -> Option<(u32, u32)> {
    let mut t = text.trim();
    if strip_v {
        t = t.strip_prefix(['v', 'V']).unwrap_or(t);
    }
    let mut it = t.split('.');
    let major = parse_u32(it.next()?)?;
    let minor = parse_u32(it.next()?)?;
    Some((major, minor))
}

/// Normalises a version key as the game does for `ByVersion` children and `LoadFolders.xml` blocks:
/// lower case, then one leading `v` removed. `v1.6`, `V1.6` and `1.6` are the same key.
pub fn normalize_version_key(key: &str) -> String {
    let lower = key.to_lowercase();
    match lower.strip_prefix('v') {
        Some(rest) => rest.to_owned(),
        None => lower,
    }
}

/// Parses a `LoadFolders` version key into a comparable tuple the way `VersionFromString` does: one
/// to three dotted non negative numbers, a missing part is zero and a revision is always `-1`.
/// Returns `None` for anything else (the game then skips the key).
pub(crate) fn key_order_tuple(key: &str) -> Option<(u32, u32, u32, i64)> {
    if key.is_empty() {
        return None;
    }
    let mut nums = [0u32; 3];
    for (count, piece) in key.split('.').enumerate() {
        *nums.get_mut(count)? = parse_u32(piece)?;
    }
    Some((nums[0], nums[1], nums[2], -1))
}

impl PartialEq for GameVersion {
    fn eq(&self, other: &Self) -> bool {
        (self.major, self.minor, self.build, self.rev)
            == (other.major, other.minor, other.build, other.rev)
    }
}

impl Eq for GameVersion {}

impl Hash for GameVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.major, self.minor, self.build, self.rev).hash(state);
    }
}

impl PartialOrd for GameVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GameVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.build, self.rev).cmp(&(
            other.major,
            other.minor,
            other.build,
            other.rev,
        ))
    }
}

impl fmt::Display for GameVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

impl std::str::FromStr for GameVersion {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GameVersion::parse(s)
    }
}

/// On disk a version is its text; the numeric parts are derived again when reading.
impl Serialize for GameVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for GameVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        GameVersion::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn v(text: &str) -> GameVersion {
        GameVersion::parse(text).unwrap()
    }

    #[rstest]
    #[case("1.6.4871 rev598", 1, 6, Some(4871), Some(598))]
    #[case("1.6.4871", 1, 6, Some(4871), None)]
    #[case("1.6", 1, 6, None, None)]
    #[case("v1.6", 1, 6, None, None)]
    #[case("V1.5.4104 rev 435", 1, 5, Some(4104), Some(435))]
    #[case("  1.6.4871 rev598\r\n", 1, 6, Some(4871), Some(598))]
    #[case("\u{feff}1.4.3901 rev 1\n", 1, 4, Some(3901), Some(1))]
    #[case("1.6.4871 (rev598)", 1, 6, Some(4871), Some(598))]
    #[case("1.6.4871.598", 1, 6, Some(4871), Some(598))]
    #[case("0.19", 0, 19, None, None)]
    fn parse_accepts_tolerant_variants(
        #[case] text: &str,
        #[case] major: u32,
        #[case] minor: u32,
        #[case] build: Option<u32>,
        #[case] rev: Option<u32>,
    ) {
        let parsed = v(text);
        assert_eq!(
            (parsed.major, parsed.minor, parsed.build, parsed.rev),
            (major, minor, build, rev)
        );
    }

    #[rstest]
    #[case("", VersionFault::Empty)]
    #[case("   ", VersionFault::Empty)]
    #[case("1", VersionFault::TooFewParts)]
    #[case("one.six", VersionFault::BadNumber)]
    #[case("1.-6", VersionFault::BadNumber)]
    #[case("1..6", VersionFault::BadNumber)]
    #[case("1.2.3.4.5", VersionFault::TooManyParts)]
    #[case("1.6.4871 beta", VersionFault::UnexpectedSuffix)]
    #[case("1.6.4871.598 rev5", VersionFault::TooManyParts)]
    #[case("99999999999.1", VersionFault::BadNumber)]
    fn parse_rejects_bad_input_with_a_reason(#[case] text: &str, #[case] fault: VersionFault) {
        match GameVersion::parse(text) {
            Err(CoreError::InvalidVersion { fault: got, .. }) => assert_eq!(got, fault),
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn raw_text_is_kept_and_equality_ignores_it() {
        let a = v("1.6.4871 rev598");
        let b = v("v1.6.4871 rev 598");
        assert_eq!(a, b);
        assert_eq!(a.raw, "1.6.4871 rev598");
        assert_eq!(b.raw, "v1.6.4871 rev 598");
        assert_eq!(a.canonical(), "1.6.4871 rev598");
    }

    #[test]
    fn ordering_is_numeric_not_textual() {
        let mut list = [
            v("1.10"),
            v("1.9"),
            v("1.6.4871 rev598"),
            v("1.6"),
            v("1.6.4871"),
            v("0.19"),
        ];
        list.sort();
        let order: Vec<String> = list.iter().map(GameVersion::canonical).collect();
        assert_eq!(
            order,
            ["0.19", "1.6", "1.6.4871", "1.6.4871 rev598", "1.9", "1.10"]
        );
    }

    #[rstest]
    #[case("1.6", &["1.6"], true)]
    #[case("1.6.4871 rev598", &["1.5", "1.6"], true)]
    #[case("1.6.4871", &["1.6.9999"], true)]
    #[case("1.6", &["v1.6"], true)]
    #[case("1.6", &["V1.6"], true)]
    #[case("1.6", &["1.5", "1.4"], false)]
    #[case("1.6", &["1.60"], false)]
    #[case("1.6", &["1.7", "1.8"], false)]
    #[case("1.6", &["1"], false)]
    #[case("1.6", &["v", "x.y", ""], false)]
    #[case("1.6", &[] as &[&str], false)]
    #[case("1.6", &[" 1.6 "], true)]
    fn supported_matches_uses_major_minor_equality(
        #[case] game: &str,
        #[case] listed: &[&str],
        #[case] expected: bool,
    ) {
        assert_eq!(v(game).supported_matches(listed), expected);
    }

    #[test]
    fn game_exact_matching_does_not_strip_v() {
        let g = v("1.6.4871 rev598");
        assert!(g.supported_matches(&["v1.6"]));
        assert!(!g.supported_matches_game_exact(&["v1.6"]));
        assert!(g.supported_matches_game_exact(&["1.6.4871"]));
    }

    #[test]
    fn made_for_newer_requires_a_newer_entry_and_no_match() {
        let g = v("1.6");
        assert!(g.made_for_newer(&["1.7"]));
        assert!(!g.made_for_newer(&["1.6", "1.7"]));
        assert!(!g.made_for_newer(&["1.5"]));
    }

    #[rstest]
    #[case("v1.6", "1.6")]
    #[case("V1.6", "1.6")]
    #[case("1.6", "1.6")]
    #[case("Default", "default")]
    #[case("vv1.6", "v1.6")]
    #[case("", "")]
    fn version_keys_are_lowercased_with_one_v_removed(#[case] key: &str, #[case] expected: &str) {
        assert_eq!(normalize_version_key(key), expected);
    }

    #[test]
    fn key_tuples_follow_the_game_parse() {
        assert_eq!(key_order_tuple("1.6"), Some((1, 6, 0, -1)));
        assert_eq!(key_order_tuple("1.6.4871"), Some((1, 6, 4871, -1)));
        assert_eq!(key_order_tuple("1.6.4871.1"), None);
        assert_eq!(key_order_tuple("default"), None);
        assert_eq!(key_order_tuple("1"), Some((1, 0, 0, -1)));
        // A key without revision is not newer than the same build with a revision.
        assert!(key_order_tuple("1.6.4871").unwrap() <= v("1.6.4871 rev598").game_order_key());
    }

    #[test]
    fn serde_round_trips_the_text() {
        let g = v("1.6.4871 rev598");
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(json, "\"1.6.4871 rev598\"");
        let back: GameVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
        assert!(serde_json::from_str::<GameVersion>("\"nope\"").is_err());
    }

    #[test]
    fn constructor_builds_canonical_text() {
        let g = GameVersion::new(1, 6, Some(4871), Some(598));
        assert_eq!(g.raw, "1.6.4871 rev598");
        assert_eq!(g.short(), "1.6");
        assert_eq!(g.with_build(), "1.6.4871");
        assert_eq!(GameVersion::new(1, 6, None, None).with_build(), "1.6.0");
    }

    proptest! {
        #[test]
        fn canonical_text_parses_back_to_the_same_version(
            major in 0u32..1000, minor in 0u32..1000,
            build in proptest::option::of(0u32..100_000),
            rev in proptest::option::of(0u32..100_000),
        ) {
            // A revision without a build cannot be written in canonical form that parses back.
            prop_assume!(build.is_some() || rev.is_none());
            let g = GameVersion::new(major, minor, build, rev);
            let back = GameVersion::parse(&g.canonical()).unwrap();
            prop_assert_eq!(back, g);
        }

        #[test]
        fn parse_never_panics_and_ok_results_reparse(text in "\\PC{0,40}") {
            if let Ok(g) = GameVersion::parse(&text) {
                let again = GameVersion::parse(&g.canonical()).unwrap();
                prop_assert_eq!(again.major_minor(), g.major_minor());
            }
        }

        #[test]
        fn ordering_agrees_with_tuple_ordering(
            a in (0u32..50, 0u32..50, proptest::option::of(0u32..9000)),
            b in (0u32..50, 0u32..50, proptest::option::of(0u32..9000)),
        ) {
            let va = GameVersion::new(a.0, a.1, a.2, None);
            let vb = GameVersion::new(b.0, b.1, b.2, None);
            prop_assert_eq!(va.cmp(&vb), (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
        }

        #[test]
        fn supported_matches_ignores_build_and_a_leading_v(
            major in 0u32..20, minor in 0u32..20, build in 0u32..9000, prefix in "[vV]?",
        ) {
            let game = GameVersion::new(major, minor, Some(5000), Some(1));
            let entry = format!("{prefix}{major}.{minor}.{build}");
            prop_assert!(game.supports_entry(&entry));
            let other = format!("{prefix}{major}.{}", minor + 1);
            prop_assert!(!game.supports_entry(&other));
        }
    }
}
