//! Fictional sample values of every DTO, shared by the integration tests.
//!
//! All names and numbers are invented (prefix `RS_`); nothing here comes from the game.

#![allow(dead_code, unreachable_pub)]

use std::collections::BTreeMap;

use rimstudio_ipc_types::defs::*;
use rimstudio_ipc_types::designer::*;
use rimstudio_ipc_types::diagnostic::*;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::jobs::*;
use rimstudio_ipc_types::library::*;
use rimstudio_ipc_types::mods::*;
use rimstudio_ipc_types::project::*;
use rimstudio_ipc_types::settings::*;
use rimstudio_ipc_types::tools::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn typed(value: f64) -> SourcedDto<f64> {
    SourcedDto {
        value,
        source: ValueSourceDto::Typed,
    }
}

pub fn suggested(value: f64) -> SourcedDto<f64> {
    SourcedDto {
        value,
        source: ValueSourceDto::Suggested,
    }
}

pub fn api_error() -> ApiError {
    ApiError::with_id(
        "settings.revision-conflict",
        "The settings changed since revision 41.",
        "e-8f3a21c9",
    )
    .detail("expectedRev", 41)
    .detail("currentRev", 44)
}

pub fn diagnostic() -> DiagnosticDto {
    DiagnosticDto {
        code: "design.value-out-of-range".into(),
        severity: SeverityDto::Warning,
        message: "Damage is above the plausible maximum.".into(),
        field: Some("/ranged/damage".into()),
        mod_idx: None,
        file_id: None,
        span: None,
        args: BTreeMap::from([("max".to_owned(), "500".to_owned())]),
    }
}

pub fn ranged_spec() -> DesignSpecDto {
    let mut spec = DesignSpecDto::new(ItemKindDto::Ranged);
    spec.identity = IdentityDto {
        def_name: "RS_TestRifle".into(),
        label: "test rifle".into(),
        description: "A fictional rifle.".into(),
        mod_prefix: "RS_".into(),
    };
    spec.tech_level = Some(TechLevelDto::Industrial);
    spec.role = Some("rifle".into());
    spec.mass = Some(typed(3.5));
    spec.work_to_make = Some(suggested(11_000.0));
    spec.cost_list = vec![CostEntryDto {
        def_name: "RS_Steel".into(),
        count: 40.0,
    }];
    spec.stuff = Some(StuffSpecDto {
        categories: vec!["RS_Metallic".into()],
        count: Some(suggested(20.0)),
    });
    spec.weapon_tags = vec!["RS_Gun".into()];
    spec.extra_stats = BTreeMap::from([("RS_Stat".to_owned(), typed(1.25))]);
    spec.ranged = Some(RangedInputsDto {
        verb_class: None,
        damage: Some(typed(12.0)),
        armor_penetration: None,
        range: Some(SourcedDto {
            value: 25.9,
            source: ValueSourceDto::Anchor,
        }),
        burst_count: Some(SourcedDto {
            value: 3,
            source: ValueSourceDto::Answered,
        }),
        ticks_between_burst_shots: Some(typed(9.0)),
        warmup: Some(typed(1.5)),
        cooldown: Some(typed(2.0)),
        accuracy: AccuracyInputsDto {
            touch: Some(typed(0.9)),
            short: Some(typed(0.8)),
            medium: Some(typed(0.6)),
            long: None,
        },
        projectile: Some(ProjectileChoiceDto::Inline(ProjectileSpecDto {
            def_name: "RS_Bullet".into(),
            label: "test bullet".into(),
            speed: Some(typed(70.0)),
            ..ProjectileSpecDto::default()
        })),
        sound_cast: Some("RS_Shot".into()),
        sound_cast_tail: None,
        muzzle_flash_scale: Some(9.0),
    });
    spec
}

pub fn melee_spec_with_ce() -> DesignSpecDto {
    let mut spec = DesignSpecDto::new(ItemKindDto::Melee);
    spec.identity.def_name = "RS_TestBlade".into();
    spec.tools = vec![ToolSpecDto {
        label: "edge".into(),
        capacities: vec!["Cut".into()],
        power: Some(typed(14.0)),
        cooldown_time: Some(typed(1.9)),
        armor_penetration: Some(suggested(0.3)),
        chance_factor: None,
        linked_body_parts_group: Some("RS_Blade".into()),
    }];
    spec.ce = Some(CePatchSpecDto {
        parry_bonus: Some(typed(1.1)),
        tool_penetration: vec![CeToolPenetrationDto {
            tool: "edge".into(),
            sharp: Some(typed(2.5)),
            blunt: None,
        }],
        one_handed: true,
        ..CePatchSpecDto::default()
    });
    spec
}

pub fn draft() -> DraftDto {
    let mut d = DraftDto::new(ranged_spec());
    d.calibration = CalibrationModeDto::Quiz;
    d.answers.insert(
        "tier".into(),
        serde_json::json!({"kind": "tier", "tier": 2}),
    );
    d.anchors = vec![AnchorDto {
        def_name: "RS_AnchorGun".into(),
        label: Some("anchor gun".into()),
    }];
    d.cloned_from = Some("RS_AnchorGun".into());
    d
}

pub fn clone_diff() -> DesignerCloneDiffResponse {
    DesignerCloneDiffResponse {
        source: "RS_AnchorGun".into(),
        source_label: "anchor gun".into(),
        changes: vec![
            CloneChangeDto {
                field: "/ranged/damage".into(),
                label: "damage".into(),
                old: Some(serde_json::json!(10.0)),
                new: Some(serde_json::json!(12.0)),
            },
            CloneChangeDto {
                field: "/weaponTags/1".into(),
                label: "weapon tag 2".into(),
                old: None,
                new: Some(serde_json::json!("RS_Auto")),
            },
        ],
        readouts: vec![ReadoutDeltaDto {
            key: "dps".into(),
            group: ReadoutGroupDto::Ranged,
            unit: ReadoutUnitDto::DamagePerSecond,
            old: Some(4.5),
            new: Some(5.4),
            delta: Some(0.9),
        }],
        notes: vec!["the projectile is shared with the source".into()],
    }
}

pub fn anchor_card() -> AnchorCardDto {
    AnchorCardDto {
        id: "RS_AnchorGun".into(),
        label: "anchor gun".into(),
        strength: 4.5,
        tier: 2,
        role: "rifle".into(),
        stats: BTreeMap::from([("damage".to_owned(), 10.0), ("range".to_owned(), 24.0)]),
    }
}

pub fn estimate() -> EstimateSummaryDto {
    EstimateSummaryDto {
        class_label: "rifle, industrial".into(),
        level: ChainLevelDto::RoleTier,
        class_n: 14,
        strength: 5.5,
        strength_percentile: 0.62,
        anchors: vec![anchor_card()],
        notes: vec!["The pool is small.".into()],
    }
}

pub fn preview() -> PreviewDto {
    PreviewDto {
        readouts: vec![
            ReadoutDto {
                key: "dps".into(),
                group: ReadoutGroupDto::Ranged,
                unit: ReadoutUnitDto::DamagePerSecond,
                value: Some(7.25),
                steps: vec![ReadoutStepDto {
                    label: "cycle seconds".into(),
                    value: 4.5,
                }],
            },
            ReadoutDto {
                key: "marketValue".into(),
                group: ReadoutGroupDto::Economy,
                unit: ReadoutUnitDto::Silver,
                value: None,
                steps: vec![],
            },
        ],
        suggestions: vec![
            SuggestionDto {
                field: "/ranged/range".into(),
                stat: "range".into(),
                value: Some(26.0),
                source: Some(SuggestionSourceDto::ClassMedian),
                band: Some(SuggestionBandDto {
                    p10: 20.0,
                    median: 26.0,
                    p90: 32.0,
                    p50: 1.1,
                    p80: 1.25,
                    shift: 0.0,
                }),
                level: ChainLevelDto::Role,
                predictor: PredictorDto::Median,
                n: 14,
                locked: false,
            },
            SuggestionDto {
                field: "/ranged/damage".into(),
                stat: "damage".into(),
                value: None,
                source: None,
                band: None,
                level: ChainLevelDto::All,
                predictor: PredictorDto::Quantile,
                n: 0,
                locked: true,
            },
        ],
        estimate: Some(estimate()),
        diagnostics: vec![diagnostic()],
    }
}

pub fn matrix() -> MaterialMatrixDto {
    MaterialMatrixDto {
        readout: "dps".into(),
        stuffs: vec!["RS_Steel".into(), "RS_Wood".into()],
        qualities: vec![QualityDto::Poor, QualityDto::Normal, QualityDto::Legendary],
        cells: vec![
            MatrixCellDto {
                stuff: 0,
                quality: 1,
                value: Some(6.5),
            },
            MatrixCellDto {
                stuff: 1,
                quality: 2,
                value: None,
            },
        ],
    }
}

pub fn fit_report() -> FitReportDto {
    FitReportDto {
        per_stat: vec![StatFitDto {
            stat: "damage".into(),
            value: 12.0,
            predicted: 11.0,
            p50: IntervalDto {
                low: 9.0,
                high: 13.0,
            },
            p80: IntervalDto {
                low: 7.0,
                high: 16.0,
            },
            rank: RankDto {
                below: 6,
                equal: 1,
                total: 14,
            },
            level: FitLevelDto::Typical,
            nearest_reference: 12.0,
            default_band: false,
        }],
        typicality: Some(0.8),
        summary: FitSummaryDto {
            scored: 1,
            typical: 1,
            plausible: 0,
            unusual: 0,
            share_in_p80: 1.0,
        },
        notices: FitNoticesDto {
            calibrated_at_ms: None,
            pool_size: 14,
            class_label: "rifle".into(),
            class_n: 14,
            rough: true,
            optimistic: false,
        },
        unscored: vec!["armorPenetration".into()],
    }
}

pub fn quiz_prompts() -> Vec<PromptDto> {
    let questions = vec![
        QuestionDto::Tier {
            options: vec![TierOptionDto {
                tier: 2,
                label: "industrial".into(),
                count: 9,
            }],
        },
        QuestionDto::Role {
            options: vec![NamedOptionDto {
                name: "rifle".into(),
                count: 5,
            }],
        },
        QuestionDto::Compare {
            anchor: anchor_card(),
            remaining: 3,
            asked: 1,
            budget: 4,
            information: 0.9,
        },
        QuestionDto::CloserTo {
            lower: anchor_card(),
            upper: anchor_card(),
        },
        QuestionDto::Group {
            options: vec![NamedOptionDto {
                name: "RS_Group".into(),
                count: 2,
            }],
        },
        QuestionDto::Interval {
            stat: "range".into(),
            bins: vec![
                BinDto {
                    lo: None,
                    hi: Some(20.0),
                    label: "under 20".into(),
                },
                BinDto {
                    lo: Some(20.0),
                    hi: None,
                    label: "20 and up".into(),
                },
            ],
            spread: 1.4,
        },
        QuestionDto::VsAnchor {
            stat: "damage".into(),
            anchor: anchor_card(),
            value: 10.0,
        },
    ];
    questions
        .into_iter()
        .enumerate()
        .map(|(i, question)| PromptDto {
            id: format!("q{i}"),
            question,
            number: u32::try_from(i + 1).unwrap_or(0),
            about_total: 8,
        })
        .collect()
}

pub fn quiz_answers() -> Vec<QuizAnswerDto> {
    vec![
        QuizAnswerDto::Tier { tier: 2 },
        QuizAnswerDto::Role {
            role: "rifle".into(),
        },
        QuizAnswerDto::Group {
            group: "RS_Group".into(),
        },
        QuizAnswerDto::Weaker,
        QuizAnswerDto::Same,
        QuizAnswerDto::Stronger,
        QuizAnswerDto::CloserToLower,
        QuizAnswerDto::CloserToUpper,
        QuizAnswerDto::Bin { index: 1 },
        QuizAnswerDto::Bucket {
            bucket: BucketDto::Similar,
        },
        QuizAnswerDto::Typed { value: 3.5 },
        QuizAnswerDto::NotSure,
        QuizAnswerDto::Skip,
        QuizAnswerDto::UseWhatIHave,
    ]
}

pub fn quiz_step() -> QuizStepDto {
    QuizStepDto {
        prompt: quiz_prompts().into_iter().nth(2),
        finished: false,
        answered: 2,
        estimate: Some(estimate()),
        implied: BTreeMap::from([("damage".to_owned(), "between 9 and 13".to_owned())]),
    }
}

pub fn calibrate_result() -> CalibrateResultDto {
    let metrics = StatMetricsDto {
        n: 14,
        median_error: Some(0.12),
        p80_error: Some(0.25),
        mean_error: Some(0.15),
        factor_p50: 1.1,
        factor_p80: 1.3,
        shift: 0.0,
        coverage_p50: 0.5,
        coverage_p80: 0.8,
        rank_correlation: None,
        baseline_median_error: Some(0.2),
    };
    CalibrateResultDto {
        kind: ItemKindDto::Ranged,
        harness_version: 1,
        pool_size: 14,
        calibrated_at_ms: Some(1_700_000_000_000),
        variants: BTreeMap::from([(
            "quiz".to_owned(),
            VariantMetricsDto {
                name: "quiz".into(),
                replicates: 5,
                per_stat: BTreeMap::from([("damage".to_owned(), metrics)]),
                macro_median_error: Some(0.12),
                macro_mean_error: None,
                mean_questions: 6.5,
                max_questions: 9,
            },
        )]),
        twin: Some(TwinCheckDto {
            plain_error: 0.1,
            twin_free_error: 0.14,
            gap: 0.04,
            optimistic: false,
        }),
        from_cache: true,
    }
}

pub fn convert_scan() -> ConvertScanDto {
    ConvertScanDto {
        candidates: vec![
            ConvertCandidateDto {
                def_name: "RS_OldRifle".into(),
                label: "old rifle".into(),
                kind: Some(ItemKindDto::Ranged),
                status: ConvertStatusDto::NotConverted,
                reason: "A conversion can be generated.".into(),
                file: Some("Defs/Weapons.xml".into()),
                family: "ranged/RS_Rifle/RS_Shot1".into(),
                asks: vec![
                    AskItemDto {
                        field: "/ce/ammoSet".into(),
                        label: "Which ammo set does it use?".into(),
                        kind: AskKindDto::Choice,
                        options: vec!["RS_AmmoSetA".into(), "RS_AmmoSetB".into()],
                        reason: None,
                        suggestion: None,
                    },
                    AskItemDto {
                        field: "/ce/oneHanded".into(),
                        label: "Is it one handed?".into(),
                        kind: AskKindDto::Flag,
                        options: vec![],
                        reason: None,
                        suggestion: None,
                    },
                    AskItemDto {
                        field: "/ce/shotSpread".into(),
                        label: "Shot spread".into(),
                        kind: AskKindDto::Number,
                        options: vec![],
                        reason: Some("the estimate was rated unreliable".into()),
                        suggestion: Some(1.25),
                    },
                ],
            },
            ConvertCandidateDto {
                def_name: "RS_Grenade".into(),
                label: String::new(),
                kind: None,
                status: ConvertStatusDto::UnsupportedKind,
                reason: "Explosives are not supported.".into(),
                file: None,
                asks: vec![],
                family: String::new(),
            },
        ],
        counts: ConvertCountsDto {
            not_converted: 1,
            already_ce: 0,
            unsupported_kind: 1,
            target_not_found: 0,
        },
        diagnostics: vec![],
    }
}

pub fn write_plan() -> WritePlanDto {
    WritePlanDto {
        plan_id: "b3-0123456789abcdef".into(),
        files: vec![
            PlannedFileDto {
                path: "Defs/ThingDefs_Weapons/RS_TestRifle.xml".into(),
                kind: FileKindDto::VanillaDefs,
                action: FileActionDto::Create,
                rendered: "<Defs>\n  <ThingDef>\n    <defName>RS_TestRifle</defName>\n  </ThingDef>\n</Defs>\n".into(),
                diff: None,
                bytes: 76,
            },
            PlannedFileDto {
                path: "LoadFolders.xml".into(),
                kind: FileKindDto::LoadFolders,
                action: FileActionDto::UpdateRegion,
                rendered: "<loadFolders/>\n".into(),
                diff: Some("@@ -1 +1 @@\n-<loadFolders></loadFolders>\n+<loadFolders/>\n".into()),
                bytes: 15,
            },
        ],
        diagnostics: vec![diagnostic()],
        has_errors: false,
    }
}

pub fn export_request() -> DesignerExportPlanRequest {
    DesignerExportPlanRequest {
        project_id: "p-1".into(),
        draft: DraftDto::new(melee_spec_with_ce()),
        convert: Some(ConvertRequestDto {
            def_name: "RS_OldRifle".into(),
            answers: ConvertAnswersDto {
                ammo_set: Some("RS_AmmoSetA".into()),
                one_handed: Some(false),
                ..ConvertAnswersDto::default()
            },
            groups: vec![ConvertAnswerGroupDto {
                family: Some("ranged/RS_Rifle/RS_Shot1".into()),
                def_names: vec!["RS_OldRifle".into()],
                answers: serde_json::json!({"weaponTagClass": "RS_CE_Class"}),
            }],
        }),
        accept_suggestions: Some(AcceptSuggestionsDto {
            fields: vec!["bulk".into()],
        }),
    }
}

pub fn apply_report() -> ApplyReportDto {
    ApplyReportDto {
        plan_id: "b3-0123456789abcdef".into(),
        written: vec![AppliedFileDto {
            path: "Defs/ThingDefs_Weapons/RS_TestRifle.xml".into(),
            action: FileActionDto::Create,
            bytes: 76,
            backup_path: None,
            verified: true,
        }],
        unchanged: vec!["About/About.xml".into()],
        dry_apply_ok: Some(true),
        diagnostics: vec![],
    }
}

pub fn detection() -> DetectionReportDto {
    DetectionReportDto {
        schema: 1,
        generated_at_ms: 1_700_000_000_000,
        os: OsDto::Linux,
        steam_roots: vec![],
        libraries: vec![SteamLibraryDto {
            path: "/fiction/lib".into(),
            canonical: "/fiction/lib".into(),
            how: HowDto::LibraryFoldersVdf,
            root: "/fiction/steam".into(),
            online: true,
            timed_out: false,
            label: String::new(),
            has_app: true,
            stale: false,
        }],
        installs: vec![],
        user_dirs: vec![UserDirDto {
            path: "/fiction/userdata".into(),
            kind: UserDirKindDto::Native,
            mods_config_exists: true,
            mods_config_mtime_ms: Some(1_700_000_000_000),
            game_version: Some("9.9.1".into()),
            player_log: None,
            prefs: None,
            newest: true,
        }],
        selected: SelectedDto::default(),
        warnings: vec![DetectionWarningDto {
            code: "detect.stale-library".into(),
            message: "A library may be out of date.".into(),
            paths: vec!["/fiction/lib".into()],
        }],
    }
}

pub fn source() -> SourceDto {
    SourceDto {
        id: "custom-1".into(),
        kind: SourceKindDto::Custom,
        path: "/fiction/mods".into(),
        label: "RS_Mods".into(),
        enabled: true,
        status: SourceStatusDto::Ready,
        mod_count: Some(12),
        layout: FolderLayoutDto::ModsRoot,
        scan_depth: 2,
        read_only: false,
    }
}

pub fn probe_response() -> SourcesProbeFolderResponse {
    SourcesProbeFolderResponse {
        kind: FolderKindDto::ModsRoot,
        mod_count: 12,
        suggested_depth: 2,
        suggested_layout: FolderLayoutDto::ModsRoot,
        warnings: vec![FolderWarningDto::ModsFoundDeeper],
        overlaps: vec![SourceOverlapDto {
            with: "custom-2".into(),
            relation: OverlapRelationDto::Inside,
            code: "sources.overlap".into(),
            hard: true,
        }],
        diagnostics: vec![diagnostic()],
        can_save: false,
    }
}

pub fn scan_result() -> LibraryScanResult {
    LibraryScanResult {
        rev: 3,
        stats: ScanStatsDto {
            mods_found: 12,
            ..ScanStatsDto::default()
        },
        timings: ScanTimingsDto {
            total_ms: 40,
            ..ScanTimingsDto::default()
        },
        sources: vec![SourceReportDto {
            id: "custom-1".into(),
            kind: SourceKindDto::Custom,
            path: "/fiction/mods".into(),
            status: SourceStatusDto::Ready,
            mods: 12,
        }],
        diagnostics: DiagnosticSummaryDto {
            counts: BTreeMap::from([("scan.about-missing".to_owned(), 2)]),
            samples: vec![diagnostic()],
            truncated: false,
            errors: 0,
            warnings: 2,
            infos: 0,
            hints: 0,
        },
        cancelled: false,
    }
}

pub fn settings() -> SettingsDto {
    SettingsDto::from_core(
        &rimstudio_core::settings::Settings::default(),
        &rimstudio_core::settings::WorkspaceSettings::default(),
        7,
    )
}

pub fn def_page() -> DefPage {
    DefPage {
        query_id: "q1".into(),
        total: 2,
        offset: 0,
        items: vec![DefRowDto {
            def_type: "ThingDef".into(),
            def_name: "RS_TestRifle".into(),
            label: Some("test rifle".into()),
            is_abstract: false,
            mod_id: "w1".into(),
            file: "Defs/Weapons.xml".into(),
            parent: None,
        }],
    }
}

pub fn resolved_def() -> ResolvedDefDto {
    ResolvedDefDto {
        def_type: "ThingDef".into(),
        def_name: "RS_TestRifle".into(),
        mod_id: "w1".into(),
        file: "Defs/Weapons.xml".into(),
        tree: serde_json::json!({"tag": "ThingDef", "attrs": [], "children": []}),
        patch_events: vec![PatchEventDto {
            mod_id: "w2".into(),
            file: "Patches/P.xml".into(),
            operation: "PatchOperationAdd".into(),
            outcome: PatchOutcomeDto::Applied,
            message: None,
        }],
    }
}

fn tree_file(name: &str, path: &str, role: NodeRoleDto, bytes: u64, issues: u32) -> TreeNodeDto {
    TreeNodeDto {
        name: name.into(),
        path: path.into(),
        kind: TreeNodeKindDto::File,
        role,
        bytes,
        files: 1,
        issues,
        children: Vec::new(),
    }
}

pub fn layout_issue() -> LayoutIssueDto {
    LayoutIssueDto {
        code: "layout.ce-outside-gate".into(),
        severity: SeverityDto::Warning,
        path: "Patches/RS_ce_patch.xml".into(),
        message: "The file uses Combat Extended classes outside the gated Combat Extended folder, so the game reports errors when Combat Extended is not active.".into(),
        fix: LayoutFixDto {
            kind: LayoutFixKindDto::MoveFile,
            summary: "Move the file to Compat/CombatExtended/Patches/RS_ce_patch.xml and gate that folder in LoadFolders.xml.".into(),
            automatic: false,
            targets: vec!["Compat/CombatExtended/Patches/RS_ce_patch.xml".into()],
        },
    }
}

pub fn project_tree() -> ProjectTreeDto {
    let weapons = TreeNodeDto {
        name: "Weapons".into(),
        path: "Defs/ThingDefs_Misc/Weapons".into(),
        kind: TreeNodeKindDto::Folder,
        role: NodeRoleDto::DefsWeapons,
        bytes: 5120,
        files: 1,
        issues: 0,
        children: vec![tree_file(
            "RangedIndustrial.xml",
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml",
            NodeRoleDto::DefsWeapons,
            5120,
            0,
        )],
    };
    let patches = TreeNodeDto {
        name: "Patches".into(),
        path: "Patches".into(),
        kind: TreeNodeKindDto::Folder,
        role: NodeRoleDto::Patches,
        bytes: 900,
        files: 1,
        issues: 1,
        children: vec![tree_file(
            "RS_ce_patch.xml",
            "Patches/RS_ce_patch.xml",
            NodeRoleDto::Patches,
            900,
            1,
        )],
    };
    ProjectTreeDto {
        project_id: "p-1a2b3c4d".into(),
        profile: LayoutProfileDto::CoreStyle,
        content_folder: None,
        weapons_folder: "Defs/ThingDefs_Misc/Weapons".into(),
        ce_folder: "Compat/CombatExtended".into(),
        ce_folder_exists: false,
        ce_folder_legacy: false,
        ce_gated: false,
        root: TreeNodeDto {
            name: "RS_Mod".into(),
            path: String::new(),
            kind: TreeNodeKindDto::Folder,
            role: NodeRoleDto::ContentRoot,
            bytes: 6020,
            files: 2,
            issues: 1,
            children: vec![
                TreeNodeDto {
                    name: "Defs".into(),
                    path: "Defs".into(),
                    kind: TreeNodeKindDto::Folder,
                    role: NodeRoleDto::Defs,
                    bytes: 5120,
                    files: 1,
                    issues: 0,
                    children: vec![TreeNodeDto {
                        name: "ThingDefs_Misc".into(),
                        path: "Defs/ThingDefs_Misc".into(),
                        kind: TreeNodeKindDto::Folder,
                        role: NodeRoleDto::Defs,
                        bytes: 5120,
                        files: 1,
                        issues: 0,
                        children: vec![weapons],
                    }],
                },
                patches,
            ],
        },
        counts: ProjectCountsDto {
            folders: 4,
            files: 2,
            bytes: 6020,
            def_files: 1,
            weapon_defs: 2,
            projectile_defs: 1,
            patch_files: 1,
            textures: 0,
            sounds: 0,
        },
        issues: vec![layout_issue()],
        truncated: false,
    }
}

pub fn layout_check() -> ProjectLayoutCheckDto {
    ProjectLayoutCheckDto {
        project_id: "p-1a2b3c4d".into(),
        profile: LayoutProfileDto::CoreStyle,
        issues: vec![
            layout_issue(),
            LayoutIssueDto {
                code: "layout.missing-folder".into(),
                severity: SeverityDto::Info,
                path: "Sounds".into(),
                message: "The standard folder Sounds does not exist.".into(),
                fix: LayoutFixDto {
                    kind: LayoutFixKindDto::CreateFolder,
                    summary: "Create the empty folder Sounds.".into(),
                    automatic: true,
                    targets: vec!["Sounds".into()],
                },
            },
        ],
        errors: 0,
        warnings: 1,
        infos: 1,
        auto_fixable: 1,
    }
}

pub fn project_file() -> ProjectFileDto {
    ProjectFileDto {
        path: "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml".into(),
        role: NodeRoleDto::DefsWeapons,
        bytes: 5120,
        text: "<Defs>\n</Defs>\n".into(),
        truncated: false,
        binary: false,
    }
}

pub fn project_summary() -> ProjectSummaryDto {
    ProjectSummaryDto {
        project_id: "p-1".into(),
        name: "RS_Project".into(),
        path: "/fiction/RS_Project".into(),
        package_id: Some("rs.project".into()),
        supported_versions: vec!["9.9".into()],
        has_about: true,
        has_load_folders: false,
        has_ce_gate: false,
        def_files: 3,
        diagnostics: vec![],
    }
}

pub fn tools() -> AppListToolsResponse {
    AppListToolsResponse {
        tools: vec![ToolDescriptor::resolve(
            "designer",
            &["designer", "project"],
            &[Capability::GameInstall, Capability::OpenProject],
            &[Capability::GameInstall],
        )],
    }
}

/// A suggestion with a held field, a derived field, an ask and a choice.
pub fn ce_suggestion() -> CeSuggestionDto {
    let band = CeBandDto {
        p50: CeIntervalDto {
            low: 6.5,
            high: 7.5,
        },
        p80: CeIntervalDto {
            low: 6.0,
            high: 8.0,
        },
    };
    let field = |name: &str, status: CeFieldStatusDto| CeSuggestedFieldDto {
        field: name.into(),
        label: format!("label of {name}"),
        required: true,
        status,
        held: None,
        held_source: None,
        value: None,
        source: None,
        band: None,
        rating: None,
        error: None,
        reference: None,
        reason: None,
    };
    CeSuggestionDto {
        kind: ItemKindDto::Ranged,
        def_name: "RS_TestRifle".into(),
        available: true,
        reason: None,
        toggle_on: true,
        class_label: Some("based on the 4 nearest of 12 converted weapons".into()),
        pool: 12,
        fields: vec![
            CeSuggestedFieldDto {
                value: Some(7.0),
                source: Some(CeSourceDto::Predicted {
                    predictor: CePredictorDto::Ratio,
                    n: 4,
                }),
                band: Some(band),
                rating: Some(CeRatingDto::Reliable),
                error: Some(0.12),
                ..field("/ce/bulk", CeFieldStatusDto::Derived)
            },
            CeSuggestedFieldDto {
                held: Some(1.25),
                held_source: Some(CeSourceDto::Typed),
                value: Some(1.3),
                source: Some(CeSourceDto::Identity { n: 12 }),
                rating: Some(CeRatingDto::Rough),
                ..field("/ce/swayFactor", CeFieldStatusDto::Held)
            },
            CeSuggestedFieldDto {
                rating: Some(CeRatingDto::Unreliable),
                reference: Some(0.07),
                reason: Some("the estimate 0.07 is not used".into()),
                ..field("/ce/shotSpread", CeFieldStatusDto::Ask)
            },
        ],
        choices: vec![CeChoiceDto {
            field: "/ce/ammoSet".into(),
            label: "Which caliber (ammo set) does the weapon use?".into(),
            kind: AskKindDto::Choice,
            required: true,
            status: CeFieldStatusDto::Ask,
            held: None,
            value: None,
            source: Some(CeSourceDto::FirstOfSet),
            candidates: vec![CeCandidateDto {
                name: "RS_AmmoSet".into(),
                used_by: 12,
                score: 9.5,
                first_damage: Some(9.0),
            }],
            reason: Some("pick one".into()),
        }],
        patch_numbers: vec![CeSuggestedFieldDto {
            value: Some(3.3),
            source: Some(CeSourceDto::Vanilla),
            ..field("mass", CeFieldStatusDto::Derived)
        }],
        asks: CeAskListDto {
            items: vec![CeAskDto {
                field: "/ce/shotSpread".into(),
                label: "CE shot spread".into(),
                kind: AskKindDto::Number,
                options: Vec::new(),
                reason: Some("rated unreliable".into()),
                suggestion: Some(0.07),
            }],
        },
        missing: vec!["/ce/ammoSet".into()],
        still_missing_after_accept: vec!["/ce/ammoSet".into(), "/ce/shotSpread".into()],
        notes: vec!["a note".into()],
    }
}

/// Every sample as `(name, json)`, after checking that it survives a JSON round trip unchanged.
pub fn all_samples() -> Vec<(&'static str, Value)> {
    let mut out: Vec<(&'static str, Value)> = Vec::new();
    macro_rules! add {
        ($name:literal, $value:expr) => {
            out.push(($name, round_trip(&$value)));
        };
    }
    add!("ApiError", api_error());
    add!("DiagnosticDto", diagnostic());
    add!(
        "JobHandleDto",
        JobHandleDto {
            job_id: "j1".into(),
            started_at_ms: 1_700_000_000_000,
        }
    );
    add!(
        "ProgressDto",
        ProgressDto {
            phase: "scan-metadata".into(),
            done: 4,
            total: Some(10),
            unit: ProgressUnitDto::Files,
            detail: None,
        }
    );
    add!(
        "JobResultEnvelope",
        JobResultEnvelope::Finished {
            job_id: "j1".into(),
            result: scan_result(),
            elapsed_ms: 40,
            diagnostics: vec![diagnostic()],
        }
    );
    add!(
        "JobEvent",
        JobEvent::<u32>::Started {
            job_id: "j1".into(),
            label_key: "job.scan".into(),
            phases: vec!["a".into()],
        }
    );
    add!(
        "CancelJobResponse",
        CancelJobResponse {
            state: CancelStateDto::Cancelling
        }
    );
    add!("AppListToolsResponse", tools());
    add!("SettingsDto", settings());
    add!(
        "SettingsUpdate",
        SettingsUpdate {
            expected_rev: 7,
            appearance: Some(AppearancePatch {
                density: Some(DensityDto::Compact),
                ..AppearancePatch::default()
            }),
            reset: vec!["paths.gameInstall".into()],
            ..SettingsUpdate::default()
        }
    );
    add!(
        "CustomFolderDto",
        CustomFolderDto {
            id: "custom-1".into(),
            path: "/fiction/mods".into(),
            label: "RS_Mods".into(),
            enabled: true,
            layout: FolderLayoutDto::SingleMod,
            scan_depth: 2,
            watch: Some(false),
            priority: Some(-1),
            read_only: true,
            link: LinkModeDto::None,
            volume_hint: Some(VolumeHintDto {
                mount: "/fiction".into(),
                label: "RS_Disk".into(),
                uuid: None,
            }),
            colour: Some("#aabbcc".into()),
        }
    );
    add!("DetectionReportDto", detection());
    add!("SourceDto", source());
    add!(
        "SourcesAddFolderRequest",
        SourcesAddFolderRequest {
            path: "/fiction/mods".into(),
            label: None,
            layout: Some(FolderLayoutDto::Auto),
            scan_depth: None,
        }
    );
    add!(
        "SourcesUpdateRequest",
        SourcesUpdateRequest {
            id: "custom-1".into(),
            label: Some("x".into()),
            enabled: Some(false),
            order: Some(0),
            layout: None,
            scan_depth: Some(3),
        }
    );
    add!("SourcesProbeFolderResponse", probe_response());
    add!(
        "DetectSetOverrideRequest",
        DetectSetOverrideRequest {
            field: PathFieldDto::UserDir,
            path: Some("/fiction/userdata".into()),
        }
    );
    add!(
        "DetectGetReportResponse",
        DetectGetReportResponse {
            report: Some(detection())
        }
    );
    add!("LibraryScanRequest", LibraryScanRequest::default());
    add!("LibraryScanResult", scan_result());
    add!("DefPage", def_page());
    add!("ResolvedDefDto", resolved_def());
    add!("DefSearchRequest", DefSearchRequest::default());
    add!("DesignSpecDto", ranged_spec());
    add!("DesignSpecDtoMeleeCe", melee_spec_with_ce());
    add!("DraftDto", draft());
    add!(
        "DesignerCloneRequest",
        DesignerCloneRequest {
            project_id: "p-1".into(),
            source: "RS_AnchorGun".into(),
            def_name: "RS_CloneGun".into(),
            label: Some("clone gun".into()),
            mod_prefix: Some("RS".into()),
        }
    );
    add!(
        "DesignerCloneResponse",
        DesignerCloneResponse {
            entry: DraftEntryDto {
                id: "d-1".into(),
                def_name: "RS_CloneGun".into(),
                label: "clone gun".into(),
                kind: ItemKindDto::Ranged,
                updated_at_ms: 1_700_000_000_000,
                draft: draft(),
            },
            notes: vec!["not carried: soundInteract".into()],
        }
    );
    add!(
        "DesignerCloneDiffRequest",
        DesignerCloneDiffRequest { draft: draft() }
    );
    add!("DesignerCloneDiffResponse", clone_diff());
    add!(
        "DesignerStructureDefaultsRequest",
        DesignerStructureDefaultsRequest { draft: draft() }
    );
    add!(
        "DesignerStructureDefaultsResponse",
        DesignerStructureDefaultsResponse {
            draft: draft(),
            reference: Some(StructureReferenceDto {
                def_name: "RS_AnchorGun".into(),
                label: "anchor gun".into(),
            }),
            filled: vec!["/parent".into(), "/costList".into()],
            notes: vec!["suggestions copied from the nearest reference".into()],
        }
    );
    add!(
        "DesignerDraftSaveRequest",
        DesignerDraftSaveRequest {
            project_id: "p-1".into(),
            id: None,
            draft: draft(),
        }
    );
    add!(
        "DesignerDraftListResponse",
        DesignerDraftListResponse {
            drafts: vec![DraftEntryDto {
                id: "d-1".into(),
                def_name: "RS_TestRifle".into(),
                label: "test rifle".into(),
                kind: ItemKindDto::Ranged,
                updated_at_ms: 1_700_000_000_000,
                draft: draft(),
            }]
        }
    );
    add!(
        "ReferenceListDto",
        ReferenceListDto {
            kind: ItemKindDto::Ranged,
            class_label: "rifle".into(),
            total: 14,
            offset: 0,
            items: vec![ReferenceItemDto {
                index: 0,
                def_name: "RS_AnchorGun".into(),
                label: "anchor gun".into(),
                tier: Some(TechLevelDto::Spacer),
                role: None,
                mod_id: None,
                strength: Some(4.5),
                stats: BTreeMap::from([("damage".to_owned(), 10.0)]),
            }],
            pools: vec![StatPoolDto {
                stat: "damage".into(),
                n: 14,
                min: 4.0,
                p10: 6.0,
                median: 10.0,
                p90: 15.0,
                max: 20.0,
            }],
        }
    );
    add!("PreviewDto", preview());
    add!("MaterialMatrixDto", matrix());
    add!("FitReportDto", fit_report());
    add!("QuizStepDto", quiz_step());
    add!(
        "DesignerQuizAnswerResponse",
        DesignerQuizAnswerResponse {
            draft: draft(),
            step: quiz_step(),
        }
    );
    add!(
        "DesignerQuizBackRequest",
        DesignerQuizBackRequest { draft: draft() }
    );
    add!("CalibrateResultDto", calibrate_result());
    add!("ConvertScanDto", convert_scan());
    add!("CeSuggestionDto", ce_suggestion());
    add!(
        "DesignerCeSuggestRequest",
        DesignerCeSuggestRequest { draft: draft() }
    );
    add!("DesignerExportPlanRequest", export_request());
    add!("WritePlanDto", write_plan());
    add!(
        "DesignerApplyPlanRequest",
        DesignerApplyPlanRequest {
            plan_id: "b3-0123456789abcdef".into(),
            request: export_request(),
            backup: true,
            dry_apply: false,
        }
    );
    add!("ApplyReportDto", apply_report());
    add!("ProjectSummaryDto", project_summary());
    add!("ProjectTreeDto", project_tree());
    add!("ProjectLayoutCheckDto", layout_check());
    add!("ProjectFileDto", project_file());
    add!(
        "ProjectTreeRequest",
        ProjectTreeRequest {
            project_id: "p-1a2b3c4d".into(),
            max_nodes: Some(500),
        }
    );
    add!(
        "ProjectScaffoldMissingRequest",
        ProjectScaffoldMissingRequest {
            project_id: "p-1a2b3c4d".into(),
            dry_run: true,
        }
    );
    add!(
        "ProjectScaffoldMissingDto",
        ProjectScaffoldMissingDto {
            project_id: "p-1a2b3c4d".into(),
            dry_run: false,
            folders: vec!["Sounds".into(), "Textures".into()],
            files: Vec::new(),
            skipped: Vec::new(),
        }
    );
    add!(
        "ProjectReadFileRequest",
        ProjectReadFileRequest {
            project_id: "p-1a2b3c4d".into(),
            path: "About/About.xml".into(),
            max_bytes: None,
        }
    );
    for (i, prompt) in quiz_prompts().into_iter().enumerate() {
        let _ = i;
        out.push(("PromptDto", round_trip(&prompt)));
    }
    for answer in quiz_answers() {
        out.push(("QuizAnswerDto", round_trip(&answer)));
    }
    out
}

/// Serialises, parses back, checks equality of the re-serialised text and returns the JSON value.
pub fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> Value {
    let text = serde_json::to_string(value).unwrap_or_default();
    assert!(!text.is_empty(), "value must serialise");
    let back: T = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => panic!("round trip failed: {e}: {text}"),
    };
    let again = serde_json::to_string(&back).unwrap_or_default();
    assert_eq!(text, again, "re-serialised text differs");
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

pub fn mod_rows(n: usize) -> ModsSnapshot {
    let rows = (0..n)
        .map(|i| ModRowDto {
            idx: u32::try_from(i).unwrap_or(0),
            id: format!("w{}", 2_000_000_000u64 + i as u64),
            name: format!("RS_Fictional Mod Number {i} With A Moderately Long Display Name"),
            authors: "RS_AuthorOne, RS_AuthorTwo".into(),
            flags: flags::ACTIVE | flags::WORKSHOP | flags::VERSION_OK,
            versions: 0b11_1100,
            load_index: Some(u32::try_from(i).unwrap_or(0)),
            errors: 1,
            warnings: 12,
            size_kib: 1_234_567,
            updated_s: 1_700_000_000,
        })
        .collect();
    ModsSnapshot {
        rev: 1,
        game_version: Some("9.9.1".into()),
        rows,
    }
}
