//! Install source detection: how the app was installed.
//!
//! [`decide_install_source`] is a pure function of [`InstallInputs`], which [`inputs_from_env`] fills
//! from an injected [`EnvProbe`]. The variable names (`FLATPAK_ID`, `SNAP`, `APPIMAGE`) come from the
//! packaging research and are unverified against each format's documentation.

use camino::Utf8PathBuf;
use rimstudio_core::os::Os;
use rimstudio_core::ports::{EnvProbe, InstallSource};

use crate::dirs::PORTABLE_MARKER;

/// The facts the decision uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallInputs {
    /// The operating system.
    pub os: Os,
    /// Value of `FLATPAK_ID`, when set and non empty.
    pub flatpak_id: Option<String>,
    /// Value of `SNAP`, when set and non empty.
    pub snap: Option<String>,
    /// Value of `APPIMAGE`, when set and non empty.
    pub appimage: Option<String>,
    /// The folder of the running executable.
    pub exe_dir: Option<Utf8PathBuf>,
    /// True when the portable marker file exists beside the executable.
    pub portable_marker: bool,
}

fn non_empty(env: &dyn EnvProbe, key: &str) -> Option<String> {
    env.var(key)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// Reads the environment facts. `portable_marker` is passed in because checking the marker file is
/// the caller's file system access.
pub fn inputs_from_env(os: Os, env: &dyn EnvProbe, portable_marker: bool) -> InstallInputs {
    InstallInputs {
        os,
        flatpak_id: non_empty(env, "FLATPAK_ID"),
        snap: non_empty(env, "SNAP"),
        appimage: non_empty(env, "APPIMAGE"),
        exe_dir: env.exe_dir(),
        portable_marker,
    }
}

/// The file name of the portable marker, `rimstudio.portable`.
pub fn portable_marker_name() -> &'static str {
    PORTABLE_MARKER
}

/// Decides the install source.
///
/// Order: Flatpak, Snap, AppImage, the portable marker, then the executable location (a macOS bundle
/// path, a path under `/usr` on Linux, a `Program Files` or `AppData\Local\Programs` path on
/// Windows), otherwise [`InstallSource::Unknown`].
pub fn decide_install_source(i: &InstallInputs) -> InstallSource {
    if i.flatpak_id.is_some() {
        return InstallSource::Flatpak;
    }
    if i.snap.is_some() {
        return InstallSource::Snap;
    }
    if i.appimage.is_some() {
        return InstallSource::AppImage;
    }
    if i.portable_marker {
        return InstallSource::Portable;
    }
    let exe = i
        .exe_dir
        .as_ref()
        .map(|p| p.as_str().replace('\\', "/").to_ascii_lowercase());
    let Some(exe) = exe else {
        return InstallSource::Unknown;
    };
    match i.os {
        Os::MacOs if exe.contains(".app/contents/") => InstallSource::MacosApp,
        Os::Linux | Os::Other if exe == "/usr" || exe.starts_with("/usr/") => {
            InstallSource::SystemPackage
        }
        Os::Windows
            if exe.contains("/program files/")
                || exe.contains("/program files (x86)/")
                || exe.contains("/appdata/local/programs/") =>
        {
            InstallSource::WindowsInstaller
        }
        _ => InstallSource::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn base(os: Os) -> InstallInputs {
        InstallInputs {
            os,
            flatpak_id: None,
            snap: None,
            appimage: None,
            exe_dir: None,
            portable_marker: false,
        }
    }

    #[rstest]
    #[case(
        Os::Linux,
        Some("dev.x.App"),
        None,
        None,
        None,
        false,
        InstallSource::Flatpak
    )]
    #[case(
        Os::Linux,
        None,
        Some("/snap/x/1"),
        None,
        None,
        false,
        InstallSource::Snap
    )]
    #[case(
        Os::Linux,
        None,
        None,
        Some("/h/App.AppImage"),
        None,
        false,
        InstallSource::AppImage
    )]
    #[case(
        Os::Linux,
        None,
        None,
        None,
        Some("/opt/app"),
        true,
        InstallSource::Portable
    )]
    #[case(
        Os::Linux,
        None,
        None,
        None,
        Some("/usr/bin"),
        false,
        InstallSource::SystemPackage
    )]
    #[case(
        Os::Linux,
        None,
        None,
        None,
        Some("/opt/app"),
        false,
        InstallSource::Unknown
    )]
    #[case(Os::Linux, None, None, None, None, false, InstallSource::Unknown)]
    #[case(
        Os::MacOs,
        None,
        None,
        None,
        Some("/Applications/RimStudio.app/Contents/MacOS"),
        false,
        InstallSource::MacosApp
    )]
    #[case(
        Os::MacOs,
        None,
        None,
        None,
        Some("/usr/local/bin"),
        false,
        InstallSource::Unknown
    )]
    #[case(
        Os::Windows,
        None,
        None,
        None,
        Some("C:\\Program Files\\RimStudio"),
        false,
        InstallSource::WindowsInstaller
    )]
    #[case(
        Os::Windows,
        None,
        None,
        None,
        Some("C:\\Users\\u\\AppData\\Local\\Programs\\RimStudio"),
        false,
        InstallSource::WindowsInstaller
    )]
    #[case(
        Os::Windows,
        None,
        None,
        None,
        Some("D:\\Tools\\RimStudio"),
        true,
        InstallSource::Portable
    )]
    #[case(
        Os::Windows,
        None,
        None,
        None,
        Some("D:\\Tools\\RimStudio"),
        false,
        InstallSource::Unknown
    )]
    fn decision_table(
        #[case] os: Os,
        #[case] flatpak: Option<&str>,
        #[case] snap: Option<&str>,
        #[case] appimage: Option<&str>,
        #[case] exe: Option<&str>,
        #[case] marker: bool,
        #[case] expected: InstallSource,
    ) {
        let i = InstallInputs {
            flatpak_id: flatpak.map(str::to_owned),
            snap: snap.map(str::to_owned),
            appimage: appimage.map(str::to_owned),
            exe_dir: exe.map(Utf8PathBuf::from),
            portable_marker: marker,
            ..base(os)
        };
        assert_eq!(decide_install_source(&i), expected);
    }

    #[test]
    fn flatpak_wins_over_everything() {
        let i = InstallInputs {
            flatpak_id: Some("a.b".into()),
            snap: Some("s".into()),
            appimage: Some("x".into()),
            portable_marker: true,
            ..base(Os::Linux)
        };
        assert_eq!(decide_install_source(&i), InstallSource::Flatpak);
    }

    struct E(Vec<(&'static str, &'static str)>);
    impl EnvProbe for E {
        fn var(&self, key: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
        fn home_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
        fn exe_dir(&self) -> Option<Utf8PathBuf> {
            Some("/usr/bin".into())
        }
    }

    #[test]
    fn inputs_ignore_empty_variables() {
        let env = E(vec![("FLATPAK_ID", "  "), ("APPIMAGE", "/a.AppImage")]);
        let i = inputs_from_env(Os::Linux, &env, false);
        assert_eq!(i.flatpak_id, None);
        assert_eq!(decide_install_source(&i), InstallSource::AppImage);
        assert_eq!(portable_marker_name(), "rimstudio.portable");
    }
}
