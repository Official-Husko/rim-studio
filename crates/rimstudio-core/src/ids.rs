//! Identifiers.
//!
//! Persistent identity (what survives moves and renames, see data-and-persistence section 5):
//! [`PackageId`], [`WorkshopId`], [`ModId`], [`SourceId`], [`ProfileId`], [`ProjectId`].
//! Session handles that are never written to disk: [`ModIdx`], [`FileId`]. Caller minted: [`JobId`].
//!
//! 64 bit ids are strings everywhere on disk and on the wire ([`WorkshopId`] serialises as a string
//! and also accepts a JSON number when reading).

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CoreError, IdFault};

const PACKAGE_ID_MAX_CHARS: usize = 256;
const GAME_PACKAGE_ID_MAX: usize = 60;
const STEAM_POSTFIX: &str = "_steam";

// ---------------------------------------------------------------------------------------------
// PackageId
// ---------------------------------------------------------------------------------------------

/// A mod's package id (`author.modname`), compared and hashed ignoring case.
///
/// The original spelling is kept for display and for writing back into `ModsConfig.xml`, which must
/// keep it. Parsing is deliberately tolerant: the game only warns about malformed ids and keeps them
/// as written, so any trimmed non empty text without control characters is accepted. Use
/// [`PackageId::check_game_format`] to learn whether the game would warn.
#[derive(Debug, Clone)]
pub struct PackageId {
    original: Box<str>,
    lower: Box<str>,
}

/// A reason the game's package id format check fails (the game only logs a warning).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageIdProblem {
    /// Longer than 60 characters.
    TooLong,
    /// A character other than ASCII letters, digits and dots.
    BadChar,
    /// Starts with a dot.
    LeadingDot,
    /// No dot at all.
    NoDot,
    /// Two dots in a row.
    EmptySegment,
    /// The last character is not a letter or digit.
    BadEnd,
}

impl PackageId {
    /// Parses and validates a package id, trimming surrounding whitespace.
    ///
    /// # Errors
    /// [`CoreError::InvalidId`] when the text is empty, longer than 256 characters or contains a
    /// control character.
    pub fn parse(input: &str) -> Result<PackageId, CoreError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(CoreError::invalid_id("package", input, IdFault::Empty));
        }
        if text.chars().count() > PACKAGE_ID_MAX_CHARS {
            return Err(CoreError::invalid_id(
                "package",
                input,
                IdFault::TooLong {
                    max: PACKAGE_ID_MAX_CHARS,
                },
            ));
        }
        if let Some((index, ch)) = text.chars().enumerate().find(|(_, c)| c.is_control()) {
            return Err(CoreError::invalid_id(
                "package",
                input,
                IdFault::BadChar { ch, index },
            ));
        }
        Ok(PackageId {
            original: text.into(),
            lower: text.to_lowercase().into(),
        })
    }

    /// The spelling as written by the mod author, for display and `ModsConfig.xml`.
    pub fn as_str(&self) -> &str {
        &self.original
    }

    /// The lower case form used for every comparison, hash and index key.
    pub fn lower(&self) -> &str {
        &self.lower
    }

    /// True when both ids are the same ignoring case and the game's `_steam` postfix.
    pub fn same_ignoring_postfix(&self, other: &PackageId) -> bool {
        self.lower_without_postfix() == other.lower_without_postfix()
    }

    /// The lower case id with one trailing `_steam` removed (the game appends it to the Workshop
    /// copy of a duplicated id).
    pub fn lower_without_postfix(&self) -> &str {
        self.lower
            .strip_suffix(STEAM_POSTFIX)
            .unwrap_or(&self.lower)
    }

    /// True when the id ends with the game's `_steam` postfix.
    pub fn has_steam_postfix(&self) -> bool {
        self.lower.ends_with(STEAM_POSTFIX)
    }

    /// True for the base game and the official expansions.
    pub fn is_official(&self) -> bool {
        crate::paths::is_official_package_id(&self.lower)
    }

    /// True when the id contains the word `ludeon` (the game warns about that for non official ids).
    pub fn contains_ludeon(&self) -> bool {
        self.lower.contains("ludeon")
    }

    /// The game's package id format check: at most 60 characters, ASCII letters, digits and dots,
    /// no leading dot, at least one dot, no two dots in a row and an alphanumeric last character.
    ///
    /// # Errors
    /// The first problem found, in the order of the list above.
    pub fn check_game_format(&self) -> Result<(), PackageIdProblem> {
        let s: &str = &self.original;
        if s.chars().count() > GAME_PACKAGE_ID_MAX {
            return Err(PackageIdProblem::TooLong);
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
            return Err(PackageIdProblem::BadChar);
        }
        if s.starts_with('.') {
            return Err(PackageIdProblem::LeadingDot);
        }
        if !s.contains('.') {
            return Err(PackageIdProblem::NoDot);
        }
        if s.contains("..") {
            return Err(PackageIdProblem::EmptySegment);
        }
        if !s.chars().last().is_some_and(|c| c.is_ascii_alphanumeric()) {
            return Err(PackageIdProblem::BadEnd);
        }
        Ok(())
    }
}

impl PartialEq for PackageId {
    fn eq(&self, other: &Self) -> bool {
        self.lower == other.lower
    }
}

impl Eq for PackageId {}

impl Hash for PackageId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lower.hash(state);
    }
}

impl PartialOrd for PackageId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Orders by the lower case form, then by the original spelling so the order is total and stable.
impl Ord for PackageId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.lower
            .cmp(&other.lower)
            .then_with(|| self.original.cmp(&other.original))
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl std::str::FromStr for PackageId {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PackageId::parse(s)
    }
}

impl Serialize for PackageId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.original)
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        PackageId::parse(&text).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------------------------
// WorkshopId
// ---------------------------------------------------------------------------------------------

/// A Steam Workshop published file id: a non zero unsigned 64 bit number.
///
/// Serialised as a decimal string (JSON numbers lose precision above 2^53 in JavaScript). Reading
/// also accepts a JSON integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkshopId(u64);

impl WorkshopId {
    /// Wraps a number; `None` for zero, which the game uses for "no id".
    pub fn new(value: u64) -> Option<WorkshopId> {
        if value == 0 {
            None
        } else {
            Some(WorkshopId(value))
        }
    }

    /// Parses decimal text, tolerating surrounding whitespace (as the game's own parse does).
    ///
    /// # Errors
    /// [`CoreError::InvalidId`] for empty text, non digits, zero or a number above `u64::MAX`.
    pub fn parse(input: &str) -> Result<WorkshopId, CoreError> {
        let t = input.trim().trim_start_matches('\u{feff}').trim();
        if t.is_empty() {
            return Err(CoreError::invalid_id("workshop", input, IdFault::Empty));
        }
        if !t.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CoreError::invalid_id(
                "workshop",
                input,
                IdFault::NotANumber,
            ));
        }
        let n: u64 = t
            .parse()
            .map_err(|_| CoreError::invalid_id("workshop", input, IdFault::OutOfRange))?;
        WorkshopId::new(n)
            .ok_or_else(|| CoreError::invalid_id("workshop", input, IdFault::OutOfRange))
    }

    /// The numeric value.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WorkshopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WorkshopId {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkshopId::parse(s)
    }
}

impl Serialize for WorkshopId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

struct WorkshopIdVisitor;

impl Visitor<'_> for WorkshopIdVisitor {
    type Value = WorkshopId;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a Workshop id as a decimal string or integer")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        WorkshopId::parse(v).map_err(E::custom)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        WorkshopId::new(v).ok_or_else(|| E::custom("a Workshop id cannot be zero"))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        u64::try_from(v)
            .ok()
            .and_then(WorkshopId::new)
            .ok_or_else(|| E::custom("a Workshop id must be positive"))
    }
}

impl<'de> Deserialize<'de> for WorkshopId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WorkshopIdVisitor)
    }
}

// ---------------------------------------------------------------------------------------------
// Session handles
// ---------------------------------------------------------------------------------------------

macro_rules! handle {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            /// Wraps a raw handle value.
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            /// Builds a handle from a vector position; `None` when it does not fit in 32 bits.
            pub fn from_index(index: usize) -> Option<Self> {
                u32::try_from(index).ok().map(Self)
            }

            /// The handle as a vector position.
            pub fn index(self) -> usize {
                self.0 as usize
            }

            /// The raw value.
            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

handle! {
    /// A session local handle of a mod inside one `ModIndex` (its position in load independent index
    /// order). Valid for one index only and never persisted (data-and-persistence section 5.2).
    ModIdx
}

handle! {
    /// A session local handle of an interned file path. Valid for one file table only and never
    /// persisted.
    FileId
}

// ---------------------------------------------------------------------------------------------
// String ids with a restricted alphabet
// ---------------------------------------------------------------------------------------------

fn validate_token(kind: &'static str, input: &str, max: usize) -> Result<(), CoreError> {
    if input.is_empty() {
        return Err(CoreError::invalid_id(kind, input, IdFault::Empty));
    }
    if input.chars().count() > max {
        return Err(CoreError::invalid_id(kind, input, IdFault::TooLong { max }));
    }
    if let Some((index, ch)) = input
        .chars()
        .enumerate()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
    {
        return Err(CoreError::invalid_id(
            kind,
            input,
            IdFault::BadChar { ch, index },
        ));
    }
    Ok(())
}

macro_rules! token_id {
    ($(#[$doc:meta])* $name:ident, $kind:literal, $max:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// The longest accepted text in characters.
            pub const MAX_LEN: usize = $max;

            /// Validates `input` (ASCII letters, digits, `_` and `-`, 1 to the maximum length).
            ///
            /// # Errors
            /// [`CoreError::InvalidId`] naming the first problem.
            pub fn new(input: &str) -> Result<Self, CoreError> {
                validate_token($kind, input, $max)?;
                Ok(Self(input.to_owned()))
            }

            /// The id text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = CoreError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Self::new(&text).map_err(de::Error::custom)
            }
        }
    };
}

token_id! {
    /// The id of a mod source: `game-data`, `game-mods`, `workshop-<n>` or `cf_<hex>` for a custom
    /// folder. It is the `<sourceId>` part of a [`ModId`], so a generated custom folder id keeps mod
    /// identity stable when the folder moves.
    SourceId, "source", 64
}

token_id! {
    /// The id of a mod list profile, for example `pr_default`.
    ProfileId, "profile", 64
}

token_id! {
    /// The id of a modding project, for example `p-0001`.
    ProjectId, "project", 64
}

token_id! {
    /// The id of a job, minted by the caller (a UUID in the webview, a ULID like string in the CLI).
    JobId, "job", 64
}

/// The number of hexadecimal characters in a generated id suffix.
const GENERATED_HEX_CHARS: usize = 8;

fn hex_suffix(seed: &[u8]) -> String {
    let hash = blake3::hash(seed);
    let hex = hash.to_hex();
    hex.as_str()[..GENERATED_HEX_CHARS].to_owned()
}

impl SourceId {
    /// The built in source of the install `Data` folder.
    pub fn game_data() -> SourceId {
        SourceId("game-data".to_owned())
    }

    /// The built in source of the install `Mods` folder.
    pub fn game_mods() -> SourceId {
        SourceId("game-mods".to_owned())
    }

    /// The Workshop source of the n-th Steam library that holds content (`workshop-0`, ...).
    pub fn workshop(n: usize) -> SourceId {
        SourceId(format!("workshop-{n}"))
    }

    /// Derives a custom folder id `cf_<8 hex characters>` from caller supplied entropy (the path and
    /// a time or counter). The hash is blake3; the same seed always gives the same id, and core
    /// itself has no clock or random source.
    pub fn custom_from_seed(seed: &[u8]) -> SourceId {
        SourceId(format!("cf_{}", hex_suffix(seed)))
    }

    /// True for ids generated for custom folders (`cf_` prefix).
    pub fn is_custom(&self) -> bool {
        self.0.starts_with("cf_")
    }

    /// True for the Workshop sources (`workshop-` prefix).
    pub fn is_workshop(&self) -> bool {
        self.0.starts_with("workshop-")
    }
}

impl ProfileId {
    /// The id of the default profile.
    pub fn default_profile() -> ProfileId {
        ProfileId("pr_default".to_owned())
    }

    /// Derives `pr_<8 hex characters>` from caller supplied entropy.
    pub fn from_seed(seed: &[u8]) -> ProfileId {
        ProfileId(format!("pr_{}", hex_suffix(seed)))
    }
}

impl ProjectId {
    /// Derives `p-<8 hex characters>` from caller supplied entropy.
    pub fn from_seed(seed: &[u8]) -> ProjectId {
        ProjectId(format!("p-{}", hex_suffix(seed)))
    }
}

// ---------------------------------------------------------------------------------------------
// ModId
// ---------------------------------------------------------------------------------------------

/// The persistent identity of a mod: `w<workshopId>` for Workshop items, otherwise
/// `<sourceId>:<packageId>` with the package id in lower case.
///
/// Two copies of one package id inside a single source share a `ModId` (a normal state for a modder,
/// see game-and-mod-discovery GD-051); the index resolves such groups with the duplicate policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModId(String);

/// The parsed shape of a [`ModId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModIdKind {
    /// A Workshop item.
    Workshop(WorkshopId),
    /// A mod identified by its source and lower case package id.
    Local {
        /// The source the mod lives in.
        source: SourceId,
        /// The lower case package id.
        package_id: String,
    },
}

impl ModId {
    /// The id of a Workshop item.
    pub fn workshop(id: WorkshopId) -> ModId {
        ModId(format!("w{}", id.get()))
    }

    /// The id of a mod in `source` with the given package id (lower cased).
    pub fn local(source: &SourceId, package_id: &PackageId) -> ModId {
        ModId(format!("{}:{}", source.as_str(), package_id.lower()))
    }

    /// Parses `w<digits>` or `<sourceId>:<packageId>` (the package part is lower cased).
    ///
    /// # Errors
    /// [`CoreError::InvalidId`] when neither shape matches or a part is invalid.
    pub fn parse(input: &str) -> Result<ModId, CoreError> {
        let t = input.trim();
        if t.is_empty() {
            return Err(CoreError::invalid_id("mod", input, IdFault::Empty));
        }
        if let Some((source, package)) = t.split_once(':') {
            let source = SourceId::new(source)
                .map_err(|_| CoreError::invalid_id("mod", input, IdFault::BadShape))?;
            let package = PackageId::parse(package)
                .map_err(|_| CoreError::invalid_id("mod", input, IdFault::BadShape))?;
            return Ok(ModId::local(&source, &package));
        }
        match t.strip_prefix('w') {
            Some(digits) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                let id = WorkshopId::parse(digits)
                    .map_err(|_| CoreError::invalid_id("mod", input, IdFault::OutOfRange))?;
                Ok(ModId::workshop(id))
            }
            _ => Err(CoreError::invalid_id("mod", input, IdFault::BadShape)),
        }
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The parsed shape of this id.
    pub fn kind(&self) -> ModIdKind {
        if let Some((source, package)) = self.0.split_once(':') {
            return ModIdKind::Local {
                source: SourceId(source.to_owned()),
                package_id: package.to_owned(),
            };
        }
        let digits = self.0.strip_prefix('w').unwrap_or("");
        match WorkshopId::parse(digits) {
            Ok(id) => ModIdKind::Workshop(id),
            // Unreachable for ids built by this module; keep a harmless fallback instead of a panic.
            Err(_) => ModIdKind::Local {
                source: SourceId(String::new()),
                package_id: self.0.clone(),
            },
        }
    }
}

impl fmt::Display for ModId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for ModId {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ModId::parse(s)
    }
}

impl Serialize for ModId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ModId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        ModId::parse(&text).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;
    use std::collections::HashSet;

    fn pid(s: &str) -> PackageId {
        PackageId::parse(s).unwrap()
    }

    #[test]
    fn package_ids_compare_and_hash_ignoring_case_but_keep_the_original() {
        let a = pid("RS.Fictional.Mod");
        let b = pid("rs.fictional.mod");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "RS.Fictional.Mod");
        assert_eq!(a.lower(), "rs.fictional.mod");
        let set: HashSet<PackageId> = [a.clone(), b.clone()].into_iter().collect();
        assert_eq!(set.len(), 1);
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn package_id_ordering_is_total_and_case_insensitive_first() {
        let mut v = [pid("b.x"), pid("A.x"), pid("a.x"), pid("B.y")];
        v.sort();
        let names: Vec<&str> = v.iter().map(PackageId::as_str).collect();
        assert_eq!(names, ["A.x", "a.x", "b.x", "B.y"]);
    }

    #[rstest]
    #[case("", IdFault::Empty)]
    #[case("   ", IdFault::Empty)]
    fn package_parse_rejects_empty(#[case] text: &str, #[case] fault: IdFault) {
        assert!(
            matches!(PackageId::parse(text), Err(CoreError::InvalidId { fault: f, .. }) if f == fault)
        );
    }

    #[test]
    fn package_parse_rejects_control_characters_and_overlong_text() {
        assert!(matches!(
            PackageId::parse("a.b\u{0}c"),
            Err(CoreError::InvalidId {
                fault: IdFault::BadChar { index: 3, .. },
                ..
            })
        ));
        assert!(matches!(
            PackageId::parse(&"a".repeat(300)),
            Err(CoreError::InvalidId {
                fault: IdFault::TooLong { max: 256 },
                ..
            })
        ));
    }

    #[test]
    fn package_parse_trims_and_is_tolerant_about_game_format() {
        let id = pid("  rs.fake_mod-x  ");
        assert_eq!(id.as_str(), "rs.fake_mod-x");
        assert_eq!(id.check_game_format(), Err(PackageIdProblem::BadChar));
    }

    #[rstest]
    #[case("rs.fictional.mod", Ok(()))]
    #[case("a.b", Ok(()))]
    #[case("nodots", Err(PackageIdProblem::NoDot))]
    #[case(".leading.dot", Err(PackageIdProblem::LeadingDot))]
    #[case("two..dots", Err(PackageIdProblem::EmptySegment))]
    #[case("trailing.", Err(PackageIdProblem::BadEnd))]
    #[case("under_score.mod", Err(PackageIdProblem::BadChar))]
    fn game_format_check_matches_the_documented_rules(
        #[case] text: &str,
        #[case] expected: Result<(), PackageIdProblem>,
    ) {
        assert_eq!(pid(text).check_game_format(), expected);
    }

    #[test]
    fn game_format_check_limits_length_to_sixty() {
        let ok = format!("a.{}", "b".repeat(58));
        let long = format!("a.{}", "b".repeat(59));
        assert_eq!(pid(&ok).check_game_format(), Ok(()));
        assert_eq!(
            pid(&long).check_game_format(),
            Err(PackageIdProblem::TooLong)
        );
    }

    #[test]
    fn steam_postfix_handling() {
        let a = pid("RS.Mod_steam");
        assert!(a.has_steam_postfix());
        assert_eq!(a.lower_without_postfix(), "rs.mod");
        assert!(a.same_ignoring_postfix(&pid("rs.MOD")));
        assert!(!pid("rs.mod").has_steam_postfix());
    }

    #[test]
    fn official_and_ludeon_checks() {
        assert!(pid("Ludeon.RimWorld.Royalty").is_official());
        assert!(!pid("rs.mod").is_official());
        assert!(pid("rs.ludeon.fake").contains_ludeon());
    }

    #[test]
    fn package_id_serde_keeps_the_original_text() {
        let id = pid("RS.Fictional.Mod");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"RS.Fictional.Mod\"");
        let back: PackageId = serde_json::from_str(&json).unwrap();
        assert_eq!(back.as_str(), "RS.Fictional.Mod");
        assert!(serde_json::from_str::<PackageId>("\"\"").is_err());
    }

    #[rstest]
    #[case("123456", Some(123_456))]
    #[case(" 42\n", Some(42))]
    #[case("18446744073709551615", Some(u64::MAX))]
    #[case("0", None)]
    #[case("", None)]
    #[case("12a", None)]
    #[case("-5", None)]
    #[case("18446744073709551616", None)]
    fn workshop_id_parse(#[case] text: &str, #[case] expected: Option<u64>) {
        assert_eq!(WorkshopId::parse(text).ok().map(WorkshopId::get), expected);
    }

    #[test]
    fn workshop_id_serialises_as_a_string_and_reads_both_forms() {
        let id = WorkshopId::new(9_007_199_254_740_993).unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"9007199254740993\"");
        assert_eq!(
            serde_json::from_str::<WorkshopId>("\"9007199254740993\"").unwrap(),
            id
        );
        assert_eq!(serde_json::from_str::<WorkshopId>("77").unwrap().get(), 77);
        assert!(serde_json::from_str::<WorkshopId>("0").is_err());
        assert!(serde_json::from_str::<WorkshopId>("-3").is_err());
        assert!(serde_json::from_str::<WorkshopId>("\"x\"").is_err());
    }

    #[test]
    fn handles_convert_to_and_from_positions() {
        assert_eq!(ModIdx::from_index(7), Some(ModIdx(7)));
        assert_eq!(ModIdx(7).index(), 7);
        assert_eq!(FileId::new(3).get(), 3);
        assert_eq!(ModIdx::from_index(usize::MAX), None);
        assert_eq!(serde_json::to_string(&ModIdx(5)).unwrap(), "5");
    }

    #[rstest]
    #[case("cf_7f3a", true)]
    #[case("game-data", true)]
    #[case("workshop-0", true)]
    #[case("", false)]
    #[case("has space", false)]
    #[case("colon:bad", false)]
    #[case("slash/bad", false)]
    fn source_id_alphabet(#[case] text: &str, #[case] ok: bool) {
        assert_eq!(SourceId::new(text).is_ok(), ok);
    }

    #[test]
    fn token_ids_enforce_a_length_limit() {
        assert!(SourceId::new(&"a".repeat(64)).is_ok());
        assert!(matches!(
            SourceId::new(&"a".repeat(65)),
            Err(CoreError::InvalidId {
                fault: IdFault::TooLong { max: 64 },
                ..
            })
        ));
        assert!(JobId::new("0b8d7d34-6bd5-4a8c-9e57-0e3c5c0f3a11").is_ok());
        assert!(ProjectId::new("p-0001").is_ok());
        assert_eq!(ProfileId::default_profile().as_str(), "pr_default");
    }

    #[test]
    fn generated_ids_are_deterministic_and_well_formed() {
        let a = SourceId::custom_from_seed(b"/mods/rs_folder|1");
        let b = SourceId::custom_from_seed(b"/mods/rs_folder|1");
        let c = SourceId::custom_from_seed(b"/mods/rs_folder|2");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.is_custom());
        assert_eq!(a.as_str().len(), 3 + 8);
        assert!(SourceId::new(a.as_str()).is_ok());
        assert!(ProfileId::from_seed(b"x").as_str().starts_with("pr_"));
        assert!(ProjectId::from_seed(b"x").as_str().starts_with("p-"));
        assert!(SourceId::workshop(2).is_workshop());
        assert!(!SourceId::game_mods().is_custom());
        assert_eq!(SourceId::game_data().as_str(), "game-data");
    }

    #[test]
    fn token_ids_validate_on_deserialize() {
        assert!(serde_json::from_str::<SourceId>("\"cf_ab12\"").is_ok());
        assert!(serde_json::from_str::<SourceId>("\"bad id\"").is_err());
        assert_eq!(
            serde_json::to_string(&JobId::new("j1").unwrap()).unwrap(),
            "\"j1\""
        );
    }

    #[test]
    fn mod_ids_build_and_parse_both_shapes() {
        let w = ModId::workshop(WorkshopId::new(2_949_339_248).unwrap());
        assert_eq!(w.as_str(), "w2949339248");
        assert_eq!(ModId::parse("w2949339248").unwrap(), w);
        assert!(matches!(w.kind(), ModIdKind::Workshop(id) if id.get() == 2_949_339_248));

        let l = ModId::local(&SourceId::new("cf_7f3a").unwrap(), &pid("RS.Weapons.Alpha"));
        assert_eq!(l.as_str(), "cf_7f3a:rs.weapons.alpha");
        assert_eq!(ModId::parse("cf_7f3a:RS.Weapons.Alpha").unwrap(), l);
        match l.kind() {
            ModIdKind::Local { source, package_id } => {
                assert_eq!(source.as_str(), "cf_7f3a");
                assert_eq!(package_id, "rs.weapons.alpha");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[rstest]
    #[case("")]
    #[case("w")]
    #[case("w0")]
    #[case("w12x")]
    #[case("x123")]
    #[case(":rs.mod")]
    #[case("src:")]
    #[case("bad source:rs.mod")]
    fn mod_id_rejects_bad_shapes(#[case] text: &str) {
        assert!(ModId::parse(text).is_err(), "{text}");
    }

    #[test]
    fn mod_id_serde_round_trip() {
        let l = ModId::parse("game-mods:rs.mod.beta").unwrap();
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<ModId>(&json).unwrap(), l);
        assert!(serde_json::from_str::<ModId>("\"nonsense\"").is_err());
    }

    proptest! {
        #[test]
        fn package_id_equality_is_case_insensitive(text in "[A-Za-z][A-Za-z0-9.]{0,40}") {
            let a = PackageId::parse(&text).unwrap();
            let b = PackageId::parse(&text.to_uppercase()).unwrap();
            let c = PackageId::parse(&text.to_lowercase()).unwrap();
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(&a, &c);
            prop_assert_eq!(a.lower(), c.as_str());
            let mut set = HashSet::new();
            set.insert(a);
            prop_assert!(set.contains(&b));
        }

        #[test]
        fn package_parse_never_panics(text in "\\PC{0,300}") {
            let _ = PackageId::parse(&text);
        }

        #[test]
        fn package_id_trim_is_idempotent(text in "[ \t]{0,3}[a-z][a-z.]{0,20}[ \t]{0,3}") {
            let a = PackageId::parse(&text).unwrap();
            let b = PackageId::parse(a.as_str()).unwrap();
            prop_assert_eq!(a.as_str(), b.as_str());
        }

        #[test]
        fn workshop_id_round_trips_through_text_and_json(n in 1u64..) {
            let id = WorkshopId::new(n).unwrap();
            prop_assert_eq!(WorkshopId::parse(&id.to_string()).unwrap(), id);
            let json = serde_json::to_string(&id).unwrap();
            prop_assert_eq!(serde_json::from_str::<WorkshopId>(&json).unwrap(), id);
        }

        #[test]
        fn mod_id_round_trips(source in "[a-z][a-z0-9_-]{0,20}", pkg in "[A-Za-z][A-Za-z0-9.]{0,30}") {
            let s = SourceId::new(&source).unwrap();
            let p = PackageId::parse(&pkg).unwrap();
            let id = ModId::local(&s, &p);
            prop_assert_eq!(ModId::parse(id.as_str()).unwrap(), id.clone());
            prop_assert_eq!(ModId::parse(&id.as_str().to_uppercase().replace(&source.to_uppercase(), &source)).unwrap(), id);
        }

        #[test]
        fn mod_id_parse_never_panics(text in "\\PC{0,100}") {
            let _ = ModId::parse(&text);
        }
    }
}
