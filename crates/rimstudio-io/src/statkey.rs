//! Stat keys: cheap change detection for cached scan results.
//!
//! A [`StatKey`] is `(relative path, size, mtime in ns, file id)`. Two keys are compared for
//! equality only, never "newer than", because timestamp granularity differs per file system and
//! clocks go backwards. On FAT family volumes the modification time has two second resolution and
//! shifts by whole hours with daylight saving time, so [`FatPolicy::Fat`] ignores the file id,
//! accepts timestamps that agree after rounding to two seconds and, when the size matches but the
//! timestamp does not, asks for a content hash ([`KeyVerdict::NeedsHash`]) instead of re-parsing.
//! The hash is computed lazily with blake3 and stored by the caller next to the key.
//!
//! The file id (inode and device) cannot be read portably from `std`, and operating system
//! conditionals belong to `rimstudio-platform`, so the id comes from an optional hook
//! ([`FileIdFn`]) that the platform layer supplies; without it the id is `None` and the key
//! degrades gracefully to `(path, size, mtime)`.

use std::io::Read;
use std::time::SystemTime;

use camino::Utf8Path;
use rimstudio_core::ports::VolumeClass;
use serde::{Deserialize, Serialize};

/// Reads a volume and file identity from metadata (inode on Unix, file index on Windows).
pub type FileIdFn = fn(&std::fs::Metadata) -> Option<u128>;

/// The change detection key of one file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatKey {
    /// The path relative to the scanned root, with `/` separators.
    pub rel_path: String,
    /// The size in bytes.
    pub size: u64,
    /// The modification time in nanoseconds since the Unix epoch, `None` when unavailable.
    pub mtime_ns: Option<i128>,
    /// The file identity, `None` when unavailable.
    pub file_id: Option<u128>,
}

/// Nanoseconds since the Unix epoch of a [`SystemTime`] (negative before 1970).
pub fn system_time_ns(t: SystemTime) -> i128 {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i128::try_from(d.as_nanos()).unwrap_or(i128::MAX),
        Err(e) => -i128::try_from(e.duration().as_nanos()).unwrap_or(i128::MAX),
    }
}

impl StatKey {
    /// Builds a key from metadata that was already read.
    pub fn from_metadata(
        rel_path: impl Into<String>,
        meta: &std::fs::Metadata,
        file_id: Option<FileIdFn>,
    ) -> StatKey {
        StatKey {
            rel_path: rel_path.into(),
            size: meta.len(),
            mtime_ns: meta.modified().ok().map(system_time_ns),
            file_id: file_id.and_then(|f| f(meta)),
        }
    }

    /// Stats `root/rel_path` (links not followed) and builds the key.
    pub fn stat(
        root: &Utf8Path,
        rel_path: &str,
        file_id: Option<FileIdFn>,
    ) -> std::io::Result<StatKey> {
        let meta = fs_err::symlink_metadata(root.join(rel_path))?;
        Ok(StatKey::from_metadata(rel_path, &meta, file_id))
    }
}

/// What comparing two keys concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyVerdict {
    /// The file is unchanged.
    Same,
    /// The file changed.
    Changed,
    /// Size matches but the timestamp does not and the volume is imprecise: compare content hashes.
    NeedsHash,
}

/// How timestamps are compared on a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FatPolicy {
    /// Exact nanosecond comparison plus the file id.
    #[default]
    Exact,
    /// Two second rounding, no file id, content hash fallback.
    Fat,
}

const TWO_SECONDS_NS: i128 = 2_000_000_000;

fn round_two_seconds(ns: i128) -> i128 {
    ns.div_euclid(TWO_SECONDS_NS)
}

impl FatPolicy {
    /// The policy for a probed volume: FAT and exFAT get [`FatPolicy::Fat`].
    pub fn for_volume(class: VolumeClass) -> FatPolicy {
        match class {
            VolumeClass::Fat | VolumeClass::Exfat => FatPolicy::Fat,
            _ => FatPolicy::Exact,
        }
    }

    /// Compares an old (cached) key with a new one.
    pub fn compare(self, old: &StatKey, new: &StatKey) -> KeyVerdict {
        if old.rel_path != new.rel_path || old.size != new.size {
            return KeyVerdict::Changed;
        }
        match self {
            FatPolicy::Exact => {
                if old.mtime_ns == new.mtime_ns && old.file_id == new.file_id {
                    KeyVerdict::Same
                } else {
                    KeyVerdict::Changed
                }
            }
            FatPolicy::Fat => match (old.mtime_ns, new.mtime_ns) {
                (Some(a), Some(b)) if a == b || round_two_seconds(a) == round_two_seconds(b) => {
                    KeyVerdict::Same
                }
                _ => KeyVerdict::NeedsHash,
            },
        }
    }

    /// Full decision including the lazy hash step. Returns `(unchanged, fresh_hash)`:
    /// `fresh_hash` is set when a hash had to be computed so the caller can store it. With no stored
    /// hash a `NeedsHash` verdict counts as changed (the caller re-reads the file and may then store
    /// the hash it computes).
    pub fn is_unchanged(
        self,
        old: &StatKey,
        old_hash: Option<&str>,
        new: &StatKey,
        abs_path: &Utf8Path,
    ) -> std::io::Result<(bool, Option<String>)> {
        match self.compare(old, new) {
            KeyVerdict::Same => Ok((true, None)),
            KeyVerdict::Changed => Ok((false, None)),
            KeyVerdict::NeedsHash => {
                let fresh = hash_file(abs_path)?;
                let same = old_hash.is_some_and(|h| h == fresh);
                Ok((same, Some(fresh)))
            }
        }
    }
}

/// The blake3 hash of a file as lowercase hex, read in chunks.
pub fn hash_file(path: &Utf8Path) -> std::io::Result<String> {
    let mut f = fs_err::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if let Some(chunk) = buf.get(..n) {
            hasher.update(chunk);
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use rstest::rstest;

    use super::*;

    const S: i128 = 1_000_000_000;
    const HOUR: i128 = 3600 * S;

    fn key(mtime: Option<i128>, size: u64, id: Option<u128>) -> StatKey {
        StatKey {
            rel_path: "About/About.xml".into(),
            size,
            mtime_ns: mtime,
            file_id: id,
        }
    }

    #[test]
    fn exact_policy_needs_everything_equal() {
        let a = key(Some(10 * S), 5, Some(1));
        assert_eq!(FatPolicy::Exact.compare(&a, &a.clone()), KeyVerdict::Same);
        assert_eq!(
            FatPolicy::Exact.compare(&a, &key(Some(10 * S + 1), 5, Some(1))),
            KeyVerdict::Changed
        );
        assert_eq!(
            FatPolicy::Exact.compare(&a, &key(Some(10 * S), 6, Some(1))),
            KeyVerdict::Changed
        );
        assert_eq!(
            FatPolicy::Exact.compare(&a, &key(Some(10 * S), 5, Some(2))),
            KeyVerdict::Changed
        );
        let mut other = a.clone();
        other.rel_path = "x".into();
        assert_eq!(FatPolicy::Exact.compare(&a, &other), KeyVerdict::Changed);
    }

    #[rstest]
    #[case(0, KeyVerdict::Same)]
    #[case(1, KeyVerdict::Same)] // within the same two second bucket (an even second plus one)
    #[case(S, KeyVerdict::Same)]
    #[case(2 * S, KeyVerdict::NeedsHash)]
    #[case(HOUR, KeyVerdict::NeedsHash)]
    #[case(-HOUR, KeyVerdict::NeedsHash)]
    fn fat_policy_rounds_to_two_seconds_and_hashes_on_big_shifts(
        #[case] shift: i128,
        #[case] expect: KeyVerdict,
    ) {
        let base = 1_000 * S; // an even second
        let old = key(Some(base), 5, Some(1));
        let new = key(Some(base + shift), 5, Some(99));
        assert_eq!(FatPolicy::Fat.compare(&old, &new), expect);
    }

    #[test]
    fn fat_policy_size_change_is_always_a_change() {
        let old = key(Some(5 * S), 5, None);
        assert_eq!(
            FatPolicy::Fat.compare(&old, &key(Some(5 * S), 6, None)),
            KeyVerdict::Changed
        );
    }

    #[test]
    fn missing_mtime_on_fat_asks_for_a_hash() {
        let old = key(None, 5, None);
        assert_eq!(
            FatPolicy::Fat.compare(&old, &old.clone()),
            KeyVerdict::NeedsHash
        );
    }

    #[test]
    fn volume_classes_map_to_policies() {
        assert_eq!(FatPolicy::for_volume(VolumeClass::Fat), FatPolicy::Fat);
        assert_eq!(FatPolicy::for_volume(VolumeClass::Exfat), FatPolicy::Fat);
        assert_eq!(FatPolicy::for_volume(VolumeClass::Ntfs), FatPolicy::Exact);
        assert_eq!(FatPolicy::for_volume(VolumeClass::Posix), FatPolicy::Exact);
        assert_eq!(
            FatPolicy::for_volume(VolumeClass::Unknown),
            FatPolicy::Exact
        );
    }

    #[test]
    fn hour_shift_with_same_content_is_unchanged_by_hash_and_changed_content_is_not() {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("f.txt")).unwrap();
        fs_err::write(&p, b"same bytes").unwrap();
        let h = hash_file(&p).unwrap();
        let old = key(Some(100 * S), 10, None);
        let shifted = key(Some(100 * S + HOUR), 10, None);
        let (same, fresh) = FatPolicy::Fat
            .is_unchanged(&old, Some(&h), &shifted, &p)
            .unwrap();
        assert!(same);
        assert_eq!(fresh.as_deref(), Some(h.as_str()));
        fs_err::write(&p, b"other byte").unwrap();
        let (same, fresh) = FatPolicy::Fat
            .is_unchanged(&old, Some(&h), &shifted, &p)
            .unwrap();
        assert!(!same);
        assert!(fresh.is_some());
        // No stored hash: treated as changed, but the fresh hash is handed back.
        let (same, fresh) = FatPolicy::Fat
            .is_unchanged(&old, None, &shifted, &p)
            .unwrap();
        assert!(!same);
        assert!(fresh.is_some());
        // No hash work for equal keys.
        let (same, fresh) = FatPolicy::Fat.is_unchanged(&old, None, &old, &p).unwrap();
        assert!(same && fresh.is_none());
    }

    #[test]
    fn stat_reads_size_and_mtime_and_uses_the_hook() {
        let t = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(t.path()).unwrap();
        fs_err::write(root.join("a.txt"), b"abc").unwrap();
        fn hook(_: &std::fs::Metadata) -> Option<u128> {
            Some(42)
        }
        let k = StatKey::stat(root, "a.txt", Some(hook)).unwrap();
        assert_eq!(k.size, 3);
        assert!(k.mtime_ns.is_some());
        assert_eq!(k.file_id, Some(42));
        assert_eq!(StatKey::stat(root, "a.txt", None).unwrap().file_id, None);
        assert!(StatKey::stat(root, "missing", None).is_err());
    }

    #[test]
    fn keys_serialise_with_camel_case_and_round_trip() {
        let k = key(Some(-5 * S), 7, Some(u128::from(u64::MAX) + 5));
        let text = serde_json::to_string(&k).unwrap();
        assert!(text.contains("relPath") && text.contains("mtimeNs") && text.contains("fileId"));
        assert_eq!(serde_json::from_str::<StatKey>(&text).unwrap(), k);
    }

    #[test]
    fn hash_is_stable_blake3() {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("e")).unwrap();
        fs_err::write(&p, b"").unwrap();
        assert_eq!(
            hash_file(&p).unwrap(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }
}
