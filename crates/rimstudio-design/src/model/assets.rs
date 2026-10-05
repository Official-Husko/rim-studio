//! Imports of a design: texture files to copy into the project and custom sounds made from clips.
//!
//! The spec holds only what the user chose: source paths and sound settings. What a source file looks like
//! (size, hash, dimensions) is read by the toolkit at planning time and never stored in a draft, so a draft
//! cannot go stale when the file changes. Everything here is optional and absent by default. The plan turns
//! an import into copy files and a `SoundDef`; see `plan::assets`.

use serde::{Deserialize, Serialize};

/// A closed range of numbers, written `min~max` in the game's files.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatRange {
    /// The lower end.
    pub min: f64,
    /// The upper end.
    pub max: f64,
}

impl FloatRange {
    /// A range from two ends.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
}

/// A custom sound: clips the user brings and the settings of the `SoundDef` made from them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSound {
    /// The def name of the sound. Absent derives it from the weapon: `<DefName>_Shot` (with the mod prefix
    /// in front when the weapon's name does not start with it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def_name: Option<String>,
    /// The clip files to copy (WAV or Ogg), as paths on this machine. The game picks one at random.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<String>,
    /// The volume range in the game's scale (50 is the default). Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<FloatRange>,
    /// The pitch range (1 is the clip's own pitch). Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<FloatRange>,
    /// The distance range in tiles in which the sound is heard. Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<FloatRange>,
    /// How many instances of the sound may play at once. Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_simultaneous: Option<u32>,
}

/// The texture files of a design, as paths on this machine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetImports {
    /// The PNG for the weapon. It is copied to the conventional place of the layout and the weapon's
    /// `texPath` points at it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// The PNG for the weapon's own projectile (the weapon needs an own projectile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projectile_texture: Option<String>,
}

impl AssetImports {
    /// True when no texture is imported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.texture.is_none() && self.projectile_texture.is_none()
    }
}

/// The custom sounds of a design.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundImports {
    /// The sound of a shot (`soundCast` of the shooting verb). Replaces a typed or cloned `soundCast`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<CustomSound>,
}

impl SoundImports {
    /// True when no custom sound is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shot.is_none()
    }
}

/// The def name of the custom shot sound of a spec: the explicit name, else `<DefName>_Shot`, with the mod
/// prefix in front when the weapon's name does not start with it. `None` when the spec has no custom shot
/// sound.
#[must_use]
pub fn shot_sound_def_name(spec: &super::spec::DesignSpec) -> Option<String> {
    let sound = spec.sounds.shot.as_ref()?;
    if let Some(name) = sound
        .def_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        return Some(name.to_owned());
    }
    let weapon = spec.identity.def_name.as_str();
    let prefix = spec.identity.mod_prefix.as_str();
    if !prefix.is_empty() && !weapon.starts_with(&format!("{prefix}_")) {
        Some(format!("{prefix}_{weapon}_Shot"))
    } else {
        Some(format!("{weapon}_Shot"))
    }
}
