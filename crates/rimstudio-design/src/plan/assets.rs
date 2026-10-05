//! Imported assets in a plan: copy files for textures and clips, the `SoundDef` of a custom sound.
//!
//! [`apply_imports`] runs after validation ([`crate::validation::validate_assets`] has already refused a
//! missing, damaged or oversized source), on a private copy of the spec. It
//!
//! - gives the weapon the conventional texture path of the layout and plans one copy file for the PNG; the
//!   same for the PNG of an own projectile;
//! - for a custom shot sound, plans one copy file per clip into the clip folder of the layout
//!   (`Sounds/Weapons/<DefName>_Shot/<name>.<ext>`), a `SoundDef` with one `AudioGrain_Clip` per clip, in
//!   the sound definition file of the layout as one marked section per sound, and sets `soundCast` of the
//!   shooting verb to the new def.
//!
//! The output is vanilla only; Combat Extended patches never touch sounds.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::{Node, NodeBuilder};

use crate::assets::{AssetFacts, AssetInfo, Detected, SourceState};
use crate::model::{
    CustomSound, DesignSpec, FloatRange, ProjectileChoice, format_number, shot_sound_def_name,
};

use super::layout::{ProjectLayout, SoundFile, file_stem};
use super::types::{CopyPlan, FileKind, PlannedFile, SectionHeader};

/// The facts of a source path, `None` unless the file was read.
fn read_info<'a>(facts: &'a AssetFacts, path: &str) -> Option<&'a AssetInfo> {
    match facts.get(path.trim()) {
        Some(SourceState::Read(info)) => Some(info),
        _ => None,
    }
}

fn copy_plan(source: &str, info: &AssetInfo) -> CopyPlan {
    let dims = info.dimensions();
    CopyPlan {
        source: source.trim().to_owned(),
        sha256: info.sha256.clone(),
        bytes: info.bytes,
        width: dims.map(|d| d.0),
        height: dims.map(|d| d.1),
    }
}

/// True for a name Windows reserves for a device (`CON`, `NUL`, `COM1` and the like), whatever the extension.
fn is_device_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            upper
                .strip_prefix(p)
                .is_some_and(|n| n.len() == 1 && n.chars().all(|c| ('1'..='9').contains(&c)))
        })
}

/// The stem of a source file name for the copy: the part before the last dot, safe for a path and never a
/// reserved device name (a leading underscore is added to one).
fn clip_stem(source: &str) -> String {
    let name = source.trim().rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let safe = file_stem(if stem.is_empty() { "clip" } else { stem });
    if is_device_name(&safe) {
        format!("_{safe}")
    } else {
        safe
    }
}

fn range_text(r: FloatRange) -> String {
    format!("{}~{}", format_number(r.min), format_number(r.max))
}

/// The `SoundDef` of a custom sound: the context `MapOnly`, the optional simultaneous limit and one sub
/// sound with the clips as `AudioGrain_Clip` entries and the optional ranges.
#[must_use]
pub fn sound_def(def_name: &str, sound: &CustomSound, clip_paths: &[String]) -> Node {
    let mut grains = Node::new("grains");
    for path in clip_paths {
        grains.push_child(
            NodeBuilder::new("li")
                .attr("Class", "AudioGrain_Clip")
                .text_elem("clipPath", path)
                .build(),
        );
    }
    let mut sub = NodeBuilder::new("li").child(grains);
    if let Some(v) = sound.volume {
        sub = sub.text_elem("volumeRange", range_text(v));
    }
    if let Some(p) = sound.pitch {
        sub = sub.text_elem("pitchRange", range_text(p));
    }
    if let Some(d) = sound.distance {
        sub = sub.text_elem("distRange", range_text(d));
    }
    let mut subs = Node::new("subSounds");
    subs.push_child(sub.build());
    let mut b = NodeBuilder::new("SoundDef")
        .text_elem("defName", def_name)
        .text_elem("context", "MapOnly");
    if let Some(n) = sound.max_simultaneous {
        b = b.text_elem("maxSimultaneous", n.to_string());
    }
    b.child(subs).build()
}

/// Applies the imports of the spec to `spec` (texture paths, `soundCast`) and returns the files to add to
/// the plan. The sources must have passed [`crate::validation::validate_assets`]; a source without facts is
/// skipped here (validation reports it).
pub fn apply_imports(
    spec: &mut DesignSpec,
    layout: &ProjectLayout,
    facts: &AssetFacts,
    _diagnostics: &mut Vec<Diagnostic>,
) -> Vec<PlannedFile> {
    let mut files = Vec::new();
    let def_name = spec.identity.def_name.clone();

    if let Some(source) = spec.assets.texture.clone()
        && let Some(info) = read_info(facts, &source)
    {
        let tex = layout.weapon_texture_path(spec.kind, &def_name);
        files.push(PlannedFile::copy_file(
            layout.texture_file(&tex),
            copy_plan(&source, info),
        ));
        spec.texture_path = Some(tex);
        spec.omit_defaults.retain(|o| o != "texPath");
    }

    if let Some(source) = spec.assets.projectile_texture.clone()
        && let Some(info) = read_info(facts, &source)
        && let Some(ProjectileChoice::Inline(p)) =
            spec.ranged.as_mut().and_then(|r| r.projectile.as_mut())
    {
        let tex = layout.projectile_texture_path(&p.def_name);
        files.push(PlannedFile::copy_file(
            layout.texture_file(&tex),
            copy_plan(&source, info),
        ));
        p.texture_path = Some(tex);
        p.omit_defaults.retain(|o| o != "texPath");
    }

    if let (Some(sound), Some(name)) = (spec.sounds.shot.clone(), shot_sound_def_name(spec)) {
        let dir = layout.sound_clip_dir(&def_name, SoundFile::Shot);
        let folder = layout.sound_clip_folder_path(&def_name, SoundFile::Shot);
        let mut used: Vec<String> = Vec::new();
        let mut seen_sources: Vec<&str> = Vec::new();
        let mut clip_paths = Vec::new();
        for source in &sound.clips {
            let trimmed = source.trim();
            if seen_sources.contains(&trimmed) {
                continue;
            }
            seen_sources.push(trimmed);
            let Some(info) = read_info(facts, trimmed) else {
                continue;
            };
            let Some(format) = info
                .detected
                .format()
                .filter(|_| matches!(info.detected, Detected::Wav(_) | Detected::Ogg(_)))
            else {
                continue;
            };
            let base = clip_stem(trimmed);
            let mut stem = base.clone();
            let mut n = 2;
            while used.iter().any(|u| u.eq_ignore_ascii_case(&stem)) {
                stem = format!("{base}_{n}");
                n += 1;
            }
            used.push(stem.clone());
            files.push(PlannedFile::copy_file(
                format!("{dir}/{stem}.{}", format.extension()),
                copy_plan(trimmed, info),
            ));
            clip_paths.push(format!("{folder}/{stem}"));
        }
        let def = sound_def(&name, &sound, &clip_paths);
        let mut root = Node::new("Defs");
        root.push_child(def);
        files.push(PlannedFile::new_file(
            layout.sound_def_path(SoundFile::Shot),
            FileKind::VanillaDefs,
            root,
            vec![SectionHeader::banner(0, &name)],
        ));
        if let Some(r) = spec.ranged.as_mut() {
            r.sound_cast = Some(name);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{detect, png::build_png, wav::build_wav};

    fn read(bytes: &[u8]) -> SourceState {
        SourceState::Read(AssetInfo {
            bytes: bytes.len() as u64,
            sha256: "ab".repeat(32),
            detected: detect(bytes),
        })
    }

    #[test]
    fn clip_names_come_from_the_source_and_stay_unique() {
        assert_eq!(clip_stem("/home/me/My Shot 1.wav"), "My_Shot_1");
        assert_eq!(clip_stem("C:\\clips\\bang.ogg"), "bang");
        assert_eq!(clip_stem(".wav"), "clip");
        assert_eq!(clip_stem(""), "clip");
        assert_eq!(clip_stem("/a/con.wav"), "_con");
        assert_eq!(clip_stem("/a/COM3.ogg"), "_COM3");
        assert_eq!(clip_stem("/a/COM10.ogg"), "COM10");
    }

    #[test]
    fn a_texture_import_sets_the_path_and_plans_one_copy() {
        let mut spec = DesignSpec::new_ranged("RS_Gun", "gun");
        spec.assets.texture = Some("/art/gun.png".into());
        let mut facts = AssetFacts::new();
        facts.insert("/art/gun.png", read(&build_png(64, 32)));
        let mut diags = Vec::new();
        let files = apply_imports(&mut spec, &ProjectLayout::default(), &facts, &mut diags);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].path,
            "Textures/Things/Item/Equipment/WeaponRanged/RS_Gun.png"
        );
        assert_eq!(files[0].kind, FileKind::Copy);
        let copy = files[0].copy.as_ref().unwrap();
        assert_eq!((copy.width, copy.height), (Some(64), Some(32)));
        assert_eq!(
            spec.texture_path.as_deref(),
            Some("Things/Item/Equipment/WeaponRanged/RS_Gun")
        );
    }

    #[test]
    fn a_custom_sound_plans_clips_and_a_sound_def() {
        let mut spec = DesignSpec::new_ranged("RS_Gun", "gun");
        spec.identity.mod_prefix = "RS".into();
        spec.sounds.shot = Some(CustomSound {
            clips: vec!["/a/bang.wav".into(), "/b/bang.wav".into()],
            volume: Some(FloatRange::new(30.0, 34.5)),
            pitch: Some(FloatRange::new(0.95, 1.05)),
            distance: Some(FloatRange::new(10.0, 60.0)),
            max_simultaneous: Some(2),
            ..CustomSound::default()
        });
        let mut facts = AssetFacts::new();
        facts.insert("/a/bang.wav", read(&build_wav(1, 8000, 4)));
        facts.insert("/b/bang.wav", read(&build_wav(1, 8000, 8)));
        let mut diags = Vec::new();
        let files = apply_imports(&mut spec, &ProjectLayout::default(), &facts, &mut diags);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Sounds/Weapons/RS_Gun_Shot/bang.wav",
                "Sounds/Weapons/RS_Gun_Shot/bang_2.wav",
                "Defs/SoundDefs/World_Oneshots_Weapons.xml"
            ]
        );
        let def = files[2].tree.as_ref().unwrap();
        let sound = def.elements().next().unwrap();
        assert_eq!(sound.child_text("defName"), Some("RS_Gun_Shot"));
        assert_eq!(sound.child_text("context"), Some("MapOnly"));
        assert_eq!(sound.child_text("maxSimultaneous"), Some("2"));
        let li = sound
            .child("subSounds")
            .unwrap()
            .children_named("li")
            .next()
            .unwrap();
        assert_eq!(li.child_text("volumeRange"), Some("30~34.5"));
        assert_eq!(li.child_text("pitchRange"), Some("0.95~1.05"));
        assert_eq!(li.child_text("distRange"), Some("10~60"));
        let grains: Vec<_> = li.child("grains").unwrap().children_named("li").collect();
        assert_eq!(grains.len(), 2);
        assert_eq!(
            grains[1].child_text("clipPath"),
            Some("Weapons/RS_Gun_Shot/bang_2")
        );
        assert_eq!(
            spec.ranged.as_ref().unwrap().sound_cast.as_deref(),
            Some("RS_Gun_Shot")
        );
    }

    #[test]
    fn the_sound_name_gets_the_prefix_when_the_weapon_name_lacks_it() {
        let mut spec = DesignSpec::new_ranged("Rifle", "rifle");
        spec.identity.mod_prefix = "RS".into();
        spec.sounds.shot = Some(CustomSound::default());
        assert_eq!(shot_sound_def_name(&spec).as_deref(), Some("RS_Rifle_Shot"));
        spec.identity.def_name = "RS_Rifle".into();
        assert_eq!(shot_sound_def_name(&spec).as_deref(), Some("RS_Rifle_Shot"));
        spec.identity.mod_prefix = String::new();
        spec.identity.def_name = "Rifle".into();
        assert_eq!(shot_sound_def_name(&spec).as_deref(), Some("Rifle_Shot"));
        if let Some(s) = spec.sounds.shot.as_mut() {
            s.def_name = Some(" Custom_Name ".into());
        }
        assert_eq!(shot_sound_def_name(&spec).as_deref(), Some("Custom_Name"));
    }
}
