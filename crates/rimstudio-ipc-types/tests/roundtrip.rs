//! JSON round trip of every DTO and the naming rules of the contract.

mod common;

use common::*;
use rimstudio_ipc_types::designer::*;
use rimstudio_ipc_types::error::ApiError;
use serde_json::Value;

/// Members whose keys are data (stat names, codes, ids), not field names.
const DATA_MAPS: &[&str] = &[
    "extraStats",
    "counts",
    "stats",
    "args",
    "shortcuts",
    "answers",
    "inheritedStats",
    "variants",
    "perStat",
    "implied",
    "details",
    "tree",
];

fn collect_keys(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            let in_data_map = DATA_MAPS
                .iter()
                .any(|name| path.rsplit('/').next() == Some(name));
            for (key, inner) in map {
                if !in_data_map {
                    out.push((path.to_owned(), key.clone()));
                }
                collect_keys(inner, &format!("{path}/{key}"), out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_keys(item, path, out);
            }
        }
        _ => {}
    }
}

#[test]
fn every_dto_sample_survives_a_json_round_trip() {
    let samples = all_samples();
    assert!(samples.len() > 50, "only {} samples", samples.len());
}

#[test]
fn no_dto_field_name_contains_an_underscore() {
    for (name, value) in all_samples() {
        let mut keys = Vec::new();
        collect_keys(&value, "", &mut keys);
        for (path, key) in keys {
            assert!(
                !key.contains('_'),
                "{name}: key `{key}` at `{path}` contains an underscore"
            );
        }
    }
}

#[test]
fn no_dto_field_name_contains_a_hyphen() {
    for (name, value) in all_samples() {
        let mut keys = Vec::new();
        collect_keys(&value, "", &mut keys);
        for (path, key) in keys {
            assert!(
                !key.contains('-'),
                "{name}: key `{key}` at `{path}` contains a hyphen"
            );
        }
    }
}

#[test]
fn tag_values_in_samples_are_kebab_case() {
    fn walk(value: &Value, name: &str) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let tag_like = matches!(
                        key.as_str(),
                        "kind" | "mode" | "severity" | "status" | "action"
                    );
                    if let (true, Value::String(text)) = (tag_like, inner) {
                        assert!(
                            is_kebab(text),
                            "{name}: `{key}` = `{text}` is not kebab-case"
                        );
                    }
                    walk(inner, name);
                }
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, name)),
            _ => {}
        }
    }
    for (name, value) in all_samples() {
        walk(&value, name);
    }
}

fn is_kebab(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--")
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Serialises each variant and checks the strings.
fn check_enum<T: serde::Serialize>(variants: &[T]) {
    for variant in variants {
        let value = serde_json::to_value(variant).unwrap_or(Value::Null);
        let text = match &value {
            Value::String(s) => s.clone(),
            Value::Object(map) => match map.get("kind").or_else(|| map.get("mode")) {
                Some(Value::String(s)) => s.clone(),
                _ => panic!("tagged variant without tag: {value}"),
            },
            other => panic!("unexpected enum shape {other}"),
        };
        assert!(is_kebab(&text), "`{text}` is not kebab-case");
    }
}

#[test]
fn all_enum_strings_are_kebab_case() {
    use rimstudio_ipc_types::defs::*;
    use rimstudio_ipc_types::diagnostic::*;
    use rimstudio_ipc_types::jobs::*;
    use rimstudio_ipc_types::library::*;
    use rimstudio_ipc_types::settings::*;
    use rimstudio_ipc_types::tools::*;

    check_enum(&[
        SeverityDto::Error,
        SeverityDto::Warning,
        SeverityDto::Info,
        SeverityDto::Hint,
    ]);
    {
        use rimstudio_ipc_types::project::*;
        check_enum(&[
            LayoutProfileDto::Rimstudio,
            LayoutProfileDto::CoreStyle,
            LayoutProfileDto::Flat,
        ]);
        check_enum(&[
            NodeRoleDto::About,
            NodeRoleDto::LoadFolders,
            NodeRoleDto::ContentRoot,
            NodeRoleDto::Defs,
            NodeRoleDto::DefsWeapons,
            NodeRoleDto::DefsSounds,
            NodeRoleDto::Patches,
            NodeRoleDto::CeCompat,
            NodeRoleDto::Textures,
            NodeRoleDto::Sounds,
            NodeRoleDto::Languages,
            NodeRoleDto::Assemblies,
            NodeRoleDto::Source,
            NodeRoleDto::Other,
        ]);
        check_enum(&[TreeNodeKindDto::Folder, TreeNodeKindDto::File]);
        check_enum(&[
            LayoutFixKindDto::CreateFolder,
            LayoutFixKindDto::MoveFile,
            LayoutFixKindDto::EditLoadFolders,
            LayoutFixKindDto::AddFile,
            LayoutFixKindDto::None,
        ]);
    }
    check_enum(&[
        ProgressUnitDto::Items,
        ProgressUnitDto::Files,
        ProgressUnitDto::Bytes,
    ]);
    check_enum(&[
        CancelStateDto::Cancelling,
        CancelStateDto::Finished,
        CancelStateDto::Unknown,
    ]);
    check_enum(&[
        Capability::GameInstall,
        Capability::WorkshopFolder,
        Capability::CombatExtended,
        Capability::OpenProject,
        Capability::SteamClient,
        Capability::Network,
    ]);
    check_enum(&[
        PatchOutcomeDto::Applied,
        PatchOutcomeDto::NoMatch,
        PatchOutcomeDto::Failed,
        PatchOutcomeDto::NotSimulated,
    ]);
    check_enum(&[OsDto::Windows, OsDto::Macos, OsDto::Linux, OsDto::Other]);
    check_enum(&[
        HowDto::Override,
        HowDto::RegistryHkcu,
        HowDto::RegistryHklm,
        HowDto::XdgDataHome,
        HowDto::SymlinkSteam,
        HowDto::Flatpak,
        HowDto::Snap,
        HowDto::LibraryFoldersVdf,
        HowDto::Appmanifest,
        HowDto::DirectoryProbe,
    ]);
    check_enum(&[
        ConfidenceDto::Low,
        ConfidenceDto::Medium,
        ConfidenceDto::High,
    ]);
    check_enum(&[
        InstallKindDto::Steam,
        InstallKindDto::Gog,
        InstallKindDto::Manual,
        InstallKindDto::Override,
    ]);
    check_enum(&[
        HealthDto::Installed,
        HealthDto::UpdatePending,
        HealthDto::NeedsVerify,
    ]);
    check_enum(&[
        UserDirKindDto::Native,
        UserDirKindDto::Proton,
        UserDirKindDto::Flatpak,
        UserDirKindDto::Snap,
        UserDirKindDto::Macos,
        UserDirKindDto::Windows,
        UserDirKindDto::Override,
    ]);
    check_enum(&[
        PathFieldDto::GameInstall,
        PathFieldDto::UserDir,
        PathFieldDto::SteamRoot,
    ]);
    check_enum(&[
        SourceKindDto::GameData,
        SourceKindDto::GameMods,
        SourceKindDto::Workshop,
        SourceKindDto::Custom,
    ]);
    check_enum(&[
        SourceStatusDto::Ready,
        SourceStatusDto::Disabled,
        SourceStatusDto::Offline,
        SourceStatusDto::NotDirectory,
    ]);
    check_enum(&[
        FolderKindDto::Missing,
        FolderKindDto::NotDirectory,
        FolderKindDto::GameData,
        FolderKindDto::GameMods,
        FolderKindDto::Workshop,
        FolderKindDto::SingleMod,
        FolderKindDto::ModsRoot,
        FolderKindDto::Empty,
    ]);
    check_enum(&[
        FolderWarningDto::NoModsFound,
        FolderWarningDto::ModsFoundDeeper,
        FolderWarningDto::AmbiguousLayout,
        FolderWarningDto::PathIsLink,
        FolderWarningDto::Unreachable,
    ]);
    check_enum(&[
        OverlapRelationDto::Same,
        OverlapRelationDto::Inside,
        OverlapRelationDto::Contains,
    ]);
    check_enum(&[DensityDto::Standard, DensityDto::Compact, DensityDto::Touch]);
    check_enum(&[
        SystemToggleDto::System,
        SystemToggleDto::On,
        SystemToggleDto::Off,
    ]);
    check_enum(&[ColourModeDto::Background, ColourModeDto::Text]);
    check_enum(&[WatchModeDto::Auto, WatchModeDto::Poll, WatchModeDto::Off]);
    check_enum(&[DesignerModeDto::Simple, DesignerModeDto::Calibrated]);
    check_enum(&[UpdateChannelDto::Stable, UpdateChannelDto::Beta]);
    check_enum(&[
        LogLevelDto::Error,
        LogLevelDto::Warn,
        LogLevelDto::Info,
        LogLevelDto::Debug,
        LogLevelDto::Trace,
    ]);
    check_enum(&[
        FolderLayoutDto::Auto,
        FolderLayoutDto::ModsRoot,
        FolderLayoutDto::SingleMod,
    ]);
    check_enum(&[
        LinkModeDto::Auto,
        LinkModeDto::Links,
        LinkModeDto::Copy,
        LinkModeDto::None,
    ]);

    check_enum(&[
        ValueSourceDto::Suggested,
        ValueSourceDto::Anchor,
        ValueSourceDto::Answered,
        ValueSourceDto::Typed,
    ]);
    check_enum(&[ItemKindDto::Ranged, ItemKindDto::Melee]);
    check_enum(&[
        TechLevelDto::Neolithic,
        TechLevelDto::Medieval,
        TechLevelDto::Industrial,
        TechLevelDto::Spacer,
        TechLevelDto::Ultra,
        TechLevelDto::Archotech,
    ]);
    check_enum(&[
        QualityDto::Awful,
        QualityDto::Poor,
        QualityDto::Normal,
        QualityDto::Good,
        QualityDto::Excellent,
        QualityDto::Masterwork,
        QualityDto::Legendary,
    ]);
    check_enum(&[
        ProjectileChoiceDto::Reference("RS_Bullet".into()),
        ProjectileChoiceDto::Inline(ProjectileSpecDto::default()),
    ]);
    check_enum(&[
        CalibrationModeDto::Simple,
        CalibrationModeDto::Quiz,
        CalibrationModeDto::Anchored,
    ]);
    check_enum(&[
        ReadoutGroupDto::Ranged,
        ReadoutGroupDto::Melee,
        ReadoutGroupDto::Economy,
        ReadoutGroupDto::CombatExtended,
    ]);
    check_enum(&[
        ReadoutUnitDto::Number,
        ReadoutUnitDto::DamagePerSecond,
        ReadoutUnitDto::Seconds,
        ReadoutUnitDto::Tiles,
        ReadoutUnitDto::Silver,
        ReadoutUnitDto::Kilograms,
        ReadoutUnitDto::Fraction,
    ]);
    check_enum(&[
        SuggestionSourceDto::Typed,
        SuggestionSourceDto::Answer,
        SuggestionSourceDto::Anchor,
        SuggestionSourceDto::ClassMedian,
        SuggestionSourceDto::Quantile,
        SuggestionSourceDto::Derived,
    ]);
    check_enum(&[
        ChainLevelDto::RoleTier,
        ChainLevelDto::Role,
        ChainLevelDto::GroupTier,
        ChainLevelDto::Tier,
        ChainLevelDto::Group,
        ChainLevelDto::All,
    ]);
    check_enum(&[
        PredictorDto::Median,
        PredictorDto::Quantile,
        PredictorDto::Anchor,
    ]);
    check_enum(&[
        FitLevelDto::Typical,
        FitLevelDto::Plausible,
        FitLevelDto::Unusual,
    ]);
    check_enum(&[BucketDto::Lower, BucketDto::Similar, BucketDto::Higher]);
    check_enum(&quiz_answers());
    let questions: Vec<QuestionDto> = quiz_prompts().into_iter().map(|p| p.question).collect();
    check_enum(&questions);
    check_enum(&[
        ConvertStatusDto::NotConverted,
        ConvertStatusDto::AlreadyCe,
        ConvertStatusDto::UnsupportedKind,
        ConvertStatusDto::TargetNotFound,
    ]);
    check_enum(&[AskKindDto::Choice, AskKindDto::Flag, AskKindDto::Number]);
    check_enum(&[
        CeRatingDto::Reliable,
        CeRatingDto::Rough,
        CeRatingDto::Unreliable,
        CeRatingDto::Unmeasured,
    ]);
    check_enum(&[
        CeFieldStatusDto::Held,
        CeFieldStatusDto::Derived,
        CeFieldStatusDto::Ask,
    ]);
    check_enum(&[
        CePredictorDto::Identity,
        CePredictorDto::Median,
        CePredictorDto::Ratio,
        CePredictorDto::Elastic,
    ]);
    check_enum(&[
        CeSourceDto::Typed,
        CeSourceDto::Anchor,
        CeSourceDto::Answered,
        CeSourceDto::Identity { n: 1 },
        CeSourceDto::Predicted {
            predictor: CePredictorDto::Median,
            n: 1,
        },
        CeSourceDto::Vanilla,
        CeSourceDto::FirstOfSet,
    ]);
    check_enum(&[
        FileActionDto::Create,
        FileActionDto::UpdateRegion,
        FileActionDto::Unchanged,
    ]);
    check_enum(&[
        FileKindDto::VanillaDefs,
        FileKindDto::CePatch,
        FileKindDto::LoadFolders,
        FileKindDto::About,
    ]);
}

#[test]
fn spec_without_ce_never_serialises_a_ce_member() {
    let text = serde_json::to_string(&ranged_spec()).unwrap_or_default();
    assert!(!text.contains("\"ce\""));
    let with = serde_json::to_string(&melee_spec_with_ce()).unwrap_or_default();
    assert!(with.contains("\"ce\""));
}

#[test]
fn spec_deserialises_from_minimal_input_with_typed_default_source() {
    let spec: Result<DesignSpecDto, _> = serde_json::from_str(
        r#"{"kind":"ranged","mass":{"value":2.0},"ranged":{"damage":{"value":9.0,"source":"anchor"}}}"#,
    );
    let Ok(spec) = spec else {
        panic!("minimal spec must parse");
    };
    assert_eq!(spec.mass.map(|m| m.source), Some(ValueSourceDto::Typed));
    assert!(!spec.wants_ce_patch());
    assert_eq!(
        spec.ranged.and_then(|r| r.damage).map(|d| d.source),
        Some(ValueSourceDto::Anchor)
    );
}

#[test]
fn draft_created_from_a_spec_carries_its_kind_and_the_current_schema() {
    let d = DraftDto::new(melee_spec_with_ce());
    assert_eq!(d.kind, ItemKindDto::Melee);
    assert_eq!(d.schema_version, DRAFT_SCHEMA_VERSION);
}

#[test]
fn api_error_samples_use_registered_codes() {
    assert!(api_error().is_registered());
    let unknown = ApiError::new("zzz.unknown", "x");
    assert!(!unknown.is_registered());
}

#[test]
fn json_serialisation_is_deterministic() {
    let a = serde_json::to_string(&preview()).unwrap_or_default();
    let b = serde_json::to_string(&preview()).unwrap_or_default();
    assert_eq!(a, b);
}

mod properties {
    use proptest::prelude::*;
    use rimstudio_ipc_types::designer::{SourcedDto, ValueSourceDto};
    use rimstudio_ipc_types::error::ApiError;

    proptest! {
        #[test]
        fn api_error_round_trips_for_any_text(code in "[a-z]{1,8}\\.[a-z]{1,8}", message in ".{0,40}", id in "e-[0-9a-f]{8}") {
            let error = ApiError::with_id(code, message, id).detail("note", "x");
            let text = serde_json::to_string(&error).unwrap_or_default();
            let back: Result<ApiError, _> = serde_json::from_str(&text);
            prop_assert_eq!(back.ok(), Some(error));
        }

        #[test]
        fn sourced_value_round_trips_for_finite_numbers(value in -1.0e9f64..1.0e9, source in 0u8..4) {
            let source = match source {
                0 => ValueSourceDto::Suggested,
                1 => ValueSourceDto::Anchor,
                2 => ValueSourceDto::Answered,
                _ => ValueSourceDto::Typed,
            };
            let sourced = SourcedDto { value, source };
            let text = serde_json::to_string(&sourced).unwrap_or_default();
            // serde_json parses floats without the `float_roundtrip` feature, which can be one unit in
            // the last place off, so compare with a relative tolerance.
            let back: Result<SourcedDto<f64>, _> = serde_json::from_str(&text);
            let back = back.ok();
            prop_assert_eq!(back.map(|b| b.source), Some(source));
            let close = back.is_some_and(|b| (b.value - value).abs() <= value.abs() * 1e-14);
            prop_assert!(close);
        }
    }
}
