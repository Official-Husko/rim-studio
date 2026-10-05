//! [`RealFs`]: the operating system implementation of the read only [`FsProbe`] port.
//!
//! Std file system calls cannot be interrupted, so a call with a deadline runs on a short lived
//! helper thread and the caller stops waiting when the deadline passes (the helper finishes on its
//! own; a dead network share costs one parked thread, never a hung caller). A zero deadline or one of
//! an hour or more runs the call inline. Writes never go through this port.
//!
//! [`volume_class_from_mountinfo`] classifies the volume of a path from the text of
//! `/proc/self/mountinfo` when that file exists; it is runtime detected, with no operating system
//! conditionals, and returns [`VolumeClass::Unknown`] elsewhere.

use std::sync::mpsc;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{
    DirEntryInfo, EntryKind, FsMeta, FsProbe, PortError, PortErrorKind, PortResult, VolumeClass,
};

use crate::statkey::{FileIdFn, system_time_ns};

const INLINE_DEADLINE: Duration = Duration::from_secs(3600);

/// The real file system as an [`FsProbe`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFs {
    /// Hook that supplies a file identity for [`FsMeta::file_id`] (from `rimstudio-platform`).
    pub file_id: Option<FileIdFn>,
}

impl RealFs {
    /// A probe without a file id hook.
    pub fn new() -> Self {
        RealFs::default()
    }

    /// A probe that fills [`FsMeta::file_id`] through `hook`.
    pub fn with_file_id(hook: FileIdFn) -> Self {
        RealFs {
            file_id: Some(hook),
        }
    }
}

fn map_io(e: &std::io::Error) -> PortError {
    let kind = match e.kind() {
        std::io::ErrorKind::NotFound => PortErrorKind::NotFound,
        std::io::ErrorKind::PermissionDenied => PortErrorKind::PermissionDenied,
        std::io::ErrorKind::TimedOut => PortErrorKind::TimedOut,
        _ => PortErrorKind::Io,
    };
    PortError::new(kind, e.to_string())
}

fn with_deadline<T, F>(deadline: Duration, f: F) -> PortResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> std::io::Result<T> + Send + 'static,
{
    if deadline.is_zero() || deadline >= INLINE_DEADLINE {
        return f().map_err(|e| map_io(&e));
    }
    let (tx, rx) = mpsc::channel();
    let work = move || {
        let _ = tx.send(f());
    };
    let handle = std::thread::Builder::new()
        .name("rs-fsprobe".to_owned())
        .spawn(work);
    match handle {
        Ok(_) => match rx.recv_timeout(deadline) {
            Ok(r) => r.map_err(|e| map_io(&e)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(PortError::new(
                PortErrorKind::TimedOut,
                format!("no answer within {} ms", deadline.as_millis()),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(PortError::new(
                PortErrorKind::Io,
                "probe thread ended early",
            )),
        },
        Err(e) => Err(map_io(&e)),
    }
}

fn utf8(p: std::path::PathBuf) -> std::io::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(p)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "path is not UTF-8"))
}

fn entry_kind(ft: std::fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

impl FsProbe for RealFs {
    fn exists(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool> {
        let p = path.to_path_buf();
        with_deadline(deadline, move || p.as_std_path().try_exists())
    }

    fn is_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool> {
        let p = path.to_path_buf();
        with_deadline(deadline, move || match std::fs::metadata(&p) {
            Ok(m) => Ok(m.is_dir()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        })
    }

    fn metadata(&self, path: &Utf8Path, deadline: Duration) -> PortResult<FsMeta> {
        let p = path.to_path_buf();
        let hook = self.file_id;
        with_deadline(deadline, move || {
            let m = fs_err::symlink_metadata(&p)?;
            let kind = entry_kind(m.file_type());
            Ok(FsMeta {
                kind,
                size: if kind == EntryKind::Dir { 0 } else { m.len() },
                mtime_ns: m.modified().ok().map(system_time_ns),
                file_id: hook.and_then(|f| f(&m)),
            })
        })
    }

    fn read_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Vec<DirEntryInfo>> {
        let p = path.to_path_buf();
        with_deadline(deadline, move || {
            let mut out = Vec::new();
            for e in fs_err::read_dir(&p)? {
                let e = e?;
                let Ok(name) = e.file_name().into_string() else {
                    continue;
                };
                out.push(DirEntryInfo {
                    name,
                    kind: entry_kind(e.file_type()?),
                });
            }
            out.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(out)
        })
    }

    fn read_to_string(&self, path: &Utf8Path, deadline: Duration) -> PortResult<String> {
        let p = path.to_path_buf();
        with_deadline(deadline, move || {
            let bytes = fs_err::read(&p)?;
            String::from_utf8(bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "file is not valid UTF-8")
            })
        })
    }

    fn canonicalize(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Utf8PathBuf> {
        let p = path.to_path_buf();
        with_deadline(deadline, move || {
            utf8(dunce::canonicalize(p.as_std_path())?)
        })
    }

    fn volume_class(&self, path: &Utf8Path) -> VolumeClass {
        let Ok(text) = std::fs::read_to_string("/proc/self/mountinfo") else {
            return VolumeClass::Unknown;
        };
        let canonical = dunce::canonicalize(path.as_std_path())
            .ok()
            .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
            .unwrap_or_else(|| path.to_path_buf());
        volume_class_from_mountinfo(&text, &canonical)
    }
}

fn unescape_mount_field(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b.get(i).copied().unwrap_or(b' ');
        let octal = b
            .get(i + 1..i + 4)
            .filter(|d| c == b'\\' && d.iter().all(|x| (b'0'..=b'7').contains(x)))
            .and_then(|d| match d {
                [a, b, c] => {
                    let v =
                        u32::from(a - b'0') * 64 + u32::from(b - b'0') * 8 + u32::from(c - b'0');
                    u8::try_from(v).ok()
                }
                _ => None,
            });
        match octal {
            Some(v) => {
                out.push(v);
                i += 4;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn class_of_fs_type(fs_type: &str) -> VolumeClass {
    match fs_type {
        "vfat" | "msdos" | "fat" => VolumeClass::Fat,
        "exfat" => VolumeClass::Exfat,
        "ntfs" | "ntfs3" => VolumeClass::Ntfs,
        "ext2" | "ext3" | "ext4" | "btrfs" | "xfs" | "f2fs" | "zfs" | "jfs" | "reiserfs"
        | "tmpfs" | "overlay" | "bcachefs" => VolumeClass::Posix,
        _ => VolumeClass::Unknown,
    }
}

/// Classifies the volume that holds `path` from `/proc/self/mountinfo` text: the mount with the
/// longest mount point that contains `path` decides.
pub fn volume_class_from_mountinfo(mountinfo: &str, path: &Utf8Path) -> VolumeClass {
    let mut best: Option<(usize, VolumeClass)> = None;
    for line in mountinfo.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let mount_point = left.split(' ').nth(4).map(unescape_mount_field);
        let fs_type = right.split(' ').next().unwrap_or("");
        let Some(mp) = mount_point else { continue };
        let mp = Utf8Path::new(&mp);
        if path.starts_with(mp) {
            let depth = mp.components().count();
            if best.is_none_or(|(d, _)| depth >= d) {
                best = Some((depth, class_of_fs_type(fs_type)));
            }
        }
    }
    best.map_or(VolumeClass::Unknown, |(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: Duration = Duration::from_secs(5);

    fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        (t, p)
    }

    #[test]
    fn probes_files_and_folders() {
        let (_t, root) = tmp();
        fs_err::create_dir_all(root.join("sub")).unwrap();
        fs_err::write(root.join("b.txt"), "\u{feff}hi").unwrap();
        fs_err::write(root.join("a.txt"), "x").unwrap();
        let fs = RealFs::new();
        assert!(fs.exists(&root.join("a.txt"), D).unwrap());
        assert!(!fs.exists(&root.join("zzz"), D).unwrap());
        assert!(fs.is_dir(&root.join("sub"), D).unwrap());
        assert!(!fs.is_dir(&root.join("a.txt"), D).unwrap());
        assert!(!fs.is_dir(&root.join("zzz"), D).unwrap());
        let m = fs.metadata(&root.join("b.txt"), D).unwrap();
        assert_eq!(m.kind, EntryKind::File);
        assert_eq!(m.size, 5);
        assert!(m.mtime_ns.is_some());
        assert_eq!(
            fs.metadata(&root.join("sub"), D).unwrap().kind,
            EntryKind::Dir
        );
        let names: Vec<_> = fs
            .read_dir(&root, D)
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.kind))
            .collect();
        assert_eq!(
            names,
            [
                ("a.txt".to_owned(), EntryKind::File),
                ("b.txt".to_owned(), EntryKind::File),
                ("sub".to_owned(), EntryKind::Dir)
            ]
        );
        assert!(
            fs.read_to_string(&root.join("b.txt"), D)
                .unwrap()
                .starts_with('\u{feff}')
        );
        let canon = fs.canonicalize(&root.join("sub/.."), D).unwrap();
        assert_eq!(canon, fs.canonicalize(&root, D).unwrap());
    }

    #[test]
    fn errors_map_to_port_kinds() {
        let (_t, root) = tmp();
        let fs = RealFs::new();
        assert_eq!(
            fs.metadata(&root.join("no"), D).unwrap_err().code(),
            "port.not-found"
        );
        assert_eq!(
            fs.read_dir(&root.join("no"), D).unwrap_err().code(),
            "port.not-found"
        );
        fs_err::write(root.join("bin"), [0xff, 0xfe, 0x00, 0xc3]).unwrap();
        assert_eq!(
            fs.read_to_string(&root.join("bin"), D).unwrap_err().code(),
            "port.io"
        );
    }

    #[test]
    fn zero_deadline_runs_inline_and_a_short_one_still_answers() {
        let (_t, root) = tmp();
        let fs = RealFs::new();
        assert!(fs.exists(&root, Duration::ZERO).unwrap());
        assert!(fs.exists(&root, Duration::from_millis(500)).unwrap());
    }

    #[test]
    fn deadline_helper_reports_a_timeout() {
        let r: PortResult<()> = with_deadline(Duration::from_millis(20), || {
            std::thread::sleep(Duration::from_millis(400));
            Ok(())
        });
        assert_eq!(r.unwrap_err().code(), "port.timed-out");
    }

    #[test]
    fn hook_supplies_the_file_id() {
        let (_t, root) = tmp();
        fs_err::write(root.join("a"), "x").unwrap();
        fn hook(_: &std::fs::Metadata) -> Option<u128> {
            Some(7)
        }
        let fs = RealFs::with_file_id(hook);
        assert_eq!(fs.metadata(&root.join("a"), D).unwrap().file_id, Some(7));
        assert_eq!(
            RealFs::new().metadata(&root.join("a"), D).unwrap().file_id,
            None
        );
    }

    const MOUNTINFO: &str = "\
22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
40 22 8:17 / /run/media/u/usb\\040drive rw,relatime shared:5 - vfat /dev/sdb1 rw,fmask=0022
41 22 8:33 / /mnt/ex rw,relatime shared:6 - exfat /dev/sdc1 rw
42 22 8:49 / /mnt/win rw,relatime shared:7 - ntfs3 /dev/sdd1 rw
43 22 0:50 / /mnt/share rw,relatime shared:8 - cifs //srv/x rw
44 40 8:18 / /run/media/u/usb\\040drive/nested rw - ext4 /dev/sdb2 rw
";

    #[test]
    fn mountinfo_picks_the_longest_matching_mount() {
        let c = |p: &str| volume_class_from_mountinfo(MOUNTINFO, Utf8Path::new(p));
        assert_eq!(c("/home/u/file"), VolumeClass::Posix);
        assert_eq!(c("/run/media/u/usb drive/Mods"), VolumeClass::Fat);
        assert_eq!(c("/run/media/u/usb drive/nested/x"), VolumeClass::Posix);
        assert_eq!(c("/mnt/ex/a"), VolumeClass::Exfat);
        assert_eq!(c("/mnt/win/a"), VolumeClass::Ntfs);
        assert_eq!(c("/mnt/share/a"), VolumeClass::Unknown);
        assert_eq!(
            c("/mnt/exx"),
            VolumeClass::Posix,
            "component comparison, not string prefix"
        );
    }

    #[test]
    fn volume_class_of_a_real_path_never_panics() {
        let (_t, root) = tmp();
        let _ = RealFs::new().volume_class(&root);
        let _ = RealFs::new().volume_class(&root.join("missing"));
    }
}
