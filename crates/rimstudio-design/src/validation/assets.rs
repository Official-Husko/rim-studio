//! Validation of imported assets: texture files and the clips of custom sounds (items toolkit section 8.6).
//!
//! The checks are pure: they work from the spec and the [`AssetFacts`] the toolkit read from disk. Nothing
//! here opens a file. A texture must exist, be a regular file (never a link), start with the PNG signature
//! with a valid header, stay within 8 MiB and 4096 pixels on a side; a clip must exist, be a WAV or an Ogg
//! file by signature and stay within 20 MiB. A texture much larger than weapon art usually is, a texture that
//! is not square and a clip with more than one channel get a warning or a hint only.

use rimstudio_core::diag::Diagnostic;

use crate::assets::{
    AssetFacts, AssetFormat, AssetInfo, Detected, LARGE_TEXTURE_BYTES, LARGE_TEXTURE_DIMENSION,
    MAX_CLIP_BYTES, MAX_TEXTURE_BYTES, MAX_TEXTURE_DIMENSION, SourceState,
};
use crate::model::{DesignSpec, FloatRange, ItemKind, ProjectileChoice, shot_sound_def_name};

use super::asset_codes as codes;
use super::codes as base_codes;

/// A size as text for messages: `812 bytes`, `1.5 MiB`.
#[must_use]
pub fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

/// Checks the texture imports and the custom sounds of a spec against what the toolkit read from disk.
///
/// Returns diagnostics only; an error among them stops the plan. A spec without imports gives none.
#[must_use]
pub fn validate_assets(spec: &DesignSpec, facts: &AssetFacts) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(path) = &spec.assets.texture {
        check_texture(
            &mut out,
            facts,
            "/assets/texture",
            "the weapon texture",
            path,
        );
    }
    if let Some(path) = &spec.assets.projectile_texture {
        let own = matches!(
            spec.ranged.as_ref().and_then(|r| r.projectile.as_ref()),
            Some(ProjectileChoice::Inline(_))
        );
        if own {
            check_texture(
                &mut out,
                facts,
                "/assets/projectileTexture",
                "the projectile texture",
                path,
            );
        } else {
            out.push(
                codes::TEXTURE_NEEDS_OWN_PROJECTILE.diagnostic("/assets/projectileTexture", &[]),
            );
        }
    }
    if let Some(sound) = &spec.sounds.shot {
        if spec.kind != ItemKind::Ranged || spec.ranged.is_none() {
            out.push(codes::SOUND_NEEDS_RANGED.diagnostic("/sounds/shot", &[]));
            return out;
        }
        check_sound(spec, sound, facts, &mut out);
    }
    out
}

fn state_problem(
    out: &mut Vec<Diagnostic>,
    pointer: &str,
    label: &str,
    path: &str,
    state: Option<&SourceState>,
    missing: &'static super::codes::CodeInfo,
) -> Option<AssetInfo> {
    match state {
        None => {
            out.push(
                codes::ASSET_UNCHECKED.diagnostic(pointer, &[("label", label), ("path", path)]),
            );
            None
        }
        Some(SourceState::Missing) => {
            out.push(missing.diagnostic(pointer, &[("label", label), ("path", path)]));
            None
        }
        Some(SourceState::Refused(reason)) => {
            out.push(codes::ASSET_REFUSED.diagnostic(
                pointer,
                &[("label", label), ("path", path), ("reason", reason)],
            ));
            None
        }
        Some(SourceState::TooLarge(bytes)) => {
            // the file was not read; only its size is known
            out.push(too_large(
                pointer,
                label,
                path,
                *bytes,
                label.contains("texture"),
            ));
            None
        }
        Some(SourceState::Read(info)) => Some(info.clone()),
    }
}

fn too_large(pointer: &str, label: &str, path: &str, bytes: u64, texture: bool) -> Diagnostic {
    if texture {
        codes::TEXTURE_TOO_LARGE.diagnostic(
            pointer,
            &[
                ("label", label),
                ("path", path),
                ("size", &format_size(bytes)),
                ("limit", &format_size(MAX_TEXTURE_BYTES)),
            ],
        )
    } else {
        codes::SOUND_CLIP_TOO_LARGE.diagnostic(
            pointer,
            &[
                ("path", path),
                ("size", &format_size(bytes)),
                ("limit", &format_size(MAX_CLIP_BYTES)),
            ],
        )
    }
}

fn check_texture(
    out: &mut Vec<Diagnostic>,
    facts: &AssetFacts,
    pointer: &str,
    label: &str,
    path: &str,
) {
    let path = path.trim();
    let state = facts.get(path);
    let Some(info) = state_problem(out, pointer, label, path, state, &codes::TEXTURE_MISSING)
    else {
        return;
    };
    let diag = |code: &super::codes::CodeInfo, extra: &[(&str, &str)]| -> Diagnostic {
        let mut a: Vec<(&str, &str)> = vec![("label", label), ("path", path)];
        a.extend_from_slice(extra);
        code.diagnostic(pointer, &a)
    };
    let png = match &info.detected {
        Detected::Png(png) => *png,
        Detected::Broken {
            format: AssetFormat::Png,
            reason,
        } => {
            out.push(diag(&codes::TEXTURE_NOT_PNG, &[("reason", reason)]));
            return;
        }
        _ => {
            out.push(diag(
                &codes::TEXTURE_NOT_PNG,
                &[("reason", "it does not start with the PNG signature")],
            ));
            return;
        }
    };
    if !png.complete {
        out.push(diag(
            &codes::TEXTURE_NOT_PNG,
            &[("reason", "the file looks cut off: it has no end chunk")],
        ));
        return;
    }
    if info.bytes > MAX_TEXTURE_BYTES {
        out.push(too_large(pointer, label, path, info.bytes, true));
        return;
    }
    if png.width > MAX_TEXTURE_DIMENSION || png.height > MAX_TEXTURE_DIMENSION {
        let size = format!("{} by {} pixels", png.width, png.height);
        let limit = format!("{MAX_TEXTURE_DIMENSION} pixels on a side");
        out.push(diag(
            &codes::TEXTURE_TOO_LARGE,
            &[("size", &size), ("limit", &limit)],
        ));
        return;
    }
    if png.width > LARGE_TEXTURE_DIMENSION
        || png.height > LARGE_TEXTURE_DIMENSION
        || info.bytes > LARGE_TEXTURE_BYTES
    {
        let size = format!(
            "{} by {} pixels and {}",
            png.width,
            png.height,
            format_size(info.bytes)
        );
        out.push(diag(&codes::TEXTURE_LARGE, &[("size", &size)]));
    }
    if png.width != png.height {
        let (w, h) = (png.width.to_string(), png.height.to_string());
        out.push(diag(
            &codes::TEXTURE_NOT_SQUARE,
            &[("width", &w), ("height", &h)],
        ));
    }
}

fn check_sound(
    spec: &DesignSpec,
    sound: &crate::model::CustomSound,
    facts: &AssetFacts,
    out: &mut Vec<Diagnostic>,
) {
    let name = shot_sound_def_name(spec).unwrap_or_default();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        out.push(base_codes::NAME_INVALID.diagnostic(
            "/sounds/shot/defName",
            &[
                ("label", "the sound def name"),
                ("value", &name),
                (
                    "reason",
                    "use letters, digits, underscores and hyphens only",
                ),
            ],
        ));
    }
    if let Some(old) = spec
        .ranged
        .as_ref()
        .and_then(|r| r.sound_cast.as_deref())
        .filter(|o| !o.is_empty() && *o != name)
    {
        out.push(
            codes::SOUND_CAST_REPLACED
                .diagnostic("/ranged/soundCast", &[("old", old), ("value", &name)]),
        );
    }
    if sound.clips.is_empty() {
        out.push(codes::SOUND_NO_CLIPS.diagnostic("/sounds/shot/clips", &[]));
    }
    let mut seen: Vec<&str> = Vec::new();
    for (i, clip) in sound.clips.iter().enumerate() {
        let pointer = format!("/sounds/shot/clips/{i}");
        let path = clip.trim();
        if seen.contains(&path) {
            out.push(
                base_codes::DUPLICATE_ENTRY
                    .diagnostic(&pointer, &[("label", "the clip list"), ("value", path)]),
            );
            continue;
        }
        seen.push(path);
        let Some(info) = state_problem(
            out,
            &pointer,
            "the clip",
            path,
            facts.get(path),
            &codes::SOUND_CLIP_MISSING,
        ) else {
            continue;
        };
        match &info.detected {
            Detected::Wav(_) | Detected::Ogg(_) => {}
            Detected::Broken { format, reason } if *format != AssetFormat::Png => {
                out.push(
                    codes::SOUND_CLIP_NOT_AUDIO
                        .diagnostic(&pointer, &[("path", path), ("reason", reason)]),
                );
                continue;
            }
            _ => {
                out.push(codes::SOUND_CLIP_NOT_AUDIO.diagnostic(
                    &pointer,
                    &[
                        ("path", path),
                        ("reason", "it is not a WAV or an Ogg file by its signature"),
                    ],
                ));
                continue;
            }
        }
        if info.bytes > MAX_CLIP_BYTES {
            out.push(too_large(&pointer, "the clip", path, info.bytes, false));
            continue;
        }
        if let Some(channels) = info.detected.channels().filter(|c| *c > 1) {
            out.push(codes::SOUND_STEREO.diagnostic(
                &pointer,
                &[("path", path), ("channels", &channels.to_string())],
            ));
        }
    }
    range(
        out,
        "/sounds/shot/volume",
        "the volume range",
        sound.volume,
        0.0,
    );
    range(
        out,
        "/sounds/shot/pitch",
        "the pitch range",
        sound.pitch,
        f64::MIN_POSITIVE,
    );
    range(
        out,
        "/sounds/shot/distance",
        "the distance range",
        sound.distance,
        0.0,
    );
    if sound.max_simultaneous == Some(0) {
        out.push(codes::SOUND_RANGE_INVALID.diagnostic(
            "/sounds/shot/maxSimultaneous",
            &[
                ("label", "the number of simultaneous sounds"),
                ("reason", "must be at least 1"),
            ],
        ));
    }
}

fn range(
    out: &mut Vec<Diagnostic>,
    pointer: &str,
    label: &str,
    value: Option<FloatRange>,
    floor: f64,
) {
    let Some(r) = value else { return };
    let reason = if !r.min.is_finite() || !r.max.is_finite() {
        Some("must be finite numbers")
    } else if r.min < floor || (floor > 0.0 && r.min <= 0.0) {
        Some(if floor > 0.0 {
            "must be above 0"
        } else {
            "must not be negative"
        })
    } else if r.min > r.max {
        Some("has a minimum above its maximum")
    } else {
        None
    };
    if let Some(reason) = reason {
        out.push(
            codes::SOUND_RANGE_INVALID.diagnostic(pointer, &[("label", label), ("reason", reason)]),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{detect, ogg::build_ogg_vorbis, png::build_png, wav::build_wav};
    use crate::model::{CustomSound, ProjectileSpec};
    use rimstudio_core::diag::Severity;

    fn read(bytes: &[u8]) -> SourceState {
        SourceState::Read(AssetInfo {
            bytes: bytes.len() as u64,
            sha256: "0".repeat(64),
            detected: detect(bytes),
        })
    }

    fn codes_of(d: &[Diagnostic]) -> Vec<&str> {
        d.iter().map(|d| d.code.as_str()).collect()
    }

    fn weapon() -> DesignSpec {
        DesignSpec::new_ranged("RS_Test", "test rifle")
    }

    #[test]
    fn no_imports_no_diagnostics() {
        assert!(validate_assets(&weapon(), &AssetFacts::new()).is_empty());
    }

    #[test]
    fn a_good_texture_is_clean_and_a_wide_one_is_a_hint() {
        let mut spec = weapon();
        spec.assets.texture = Some("/art/gun.png".into());
        let mut facts = AssetFacts::new();
        facts.insert("/art/gun.png", read(&build_png(64, 64)));
        assert!(validate_assets(&spec, &facts).is_empty());
        facts.insert("/art/gun.png", read(&build_png(96, 48)));
        let d = validate_assets(&spec, &facts);
        assert_eq!(codes_of(&d), ["design.texture-not-square"]);
        assert_eq!(d[0].severity, Severity::Info);
    }

    #[test]
    fn texture_problems_are_named() {
        let mut spec = weapon();
        spec.assets.texture = Some("a.png".into());
        let cases: Vec<(Option<SourceState>, &str)> = vec![
            (None, "design.asset-unchecked"),
            (Some(SourceState::Missing), "design.texture-missing"),
            (
                Some(SourceState::Refused("it is a link".into())),
                "design.asset-refused",
            ),
            (
                Some(SourceState::TooLarge(9 * 1024 * 1024)),
                "design.texture-too-large",
            ),
            (Some(read(b"GIF89a....")), "design.texture-not-png"),
            (Some(read(&build_wav(1, 8000, 4))), "design.texture-not-png"),
            (
                Some(read(&crate::assets::png::PNG_SIGNATURE)),
                "design.texture-not-png",
            ),
            (Some(read(&build_png(5000, 10))), "design.texture-too-large"),
            (Some(read(&build_png(2048, 2048))), "design.texture-large"),
        ];
        for (state, want) in cases {
            let mut facts = AssetFacts::new();
            if let Some(s) = state {
                facts.insert("a.png", s);
            }
            let d = validate_assets(&spec, &facts);
            assert!(codes_of(&d).contains(&want), "{want}: {:?}", codes_of(&d));
        }
    }

    #[test]
    fn a_cut_off_png_is_not_a_png() {
        let mut spec = weapon();
        spec.assets.texture = Some("a.png".into());
        let png = build_png(16, 16);
        let mut facts = AssetFacts::new();
        facts.insert("a.png", read(&png[..png.len() - 6]));
        assert_eq!(
            codes_of(&validate_assets(&spec, &facts)),
            ["design.texture-not-png"]
        );
    }

    #[test]
    fn a_projectile_texture_needs_an_own_projectile() {
        let mut spec = weapon();
        spec.assets.projectile_texture = Some("p.png".into());
        let mut facts = AssetFacts::new();
        facts.insert("p.png", read(&build_png(8, 8)));
        assert_eq!(
            codes_of(&validate_assets(&spec, &facts)),
            ["design.texture-needs-own-projectile"]
        );
        if let Some(r) = spec.ranged.as_mut() {
            r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
                def_name: "RS_Test_Bullet".into(),
                ..ProjectileSpec::default()
            }));
        }
        assert!(validate_assets(&spec, &facts).is_empty());
    }

    fn with_sound(clips: &[&str]) -> DesignSpec {
        let mut spec = weapon();
        spec.sounds.shot = Some(CustomSound {
            clips: clips.iter().map(|c| (*c).to_owned()).collect(),
            ..CustomSound::default()
        });
        spec
    }

    #[test]
    fn clips_are_checked_one_by_one() {
        let spec = with_sound(&["m.wav", "s.wav", "x.ogg", "t.txt", "gone.wav"]);
        let mut facts = AssetFacts::new();
        facts.insert("m.wav", read(&build_wav(1, 8000, 4)));
        facts.insert("s.wav", read(&build_wav(2, 8000, 4)));
        facts.insert("x.ogg", read(&build_ogg_vorbis(1, 8000)));
        facts.insert("t.txt", read(b"hello"));
        facts.insert("gone.wav", SourceState::Missing);
        let d = validate_assets(&spec, &facts);
        assert_eq!(
            codes_of(&d),
            [
                "design.sound-stereo",
                "design.sound-clip-not-audio",
                "design.sound-clip-missing"
            ]
        );
        assert_eq!(d[0].severity, Severity::Warning);
    }

    #[test]
    fn a_sound_without_clips_or_on_a_melee_weapon_is_refused() {
        assert_eq!(
            codes_of(&validate_assets(&with_sound(&[]), &AssetFacts::new())),
            ["design.sound-no-clips"]
        );
        let mut melee = DesignSpec::new_melee("RS_Blade", "blade");
        melee.sounds.shot = Some(CustomSound::default());
        assert_eq!(
            codes_of(&validate_assets(&melee, &AssetFacts::new())),
            ["design.sound-needs-ranged"]
        );
    }

    #[test]
    fn ranges_and_names_are_validated() {
        let mut spec = with_sound(&["m.wav"]);
        let mut facts = AssetFacts::new();
        facts.insert("m.wav", read(&build_wav(1, 8000, 4)));
        if let Some(s) = spec.sounds.shot.as_mut() {
            s.volume = Some(FloatRange::new(40.0, 30.0));
            s.pitch = Some(FloatRange::new(0.0, 1.0));
            s.distance = Some(FloatRange::new(-1.0, 5.0));
            s.max_simultaneous = Some(0);
            s.def_name = Some("bad name".into());
        }
        let d = validate_assets(&spec, &facts);
        let codes = codes_of(&d);
        assert_eq!(
            codes
                .iter()
                .filter(|c| **c == "design.sound-range-invalid")
                .count(),
            4
        );
        assert!(codes.contains(&"design.name-invalid"));
        let mut ok = with_sound(&["m.wav"]);
        if let Some(s) = ok.sounds.shot.as_mut() {
            s.volume = Some(FloatRange::new(30.0, 30.0));
            s.pitch = Some(FloatRange::new(0.9, 1.1));
        }
        assert!(validate_assets(&ok, &facts).is_empty());
    }

    #[test]
    fn a_replaced_shot_sound_is_an_info() {
        let mut spec = with_sound(&["m.wav"]);
        if let Some(r) = spec.ranged.as_mut() {
            r.sound_cast = Some("Shot_Autopistol".into());
        }
        let mut facts = AssetFacts::new();
        facts.insert("m.wav", read(&build_wav(1, 8000, 4)));
        let d = validate_assets(&spec, &facts);
        assert_eq!(codes_of(&d), ["design.sound-cast-replaced"]);
        assert_eq!(d[0].severity, Severity::Info);
    }

    #[test]
    fn a_clip_listed_twice_is_a_warning() {
        let spec = with_sound(&["m.wav", "m.wav"]);
        let mut facts = AssetFacts::new();
        facts.insert("m.wav", read(&build_wav(1, 8000, 4)));
        assert_eq!(
            codes_of(&validate_assets(&spec, &facts)),
            ["design.duplicate-entry"]
        );
    }

    #[test]
    fn sizes_read_plainly() {
        assert_eq!(format_size(12), "12 bytes");
        assert_eq!(format_size(2048), "2.0 KiB");
        assert_eq!(format_size(3 * 1024 * 1024 / 2), "1.5 MiB");
    }
}
