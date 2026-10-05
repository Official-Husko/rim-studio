//! Imported assets: texture files and custom sounds of a design, and `designer_asset_info`.
//!
//! The spec carries source paths on the user's machine and the settings of a custom sound; the backend reads
//! the files, plans the copies and writes a `SoundDef`. The webview never reads a file: the info query
//! returns what it needs to show (size, dimensions, a thumbnail as a data URL).

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;

/// A closed range of numbers, written `min~max` in the game's files.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FloatRangeDto {
    /// The lower end.
    pub min: f64,
    /// The upper end.
    pub max: f64,
}

/// A custom sound: clip files and the settings of the sound definition made from them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CustomSoundDto {
    /// The def name of the sound. Absent derives `<DefName>_Shot` from the weapon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub def_name: Option<String>,
    /// The clip files to copy (WAV or Ogg), as paths on this machine. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub clips: Vec<String>,
    /// Volume range in the game's scale (50 is the default). Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub volume: Option<FloatRangeDto>,
    /// Pitch range (1 is the clip's own pitch). Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pitch: Option<FloatRangeDto>,
    /// Distance range in tiles in which the sound is heard. Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub distance: Option<FloatRangeDto>,
    /// How many instances may play at once. Absent writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_simultaneous: Option<u32>,
}

/// The texture files of a design, as paths on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AssetImportsDto {
    /// The PNG for the weapon, copied to the conventional place of the layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub texture: Option<String>,
    /// The PNG for the weapon's own projectile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub projectile_texture: Option<String>,
}

impl AssetImportsDto {
    /// True when no texture is imported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.texture.is_none() && self.projectile_texture.is_none()
    }
}

/// The custom sounds of a design.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SoundImportsDto {
    /// The sound of a shot. Replaces a typed or cloned `soundCast`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub shot: Option<CustomSoundDto>,
}

impl SoundImportsDto {
    /// True when no custom sound is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shot.is_none()
    }
}

/// The source of a copied file in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CopyPlanDto {
    /// The source path as written in the spec.
    pub source: String,
    /// SHA-256 of the source, 64 lower case hexadecimal characters.
    pub sha256: String,
    /// Size of the source in bytes.
    pub bytes: u32,
    /// Width in pixels, for an image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub width: Option<u32>,
    /// Height in pixels, for an image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub height: Option<u32>,
    /// SHA-256 of the file that is at the target now, when there is one and it differs (it is backed up
    /// before it is replaced).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub existing_sha256: Option<String>,
}

/// What a file is, by its signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum AssetKindDto {
    /// A PNG image.
    Png,
    /// A WAV sound.
    Wav,
    /// An Ogg stream.
    Ogg,
    /// None of the formats the designer imports.
    Unknown,
}

/// What `designer_asset_info` found at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum AssetStatusDto {
    /// A regular file that was read.
    Found,
    /// Nothing is at the path.
    Missing,
    /// A link, a folder or an unreadable file; never accepted.
    Refused,
    /// A regular file above the size limit; only its size is known.
    TooLarge,
}

/// Request of `designer_asset_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerAssetInfoRequest {
    /// The file: absolute, or relative to the project root when `projectId` is given.
    pub path: String,
    /// The open project a relative path is read against. Absent: the path must be absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub project_id: Option<String>,
}

/// Response of `designer_asset_info`: the facts of one file, read without decoding it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerAssetInfoResponse {
    /// The path as asked.
    pub path: String,
    /// What was found.
    pub status: AssetStatusDto,
    /// The format by signature. Absent unless the file was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<AssetKindDto>,
    /// Size in bytes (saturated at the 32 bit limit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bytes: Option<u32>,
    /// SHA-256 of the content. Absent unless the file was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sha256: Option<String>,
    /// Width in pixels, for a PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub width: Option<u32>,
    /// Height in pixels, for a PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub height: Option<u32>,
    /// Number of channels, for a sound whose header says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub channels: Option<u32>,
    /// Sample rate, for a sound whose header says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sample_rate: Option<u32>,
    /// Length in milliseconds, for a WAV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub duration_ms: Option<u32>,
    /// A `data:image/png;base64,...` URL of the file itself, for a valid PNG up to 256 KiB, so the page can
    /// show a thumbnail without file access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub preview: Option<String>,
    /// What the designer would say about the file as a texture or a clip: size and format problems as
    /// diagnostics. Omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<DiagnosticDto>>", optional))]
    pub diagnostics: Vec<DiagnosticDto>,
}
