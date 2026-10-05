//! The diagnostic codes of imported assets (textures and custom sounds). They follow the conventions of
//! [`super::codes`] and are looked up through [`super::codes::lookup`] as well.

use rimstudio_core::diag::Severity;

use super::codes::CodeInfo;

macro_rules! asset_code {
    ($(#[$doc:meta])* $ident:ident, $code:literal, $sev:ident, $req:literal, $tpl:literal, [$($arg:literal),*]) => {
        $(#[$doc])*
        pub const $ident: CodeInfo = CodeInfo {
            code: $code,
            severity: Severity::$sev,
            requirement: $req,
            template: $tpl,
            args: &[$($arg),*],
        };
    };
}

asset_code!(
    /// The planner was not given the facts of a source file (the file was not inspected).
    ASSET_UNCHECKED, "design.asset-unchecked", Error, "IT-090",
    "{label}: the file {path} was not inspected", ["label", "path"]
);
asset_code!(
    /// A source is a link, a folder or an unreadable file.
    ASSET_REFUSED, "design.asset-refused", Error, "IT-090",
    "{label}: {path} is not accepted: {reason}", ["label", "path", "reason"]
);
asset_code!(
    /// The texture file does not exist.
    TEXTURE_MISSING, "design.texture-missing", Error, "IT-090",
    "{label}: the file {path} does not exist", ["label", "path"]
);
asset_code!(
    /// The texture file is not a PNG by its signature, or its header is damaged.
    TEXTURE_NOT_PNG, "design.texture-not-png", Error, "IT-090",
    "{label}: {path} is not a usable PNG image ({reason})", ["label", "path", "reason"]
);
asset_code!(
    /// The texture is above the hard limits (8 MiB, 4096 pixels on a side).
    TEXTURE_TOO_LARGE, "design.texture-too-large", Error, "IT-090",
    "{label}: {path} is {size}, above the limit of {limit}", ["label", "path", "size", "limit"]
);
asset_code!(
    /// The texture is much larger than the art of a weapon usually is.
    TEXTURE_LARGE, "design.texture-large", Warning, "IT-090",
    "{label}: {path} is {size}; the art of a weapon is usually much smaller", ["label", "path", "size"]
);
asset_code!(
    /// The texture is not square.
    TEXTURE_NOT_SQUARE, "design.texture-not-square", Info, "IT-090",
    "{label}: {path} is {width} by {height} pixels, not square", ["label", "path", "width", "height"]
);
asset_code!(
    /// A projectile texture is imported but the weapon has no projectile of its own.
    TEXTURE_NEEDS_OWN_PROJECTILE, "design.texture-needs-own-projectile", Error, "IT-090",
    "the projectile texture needs a projectile of its own; switch the own projectile on first", []
);
asset_code!(
    /// A custom shot sound on a weapon that does not shoot.
    SOUND_NEEDS_RANGED, "design.sound-needs-ranged", Error, "IT-091",
    "a custom shot sound belongs to a ranged weapon", []
);
asset_code!(
    /// A custom sound without a clip.
    SOUND_NO_CLIPS, "design.sound-no-clips", Error, "IT-091",
    "the custom sound has no clip; add at least one WAV or OGG file", []
);
asset_code!(
    /// A clip file does not exist.
    SOUND_CLIP_MISSING, "design.sound-clip-missing", Error, "IT-091",
    "the clip {path} does not exist", ["path"]
);
asset_code!(
    /// A clip is neither a WAV nor an Ogg file by its signature, or its header is damaged.
    SOUND_CLIP_NOT_AUDIO, "design.sound-clip-not-audio", Error, "IT-091",
    "{path} is not a usable WAV or OGG file ({reason})", ["path", "reason"]
);
asset_code!(
    /// A clip is above the size limit (20 MiB).
    SOUND_CLIP_TOO_LARGE, "design.sound-clip-too-large", Error, "IT-091",
    "the clip {path} is {size}, above the limit of {limit}", ["path", "size", "limit"]
);
asset_code!(
    /// A clip has more than one channel; the game plays positional sounds from mono clips.
    SOUND_STEREO, "design.sound-stereo", Warning, "IT-091",
    "the clip {path} has {channels} channels; a sound heard on the map should be mono, or it plays without position", ["path", "channels"]
);
asset_code!(
    /// A range of the custom sound is not valid.
    SOUND_RANGE_INVALID, "design.sound-range-invalid", Error, "IT-091",
    "{label} {reason}", ["label", "reason"]
);
asset_code!(
    /// The def name of the custom sound is already used by a loaded def.
    SOUND_DUPLICATE, "design.sound-duplicate", Error, "IT-091",
    "the sound def name {value} is already used by {other}", ["value", "other"]
);
asset_code!(
    /// A shot sound named in the spec (typed or cloned) is replaced by the custom sound.
    SOUND_CAST_REPLACED, "design.sound-cast-replaced", Info, "IT-091",
    "the shot sound {old} is replaced by the custom sound {value}", ["old", "value"]
);

/// Every code of imported assets, in a stable order.
pub const ASSET_REGISTRY: &[CodeInfo] = &[
    ASSET_UNCHECKED,
    ASSET_REFUSED,
    TEXTURE_MISSING,
    TEXTURE_NOT_PNG,
    TEXTURE_TOO_LARGE,
    TEXTURE_LARGE,
    TEXTURE_NOT_SQUARE,
    TEXTURE_NEEDS_OWN_PROJECTILE,
    SOUND_NEEDS_RANGED,
    SOUND_NO_CLIPS,
    SOUND_CLIP_MISSING,
    SOUND_CLIP_NOT_AUDIO,
    SOUND_CLIP_TOO_LARGE,
    SOUND_STEREO,
    SOUND_RANGE_INVALID,
    SOUND_DUPLICATE,
    SOUND_CAST_REPLACED,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_well_formed_unique_and_not_in_the_main_table() {
        let mut seen = std::collections::BTreeSet::new();
        for info in ASSET_REGISTRY {
            assert!(info.diag_code().is_well_formed(), "{}", info.code);
            assert!(seen.insert(info.code), "duplicate {}", info.code);
            assert!(
                !super::super::codes::REGISTRY
                    .iter()
                    .any(|c| c.code == info.code),
                "{} is in both tables",
                info.code
            );
        }
    }

    #[test]
    fn templates_use_only_declared_arguments() {
        for info in ASSET_REGISTRY {
            let mut rest = info.template;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                let close = after.find('}').unwrap();
                let name = &after[..close];
                assert!(
                    info.args.contains(&name),
                    "{}: undeclared {name}",
                    info.code
                );
                rest = &after[close + 1..];
            }
            for arg in info.args {
                assert!(
                    info.template.contains(&format!("{{{arg}}}")),
                    "{}: unused {arg}",
                    info.code
                );
            }
        }
    }

    #[test]
    fn severities_match_the_specification() {
        assert_eq!(TEXTURE_MISSING.severity, Severity::Error);
        assert_eq!(TEXTURE_NOT_PNG.severity, Severity::Error);
        assert_eq!(TEXTURE_LARGE.severity, Severity::Warning);
        assert_eq!(TEXTURE_NOT_SQUARE.severity, Severity::Info);
        assert_eq!(SOUND_STEREO.severity, Severity::Warning);
        assert_eq!(SOUND_DUPLICATE.severity, Severity::Error);
    }
}
