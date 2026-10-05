//! `designer_asset_info`: the facts of one source file, for the page that lets a person pick a texture or a
//! sound clip.
//!
//! The query reads the file under the size limits of the designer ([`rimstudio_design::assets`]), recognises
//! its format by signature, reads the PNG or audio header in house and never decodes anything. For a valid
//! PNG of at most 256 KiB it adds a `data:` URL so the page can show a thumbnail without file access. The
//! diagnostics are the ones the designer would report if the file were imported as a texture (a PNG or an
//! unknown file) or as a clip (a WAV or an Ogg file).

use rimstudio_design::assets::{AssetFacts, AssetInfo, Detected, SourceState};
use rimstudio_design::model::{CustomSound, DesignSpec};
use rimstudio_design::validation::validate_assets;
use rimstudio_ipc_types::designer::{
    AssetKindDto, AssetStatusDto, DesignerAssetInfoRequest, DesignerAssetInfoResponse,
};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;

use super::ctx::Ctx;
use super::dto::count;
use crate::error::ToolkitResult;
use crate::shared::assets::{AssetRole, png_preview, probe, resolve_source};

fn kind_of(detected: &Detected) -> AssetKindDto {
    match detected.format() {
        Some(rimstudio_design::assets::AssetFormat::Png) => AssetKindDto::Png,
        Some(rimstudio_design::assets::AssetFormat::Wav) => AssetKindDto::Wav,
        Some(rimstudio_design::assets::AssetFormat::Ogg) => AssetKindDto::Ogg,
        None => AssetKindDto::Unknown,
    }
}

fn blank(path: &str, status: AssetStatusDto) -> DesignerAssetInfoResponse {
    DesignerAssetInfoResponse {
        path: path.to_owned(),
        status,
        kind: None,
        bytes: None,
        sha256: None,
        width: None,
        height: None,
        channels: None,
        sample_rate: None,
        duration_ms: None,
        preview: None,
        diagnostics: Vec::new(),
    }
}

/// Reads one file for the page: format, size, hash, dimensions, a thumbnail for a small PNG and the
/// diagnostics of importing it.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ProjectNotOpen`] (and the other project errors of the environment) when
/// `projectId` is given and does not name an open project. A missing, refused or oversized file is a status
/// of the answer, not an error.
pub fn asset_info(
    ctx: &Ctx,
    req: DesignerAssetInfoRequest,
) -> ToolkitResult<DesignerAssetInfoResponse> {
    let root = match &req.project_id {
        Some(id) => Some(ctx.env().view(id)?.1.root),
        None => None,
    };
    let root = root.as_deref();
    // read once as a texture (8 MiB) and, when that says too large, nothing else: the clip limit is larger,
    // so a clip is read with its own limit afterwards
    let mut state = probe(&req.path, root, AssetRole::Texture);
    if matches!(state, SourceState::TooLarge(_)) {
        let clip = probe(&req.path, root, AssetRole::Clip);
        // a large file that is not audio stays a too large texture
        let not_audio = matches!(
            &clip,
            SourceState::Read(info) if !matches!(info.detected, Detected::Wav(_) | Detected::Ogg(_))
        );
        if !not_audio {
            state = clip;
        }
    }
    Ok(describe(&req.path, root, state))
}

fn describe(
    path: &str,
    root: Option<&camino::Utf8Path>,
    state: SourceState,
) -> DesignerAssetInfoResponse {
    match state {
        SourceState::Missing => blank(path, AssetStatusDto::Missing),
        SourceState::Refused(_) => blank(path, AssetStatusDto::Refused),
        SourceState::TooLarge(bytes) => {
            let mut out = blank(path, AssetStatusDto::TooLarge);
            out.bytes = Some(u32::try_from(bytes).unwrap_or(u32::MAX));
            out
        }
        SourceState::Read(info) => described_file(path, root, &info),
    }
}

fn described_file(
    path: &str,
    root: Option<&camino::Utf8Path>,
    info: &AssetInfo,
) -> DesignerAssetInfoResponse {
    let mut out = blank(path, AssetStatusDto::Found);
    out.kind = Some(kind_of(&info.detected));
    out.bytes = Some(count(usize::try_from(info.bytes).unwrap_or(usize::MAX)));
    out.sha256 = Some(info.sha256.clone());
    if let Some((w, h)) = info.dimensions() {
        out.width = Some(w);
        out.height = Some(h);
    }
    match &info.detected {
        Detected::Wav(w) => {
            out.channels = Some(u32::from(w.channels));
            out.sample_rate = Some(w.sample_rate);
            out.duration_ms = Some(u32::try_from(w.duration_ms).unwrap_or(u32::MAX));
        }
        Detected::Ogg(o) => {
            out.channels = o.channels.map(u32::from);
            out.sample_rate = o.sample_rate;
        }
        _ => {}
    }
    if matches!(info.detected, Detected::Png(_))
        && let Ok(resolved) = resolve_source(path, root)
    {
        out.preview = png_preview(&resolved);
    }
    // what the designer would say about the file as an import
    let mut facts = AssetFacts::new();
    facts.insert(path.trim(), SourceState::Read(info.clone()));
    let mut spec = DesignSpec::new_ranged("Asset_Check", "check");
    match &info.detected {
        Detected::Wav(_) | Detected::Ogg(_) => {
            spec.sounds.shot = Some(CustomSound {
                clips: vec![path.trim().to_owned()],
                ..CustomSound::default()
            });
        }
        _ => spec.assets.texture = Some(path.trim().to_owned()),
    }
    out.diagnostics = diagnostics_to_dtos(&validate_assets(&spec, &facts));
    out
}
