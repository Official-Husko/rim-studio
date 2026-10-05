//! Verified backups with a retention policy.
//!
//! [`rotate`] copies the current bytes of a file into a backup folder before the file is
//! rewritten, verifies the copy by re-reading it, and prunes old copies. The retention rule keeps
//! every backup of the current UTC day, the last backup of each earlier day inside a window, the
//! newest `keep_last` backups, and always at least the newest one. Backups are plain copies in the
//! original format named `<file>.<yyyymmddThhmmssmmmZ>.bak`, so a person can restore by copying.
//! Time comes from an injected [`Clock`], so retention is testable.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;

use crate::atomic::atomic_write;
use crate::error::StoreError;

const DAY_MS: u64 = 86_400_000;

/// Where backups go and how long they live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPolicy {
    /// Keep the newest this many backups regardless of age.
    pub keep_last: usize,
    /// Keep every backup made on the current UTC day.
    pub keep_all_today: bool,
    /// Keep the last backup of each earlier day for this many days (0 disables the rule).
    pub daily_window_days: u32,
    /// Skip a new backup when the newest existing one holds identical bytes.
    pub skip_identical: bool,
    /// Root folder of backups; copies go to `<root>/<file name>/`. When `None` the folder is
    /// `<parent of the file>/backups/<file name>/`.
    pub root: Option<Utf8PathBuf>,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self::user_data()
    }
}

impl BackupPolicy {
    /// The policy for settings and user data files: the last 10 plus one per day for 14 days.
    pub fn user_data() -> Self {
        BackupPolicy {
            keep_last: 10,
            keep_all_today: true,
            daily_window_days: 14,
            skip_identical: true,
            root: None,
        }
    }

    /// A lighter policy for documents inside a collection: the last 3 and today's.
    pub fn light() -> Self {
        BackupPolicy {
            keep_last: 3,
            keep_all_today: false,
            daily_window_days: 0,
            skip_identical: true,
            root: None,
        }
    }

    /// The same policy with another backup root.
    pub fn with_root(mut self, root: impl Into<Utf8PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// The folder that holds the backups of `path`.
    pub fn dir_for(&self, path: &Utf8Path) -> Utf8PathBuf {
        let name = path.file_name().unwrap_or("file");
        match &self.root {
            Some(r) => r.join(name),
            None => path
                .parent()
                .filter(|p| !p.as_str().is_empty())
                .unwrap_or(Utf8Path::new("."))
                .join("backups")
                .join(name),
        }
    }
}

/// One backup copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    /// The copy.
    pub path: Utf8PathBuf,
    /// When it was made, milliseconds since the Unix epoch.
    pub unix_ms: u64,
}

/// What [`rotate`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupReport {
    /// The new copy, `None` when the source did not exist or was identical to the newest backup.
    pub created: Option<Utf8PathBuf>,
    /// The copies removed by retention.
    pub pruned: Vec<Utf8PathBuf>,
}

/// Formats a timestamp as `yyyymmddThhmmssmmmZ` (UTC).
pub fn format_stamp(unix_ms: u64) -> String {
    let days = i64::try_from(unix_ms / DAY_MS).unwrap_or(0);
    let rem = unix_ms % DAY_MS;
    let (y, m, d) = civil_from_days(days);
    let ms = rem % 1000;
    let secs = rem / 1000;
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}{ms:03}Z",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Parses the output of [`format_stamp`]; `None` for anything else.
pub fn parse_stamp(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    if b.len() != 19 || b.get(8) != Some(&b'T') || b.get(18) != Some(&b'Z') {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<u64> {
        let s = text.get(a..z)?;
        if s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    let (y, m, d) = (num(0, 4)?, num(4, 6)?, num(6, 8)?);
    let (hh, mm, ss, ms) = (num(9, 11)?, num(11, 13)?, num(13, 15)?, num(15, 18)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    let days = days_from_civil(
        i64::try_from(y).ok()?,
        u32::try_from(m).ok()?,
        u32::try_from(d).ok()?,
    );
    let days = u64::try_from(days).ok()?;
    Some(days * DAY_MS + ((hh * 60 + mm) * 60 + ss) * 1000 + ms)
}

// Civil date algorithms after Howard Hinnant (public domain), written out here.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn backup_name(file_name: &str, unix_ms: u64, n: u32) -> String {
    if n == 0 {
        format!("{file_name}.{}.bak", format_stamp(unix_ms))
    } else {
        format!("{file_name}.{}-{n}.bak", format_stamp(unix_ms))
    }
}

fn parse_backup_name(file_name: &str, candidate: &str) -> Option<u64> {
    let rest = candidate.strip_prefix(file_name)?.strip_prefix('.')?;
    let stamp = rest.strip_suffix(".bak")?;
    let stamp = stamp.split('-').next().unwrap_or(stamp);
    parse_stamp(stamp)
}

/// The backups of `path`, oldest first (ties broken by name).
pub fn list_backups(path: &Utf8Path, policy: &BackupPolicy) -> Vec<BackupEntry> {
    let dir = policy.dir_for(path);
    let Some(file_name) = path.file_name() else {
        return Vec::new();
    };
    let Ok(rd) = fs_err::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, BackupEntry)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let unix_ms = parse_backup_name(file_name, &name)?;
            Some((
                name.clone(),
                BackupEntry {
                    path: dir.join(&name),
                    unix_ms,
                },
            ))
        })
        .collect();
    out.sort_by(|a, b| (a.1.unix_ms, &a.0).cmp(&(b.1.unix_ms, &b.0)));
    out.into_iter().map(|(_, e)| e).collect()
}

/// Decides which of `stamps` (ascending) survive. Pure; the newest always survives.
pub fn select_keep(stamps: &[u64], now_ms: u64, policy: &BackupPolicy) -> Vec<bool> {
    let n = stamps.len();
    let mut keep = vec![false; n];
    if n == 0 {
        return keep;
    }
    if let Some(last) = keep.last_mut() {
        *last = true;
    }
    for flag in keep.iter_mut().rev().take(policy.keep_last) {
        *flag = true;
    }
    let today = now_ms / DAY_MS;
    let oldest_day = today.saturating_sub(u64::from(policy.daily_window_days));
    let mut last_of_day: Option<u64> = None;
    for i in (0..n).rev() {
        let day = stamps.get(i).copied().unwrap_or(0) / DAY_MS;
        if day >= today && policy.keep_all_today {
            if let Some(f) = keep.get_mut(i) {
                *f = true;
            }
            continue;
        }
        if policy.daily_window_days > 0 && day >= oldest_day && last_of_day != Some(day) {
            last_of_day = Some(day);
            if let Some(f) = keep.get_mut(i) {
                *f = true;
            }
        }
    }
    keep
}

/// Copies `path` into its backup folder, verifies the copy and prunes by `policy`. A missing source
/// is not an error and creates nothing. A failed verification removes the bad copy and returns
/// [`StoreError::WriteVerifyFailed`].
pub fn rotate(
    path: &Utf8Path,
    policy: &BackupPolicy,
    clock: &dyn Clock,
) -> Result<BackupReport, StoreError> {
    let bytes = match fs_err::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BackupReport::default()),
        Err(e) => return Err(StoreError::io("backup-read", path, e)),
    };
    let now = clock.now_unix_ms();
    let file_name = path.file_name().unwrap_or("file");
    let dir = policy.dir_for(path);
    let existing = list_backups(path, policy);
    let mut report = BackupReport::default();

    let identical = policy.skip_identical
        && existing
            .last()
            .and_then(|e| fs_err::read(&e.path).ok())
            .is_some_and(|old| old == bytes);

    if !identical {
        let mut n = 0u32;
        let target = loop {
            let candidate = dir.join(backup_name(file_name, now, n));
            if !candidate.exists() {
                break candidate;
            }
            n = n.saturating_add(1);
            if n > 10_000 {
                return Err(StoreError::io(
                    "backup-name",
                    &dir,
                    std::io::Error::other("no free backup name"),
                ));
            }
        };
        atomic_write(&target, &bytes)?;
        let back =
            fs_err::read(&target).map_err(|e| StoreError::io("backup-verify", &target, e))?;
        if back != bytes {
            let _ = fs_err::remove_file(&target);
            return Err(StoreError::WriteVerifyFailed { path: target });
        }
        report.created = Some(target);
    }

    let all = list_backups(path, policy);
    let stamps: Vec<u64> = all.iter().map(|e| e.unix_ms).collect();
    let keep = select_keep(&stamps, now, policy);
    for (entry, keep) in all.iter().zip(keep) {
        if !keep && fs_err::remove_file(&entry.path).is_ok() {
            report.pruned.push(entry.path.clone());
        }
    }
    Ok(report)
}

/// Keeps the pre migration copy of `path` as `<backup folder>/<file>.bak-v<from_version>`. An
/// existing copy of that version is replaced. A missing source does nothing.
pub fn keep_pre_migration(
    path: &Utf8Path,
    policy: &BackupPolicy,
    from_version: u32,
) -> Result<Option<Utf8PathBuf>, StoreError> {
    let bytes = match fs_err::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(StoreError::io("backup-read", path, e)),
    };
    let target = policy.dir_for(path).join(format!(
        "{}.bak-v{from_version}",
        path.file_name().unwrap_or("file")
    ));
    atomic_write(&target, &bytes)?;
    let back = fs_err::read(&target).map_err(|e| StoreError::io("backup-verify", &target, e))?;
    if back != bytes {
        return Err(StoreError::WriteVerifyFailed { path: target });
    }
    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct TestClock(AtomicU64);
    impl TestClock {
        fn at(ms: u64) -> Self {
            TestClock(AtomicU64::new(ms))
        }
        fn set(&self, ms: u64) {
            self.0.store(ms, Ordering::SeqCst);
        }
    }
    impl Clock for TestClock {
        fn now_unix_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    // 2030-01-15T12:00:00Z
    const BASE: u64 = 1_894_708_800_000;

    fn setup() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().join("settings.jsonc")).unwrap();
        (t, p)
    }

    #[test]
    fn stamp_round_trips() {
        for ms in [0, 1, BASE, BASE + 999, 4_102_444_799_999] {
            assert_eq!(parse_stamp(&format_stamp(ms)), Some(ms), "{ms}");
        }
        assert_eq!(format_stamp(0), "19700101T000000000Z");
        assert!(parse_stamp("2030-01-15").is_none());
        assert!(parse_stamp("20301315T000000000Z").is_none());
    }

    #[test]
    fn base_constant_is_the_expected_date() {
        assert_eq!(format_stamp(BASE), "20300115T120000000Z");
    }

    #[test]
    fn missing_source_creates_nothing() {
        let (_t, p) = setup();
        let r = rotate(&p, &BackupPolicy::user_data(), &TestClock::at(BASE)).unwrap();
        assert_eq!(r, BackupReport::default());
    }

    #[test]
    fn copy_is_verified_and_identical_content_is_skipped() {
        let (_t, p) = setup();
        let pol = BackupPolicy::user_data();
        let clock = TestClock::at(BASE);
        fs_err::write(&p, b"one").unwrap();
        let r = rotate(&p, &pol, &clock).unwrap();
        let created = r.created.unwrap();
        assert_eq!(fs_err::read(created).unwrap(), b"one");
        clock.set(BASE + 10);
        assert!(rotate(&p, &pol, &clock).unwrap().created.is_none());
        fs_err::write(&p, b"two").unwrap();
        assert!(rotate(&p, &pol, &clock).unwrap().created.is_some());
        assert_eq!(list_backups(&p, &pol).len(), 2);
    }

    #[test]
    fn same_millisecond_gets_distinct_names() {
        let (_t, p) = setup();
        let pol = BackupPolicy {
            skip_identical: false,
            ..BackupPolicy::user_data()
        };
        let clock = TestClock::at(BASE);
        fs_err::write(&p, b"a").unwrap();
        rotate(&p, &pol, &clock).unwrap();
        rotate(&p, &pol, &clock).unwrap();
        assert_eq!(list_backups(&p, &pol).len(), 2);
    }

    #[test]
    fn retention_keeps_today_last_of_each_day_and_window() {
        let policy = BackupPolicy {
            keep_last: 0,
            keep_all_today: true,
            daily_window_days: 3,
            skip_identical: false,
            root: None,
        };
        let now = BASE;
        let day = DAY_MS;
        let stamps = [
            now - 10 * day,       // outside the window
            now - 3 * day - 5000, // three days ago, the only one that day
            now - 2 * day - 2000, // two days ago, superseded by the later one
            now - 2 * day - 1000, // two days ago, last of that day
            now - day,            // yesterday, only one
            now - 5000,           // today
            now - 1000,           // today
        ];
        let keep = select_keep(&stamps, now, &policy);
        // BASE is noon, so subtracting whole days stays on the same time of day.
        assert_eq!(keep, [false, true, false, true, true, true, true]);
    }

    #[test]
    fn newest_is_always_kept_even_when_ancient() {
        let policy = BackupPolicy {
            keep_last: 0,
            keep_all_today: false,
            daily_window_days: 0,
            skip_identical: false,
            root: None,
        };
        assert_eq!(select_keep(&[1, 2, 3], BASE, &policy), [false, false, true]);
        assert!(select_keep(&[], BASE, &policy).is_empty());
    }

    #[test]
    fn keep_last_applies_without_other_rules() {
        let policy = BackupPolicy {
            keep_last: 2,
            keep_all_today: false,
            daily_window_days: 0,
            skip_identical: false,
            root: None,
        };
        assert_eq!(
            select_keep(&[1, 2, 3, 4], BASE, &policy),
            [false, false, true, true]
        );
    }

    #[test]
    fn rotate_prunes_with_an_injected_clock() {
        let (_t, p) = setup();
        let pol = BackupPolicy {
            keep_last: 2,
            keep_all_today: false,
            daily_window_days: 0,
            skip_identical: false,
            root: None,
        };
        let clock = TestClock::at(BASE);
        for i in 0..5u64 {
            fs_err::write(&p, format!("v{i}")).unwrap();
            clock.set(BASE + i * 1000);
            rotate(&p, &pol, &clock).unwrap();
        }
        let left = list_backups(&p, &pol);
        assert_eq!(left.len(), 2);
        assert_eq!(fs_err::read(&left[1].path).unwrap(), b"v4");
        assert_eq!(fs_err::read(&left[0].path).unwrap(), b"v3");
    }

    #[test]
    fn custom_root_is_used() {
        let (t, p) = setup();
        let root = Utf8PathBuf::from_path_buf(t.path().join("bk")).unwrap();
        let pol = BackupPolicy::user_data().with_root(&root);
        fs_err::write(&p, b"x").unwrap();
        let r = rotate(&p, &pol, &TestClock::at(BASE)).unwrap();
        assert!(r.created.unwrap().starts_with(root.join("settings.jsonc")));
    }

    #[test]
    fn pre_migration_copy_keeps_the_old_bytes() {
        let (_t, p) = setup();
        fs_err::write(&p, b"v1 bytes").unwrap();
        let pol = BackupPolicy::user_data();
        let copy = keep_pre_migration(&p, &pol, 1).unwrap().unwrap();
        assert!(copy.as_str().ends_with("settings.jsonc.bak-v1"));
        assert_eq!(fs_err::read(copy).unwrap(), b"v1 bytes");
    }

    #[test]
    fn unrelated_files_in_the_backup_folder_are_ignored() {
        let (_t, p) = setup();
        let pol = BackupPolicy::user_data();
        fs_err::write(&p, b"x").unwrap();
        rotate(&p, &pol, &TestClock::at(BASE)).unwrap();
        let dir = pol.dir_for(&p);
        fs_err::write(dir.join("notes.txt"), b"hi").unwrap();
        fs_err::write(dir.join("settings.jsonc.bak-v1"), b"hi").unwrap();
        assert_eq!(list_backups(&p, &pol).len(), 1);
    }
}
