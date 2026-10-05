//! The command a person can run by hand when the automatic way is refused.

use camino::Utf8Path;
use rimstudio_core::os::Os;

/// Quotes `text` for a POSIX shell with single quotes.
fn posix_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The shell command that creates a link named `link` pointing at `target`: `ln -s` on Linux and macOS,
/// `mklink /J` (a junction, which needs no privilege) in the Windows command prompt.
#[must_use]
pub fn manual_command(os: Os, target: &Utf8Path, link: &Utf8Path) -> String {
    match os {
        Os::Windows => format!(
            "mklink /J \"{}\" \"{}\"",
            link.as_str().replace('/', "\\"),
            target.as_str().replace('/', "\\")
        ),
        Os::MacOs | Os::Linux | Os::Other => {
            format!(
                "ln -s {} {}",
                posix_quote(target.as_str()),
                posix_quote(link.as_str())
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_quotes_spaces_and_single_quotes() {
        let c = manual_command(
            Os::Linux,
            Utf8Path::new("/home/a b/it's/RS_Mod"),
            Utf8Path::new("/games/RimWorld/Mods/RS_Mod"),
        );
        assert_eq!(
            c,
            "ln -s '/home/a b/it'\\''s/RS_Mod' '/games/RimWorld/Mods/RS_Mod'"
        );
    }

    #[test]
    fn windows_uses_a_junction_with_backslashes() {
        let c = manual_command(
            Os::Windows,
            Utf8Path::new("C:/Mods/RS_Mod"),
            Utf8Path::new("C:/Games/RimWorld/Mods/RS_Mod"),
        );
        assert_eq!(
            c,
            "mklink /J \"C:\\Games\\RimWorld\\Mods\\RS_Mod\" \"C:\\Mods\\RS_Mod\""
        );
    }
}
