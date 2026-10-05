//! Where the files of a project go: the file layout rule (RimStudio mod layout v1).
//!
//! The rule follows the names of the game's own data: weapon definitions live under
//! `Defs/ThingDefs_Misc/Weapons`, sound definitions under `Defs/SoundDefs`, textures under
//! `Textures/Things/Item/Equipment/WeaponRanged` and its siblings, and the optional Combat Extended
//! content under `Compat/CombatExtended` (gated in `LoadFolders.xml`). How a weapon file is named depends on
//! the convention of the project ([`LayoutProfile`]): a project made by RimStudio keeps one file per weapon
//! (holding the weapon and its own projectile) in a folder named like the game's category file
//! ([`WeaponCategory`]); a project that follows the game's own style keeps one file per category and a new
//! weapon is appended to it; a flat project keeps its weapons in one folder, one file per weapon.
//! The specification is `docs/features/mod-layout.md`.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::paths;
use serde::{Deserialize, Serialize};

use crate::model::{ItemKind, TechLevel};
use crate::validation::codes;

use super::builder::is_safe_relative_path;

/// The convention a project follows for weapon files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutProfile {
    /// One file per weapon in a category folder: `Defs/ThingDefs_Misc/Weapons/<Category>/<DefName>.xml`.
    /// The default of a new project.
    #[default]
    Rimstudio,
    /// The game's own style: one file per category, `Defs/ThingDefs_Misc/Weapons/<Category>.xml`. A new
    /// weapon is appended to the matching file as a marked section.
    CoreStyle,
    /// One folder of weapon files, one file per weapon: `<folder>/<DefName>.xml`. The folder is the one the
    /// project already uses.
    Flat,
}

impl LayoutProfile {
    /// The word used in documents and on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rimstudio => "rimstudio",
            Self::CoreStyle => "core-style",
            Self::Flat => "flat",
        }
    }
}

/// The category of a weapon: the names of the game's own category files in `Weapons`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WeaponCategory {
    /// Bows, slings and primitive guns.
    RangedNeolithic,
    /// Guns of the industrial age.
    RangedIndustrial,
    /// Spacer tier guns.
    RangedSpacer,
    /// Special and archotech guns.
    RangedSpecial,
    /// Weapons of mechanoids (recognised, never chosen for a design).
    RangedMechanoid,
    /// Clubs, spears and other primitive melee weapons.
    MeleeNeolithic,
    /// Blades and maces of the medieval and industrial age.
    MeleeMedieval,
    /// Spacer and archotech melee weapons.
    MeleeUltratech,
}

impl WeaponCategory {
    /// Every category.
    pub const ALL: [WeaponCategory; 8] = [
        Self::RangedNeolithic,
        Self::RangedIndustrial,
        Self::RangedSpacer,
        Self::RangedSpecial,
        Self::RangedMechanoid,
        Self::MeleeNeolithic,
        Self::MeleeMedieval,
        Self::MeleeUltratech,
    ];

    /// The name used for the folder or the file stem.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::RangedNeolithic => "RangedNeolithic",
            Self::RangedIndustrial => "RangedIndustrial",
            Self::RangedSpacer => "RangedSpacer",
            Self::RangedSpecial => "RangedSpecial",
            Self::RangedMechanoid => "RangedMechanoid",
            Self::MeleeNeolithic => "MeleeNeolithic",
            Self::MeleeMedieval => "MeleeMedieval",
            Self::MeleeUltratech => "MeleeUltratech",
        }
    }

    /// The category named `name` (a folder name or a file stem), compared without regard to letter case.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(name))
    }

    /// True for the ranged categories.
    #[must_use]
    pub fn is_ranged(self) -> bool {
        matches!(
            self,
            Self::RangedNeolithic
                | Self::RangedIndustrial
                | Self::RangedSpacer
                | Self::RangedSpecial
                | Self::RangedMechanoid
        )
    }

    /// The category of a design: the kind and the tech level decide. Ranged: Neolithic and Medieval go to
    /// `RangedNeolithic`, Industrial to `RangedIndustrial`, Spacer and Ultra to `RangedSpacer`, Archotech to
    /// `RangedSpecial`. Melee: Neolithic to `MeleeNeolithic`, Medieval and Industrial to `MeleeMedieval`,
    /// Spacer, Ultra and Archotech to `MeleeUltratech`. An absent level counts as Industrial for ranged
    /// weapons and Medieval for melee weapons (the validator asks for the level, so this is only a fallback).
    #[must_use]
    pub fn of(kind: ItemKind, tier: Option<TechLevel>) -> Self {
        match kind {
            ItemKind::Melee => match tier.unwrap_or(TechLevel::Medieval) {
                TechLevel::Neolithic => Self::MeleeNeolithic,
                TechLevel::Medieval | TechLevel::Industrial => Self::MeleeMedieval,
                _ => Self::MeleeUltratech,
            },
            _ => match tier.unwrap_or(TechLevel::Industrial) {
                TechLevel::Neolithic | TechLevel::Medieval => Self::RangedNeolithic,
                TechLevel::Industrial => Self::RangedIndustrial,
                TechLevel::Spacer | TechLevel::Ultra => Self::RangedSpacer,
                TechLevel::Archotech => Self::RangedSpecial,
            },
        }
    }
}

/// The sound definition files of weapons, named like the game's own sound files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoundFile {
    /// The report of a shot or a swing.
    Shot,
    /// Reloading and handling.
    Reload,
    /// Projectile impacts.
    Impact,
}

impl SoundFile {
    /// The file stem inside `Defs/SoundDefs`.
    #[must_use]
    pub fn stem(self) -> &'static str {
        match self {
            Self::Shot => "World_Oneshots_Weapons",
            Self::Reload => "Reload_Oneshots_Weapons",
            Self::Impact => "World_Oneshots_ProjectileImpacts",
        }
    }

    /// The suffix of the clip folder of a weapon (`<DefName>_Shot`).
    #[must_use]
    pub fn clip_suffix(self) -> &'static str {
        match self {
            Self::Shot => "Shot",
            Self::Reload => "Reload",
            Self::Impact => "Impact",
        }
    }
}

/// The project conventions that decide a planned file's path. All paths are relative to the project root
/// with `/` separators. The defaults are the layout v1 of a new project
/// (`Defs/ThingDefs_Misc/Weapons/<Category>/<DefName>.xml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayout {
    /// The folder that holds the content when it is not the project root: a version folder such as `1.6`,
    /// or `Common`. Every planned file then lives inside it, except `LoadFolders.xml`, `About` and `Source`,
    /// which stay at the root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_folder: Option<String>,
    /// The convention for weapon files.
    pub profile: LayoutProfile,
    /// The folder of definitions, `Defs` by default.
    pub defs_dir: String,
    /// The folder of weapon definitions below the definitions folder, `ThingDefs_Misc/Weapons` by default.
    /// For a flat project this is the folder the project already uses (it may be empty).
    pub weapons_dir: String,
    /// The folder selected by `LoadFolders.xml` for Combat Extended, `Compat/CombatExtended` by default. A
    /// project that already has another folder keeps it (for example `CE`); it is never moved.
    pub ce_folder: String,
    /// A short slug of the mod used in CE patch file names (`<slug>_Weapons_Ranged.xml`). Empty omits it.
    pub mod_slug: String,
}

impl Default for ProjectLayout {
    fn default() -> Self {
        Self {
            version_folder: None,
            profile: LayoutProfile::Rimstudio,
            defs_dir: paths::DEFS_DIR.into(),
            weapons_dir: paths::DEFS_WEAPONS_DIR.into(),
            ce_folder: paths::CE_COMPAT_DIR.into(),
            mod_slug: String::new(),
        }
    }
}

/// Turns a def name into a file stem: characters other than letters, digits, `_` and `-` become `_`; an
/// empty result is `Unnamed`.
#[must_use]
pub fn file_stem(def_name: &str) -> String {
    let stem: String = def_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "Unnamed".into()
    } else {
        stem
    }
}

impl ProjectLayout {
    /// A layout that keeps everything under a version folder.
    #[must_use]
    pub fn with_version_folder(version: impl Into<String>) -> Self {
        Self {
            version_folder: Some(version.into()),
            ..Self::default()
        }
    }

    /// The same layout with another profile (and the weapon folder that goes with it by default).
    #[must_use]
    pub fn with_profile(mut self, profile: LayoutProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Prefixes a path with the version folder, when there is one.
    #[must_use]
    pub fn in_version(&self, relative: &str) -> String {
        match &self.version_folder {
            Some(v) => format!("{v}/{relative}"),
            None => relative.to_owned(),
        }
    }

    /// The folder of weapon definitions, relative to the project root: `[<version>/]Defs/<weapons dir>`.
    #[must_use]
    pub fn weapons_folder(&self) -> String {
        if self.weapons_dir.is_empty() {
            self.in_version(&self.defs_dir)
        } else {
            self.in_version(&format!("{}/{}", self.defs_dir, self.weapons_dir))
        }
    }

    /// The path of the file that holds a weapon (and its own projectile) of `category`, by profile:
    /// [`LayoutProfile::Rimstudio`] `<weapons>/<Category>/<DefName>.xml`, [`LayoutProfile::CoreStyle`]
    /// `<weapons>/<Category>.xml` (shared by every weapon of the category), [`LayoutProfile::Flat`]
    /// `<weapons>/<DefName>.xml`.
    #[must_use]
    pub fn weapon_def_path(&self, category: WeaponCategory, def_name: &str) -> String {
        let folder = self.weapons_folder();
        match self.profile {
            LayoutProfile::Rimstudio => {
                format!("{folder}/{}/{}.xml", category.name(), file_stem(def_name))
            }
            LayoutProfile::CoreStyle => format!("{folder}/{}.xml", category.name()),
            LayoutProfile::Flat => format!("{folder}/{}.xml", file_stem(def_name)),
        }
    }

    /// True when every weapon of a category shares one file (an appended section is the normal case).
    #[must_use]
    pub fn shares_category_files(&self) -> bool {
        self.profile == LayoutProfile::CoreStyle
    }

    /// The path of a sound definition file: `[<version>/]Defs/SoundDefs/<stem>.xml`.
    #[must_use]
    pub fn sound_def_path(&self, file: SoundFile) -> String {
        self.in_version(&format!(
            "{}/{}/{}.xml",
            self.defs_dir,
            paths::DEFS_SOUNDS_DIR,
            file.stem()
        ))
    }

    /// The `clipFolderPath` a sound definition of a weapon uses (relative to `Sounds`):
    /// `Weapons/<DefName>_<Shot|Reload|Impact>`.
    #[must_use]
    pub fn sound_clip_folder_path(&self, def_name: &str, file: SoundFile) -> String {
        format!(
            "{}/{}_{}",
            paths::SOUNDS_WEAPONS_DIR,
            file_stem(def_name),
            file.clip_suffix()
        )
    }

    /// The folder (relative to the project root) that holds the clips of
    /// [`Self::sound_clip_folder_path`]: `[<version>/]Sounds/Weapons/<DefName>_<...>`.
    #[must_use]
    pub fn sound_clip_dir(&self, def_name: &str, file: SoundFile) -> String {
        self.in_version(&format!(
            "{}/{}",
            paths::SOUNDS_DIR,
            self.sound_clip_folder_path(def_name, file)
        ))
    }

    /// The `texPath` reserved for the art of a weapon, mirroring the game's own convention:
    /// `Things/Item/Equipment/WeaponRanged/<DefName>` or `.../WeaponMelee/<DefName>`.
    #[must_use]
    pub fn weapon_texture_path(&self, kind: ItemKind, def_name: &str) -> String {
        let dir = match kind {
            ItemKind::Melee => paths::TEXTURES_WEAPON_MELEE_DIR,
            _ => paths::TEXTURES_WEAPON_RANGED_DIR,
        };
        format!("{dir}/{}", file_stem(def_name))
    }

    /// The `texPath` reserved for the art of a projectile: `Things/Projectile/<DefName>`.
    #[must_use]
    pub fn projectile_texture_path(&self, def_name: &str) -> String {
        format!("{}/{}", paths::TEXTURES_PROJECTILE_DIR, file_stem(def_name))
    }

    /// The file where the art of a `texPath` goes: `[<version>/]Textures/<texPath>.png`. The game also
    /// accepts other image formats; the layout reserves the PNG name.
    #[must_use]
    pub fn texture_file(&self, tex_path: &str) -> String {
        self.in_version(&format!("{}/{tex_path}.png", paths::TEXTURES_DIR))
    }

    /// The folder for art sources the game never reads: `Source/Art`, always in the project root.
    #[must_use]
    pub fn art_source_dir(&self) -> String {
        paths::SOURCE_ART_DIR.into()
    }

    /// The folder (relative to the project root) of the gated Combat Extended content.
    #[must_use]
    pub fn ce_dir(&self) -> String {
        self.in_version(&self.ce_folder)
    }

    /// True when the Combat Extended folder is not the standard `Compat/CombatExtended` (for example a
    /// project made before layout v1 with a `CE` folder). Such a folder is kept as it is.
    #[must_use]
    pub fn has_legacy_ce_folder(&self) -> bool {
        self.ce_folder != paths::CE_COMPAT_DIR
    }

    /// The path of a CE patch file of a category (`Weapons_Ranged`, `Weapons_Melee`, `Apparel`):
    /// `[<version>/]<ce folder>/Patches/[<slug>_]<category>.xml`.
    #[must_use]
    pub fn ce_patch_path(&self, category: &str) -> String {
        let name = if self.mod_slug.is_empty() {
            format!("{category}.xml")
        } else {
            format!("{}_{category}.xml", file_stem(&self.mod_slug))
        };
        self.in_version(&format!("{}/{}/{name}", self.ce_folder, paths::PATCHES_DIR))
    }

    /// The path of `LoadFolders.xml`, always at the project root.
    #[must_use]
    pub fn load_folders_path(&self) -> String {
        "LoadFolders.xml".into()
    }

    /// Checks that every folder name is a safe relative path; a bad layout would escape the project root.
    #[must_use]
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut parts: Vec<(&str, &str)> = vec![
            ("/layout/defsDir", &self.defs_dir),
            ("/layout/ceFolder", &self.ce_folder),
        ];
        if !self.weapons_dir.is_empty() {
            parts.push(("/layout/weaponsDir", &self.weapons_dir));
        }
        if let Some(v) = &self.version_folder {
            parts.push(("/layout/versionFolder", v));
        }
        parts
            .into_iter()
            .filter(|(_, v)| !is_safe_relative_path(v))
            .map(|(field, v)| codes::PLAN_PATH_INVALID.diagnostic(field, &[("path", v)]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn default_paths_follow_the_specification() {
        let l = ProjectLayout::default();
        assert_eq!(
            l.weapon_def_path(WeaponCategory::RangedIndustrial, "RS_TestRifle"),
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_TestRifle.xml"
        );
        assert_eq!(
            l.ce_patch_path("Weapons_Ranged"),
            "Compat/CombatExtended/Patches/Weapons_Ranged.xml"
        );
        assert_eq!(l.ce_dir(), "Compat/CombatExtended");
        assert!(!l.has_legacy_ce_folder());
        assert_eq!(l.load_folders_path(), "LoadFolders.xml");
    }

    #[test]
    fn version_folder_prefixes_content_but_not_load_folders() {
        let mut l = ProjectLayout::with_version_folder("1.6");
        l.mod_slug = "RS_Mod".into();
        assert_eq!(
            l.weapon_def_path(WeaponCategory::MeleeMedieval, "RS_X"),
            "1.6/Defs/ThingDefs_Misc/Weapons/MeleeMedieval/RS_X.xml"
        );
        assert_eq!(
            l.ce_patch_path("Weapons_Melee"),
            "1.6/Compat/CombatExtended/Patches/RS_Mod_Weapons_Melee.xml"
        );
        assert_eq!(l.load_folders_path(), "LoadFolders.xml");
        assert_eq!(l.art_source_dir(), "Source/Art");
    }

    #[test]
    fn the_profile_decides_the_weapon_file() {
        let cat = WeaponCategory::RangedIndustrial;
        let core = ProjectLayout::default().with_profile(LayoutProfile::CoreStyle);
        assert_eq!(
            core.weapon_def_path(cat, "OH_G41m"),
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml"
        );
        assert!(core.shares_category_files());
        let flat = ProjectLayout {
            profile: LayoutProfile::Flat,
            weapons_dir: "ThingsDef_Misc/Weapons".into(),
            version_folder: Some("Common".into()),
            ..ProjectLayout::default()
        };
        assert_eq!(
            flat.weapon_def_path(cat, "TL_Rifle"),
            "Common/Defs/ThingsDef_Misc/Weapons/TL_Rifle.xml"
        );
        let bare = ProjectLayout {
            profile: LayoutProfile::Flat,
            weapons_dir: String::new(),
            ..ProjectLayout::default()
        };
        assert_eq!(bare.weapon_def_path(cat, "A"), "Defs/A.xml");
    }

    #[rstest]
    #[case(ItemKind::Ranged, Some(TechLevel::Neolithic), "RangedNeolithic")]
    #[case(ItemKind::Ranged, Some(TechLevel::Medieval), "RangedNeolithic")]
    #[case(ItemKind::Ranged, Some(TechLevel::Industrial), "RangedIndustrial")]
    #[case(ItemKind::Ranged, Some(TechLevel::Spacer), "RangedSpacer")]
    #[case(ItemKind::Ranged, Some(TechLevel::Ultra), "RangedSpacer")]
    #[case(ItemKind::Ranged, Some(TechLevel::Archotech), "RangedSpecial")]
    #[case(ItemKind::Ranged, None, "RangedIndustrial")]
    #[case(ItemKind::Melee, Some(TechLevel::Neolithic), "MeleeNeolithic")]
    #[case(ItemKind::Melee, Some(TechLevel::Medieval), "MeleeMedieval")]
    #[case(ItemKind::Melee, Some(TechLevel::Industrial), "MeleeMedieval")]
    #[case(ItemKind::Melee, Some(TechLevel::Spacer), "MeleeUltratech")]
    #[case(ItemKind::Melee, Some(TechLevel::Archotech), "MeleeUltratech")]
    #[case(ItemKind::Melee, None, "MeleeMedieval")]
    fn categories_follow_kind_and_tier(
        #[case] kind: ItemKind,
        #[case] tier: Option<TechLevel>,
        #[case] name: &str,
    ) {
        let category = WeaponCategory::of(kind, tier);
        assert_eq!(category.name(), name);
        assert_eq!(category.is_ranged(), kind == ItemKind::Ranged);
    }

    #[test]
    fn category_names_parse_without_regard_to_case() {
        for c in WeaponCategory::ALL {
            assert_eq!(WeaponCategory::parse(c.name()), Some(c));
            assert_eq!(WeaponCategory::parse(&c.name().to_lowercase()), Some(c));
        }
        assert_eq!(WeaponCategory::parse("Weapons_Ranged"), None);
        assert_eq!(WeaponCategory::parse(""), None);
    }

    #[test]
    fn sounds_textures_and_art_have_reserved_places() {
        let l = ProjectLayout::with_version_folder("1.6");
        assert_eq!(
            l.sound_def_path(SoundFile::Shot),
            "1.6/Defs/SoundDefs/World_Oneshots_Weapons.xml"
        );
        assert_eq!(
            l.sound_clip_folder_path("RS_Rifle", SoundFile::Shot),
            "Weapons/RS_Rifle_Shot"
        );
        assert_eq!(
            l.sound_clip_dir("RS_Rifle", SoundFile::Reload),
            "1.6/Sounds/Weapons/RS_Rifle_Reload"
        );
        let tex = l.weapon_texture_path(ItemKind::Ranged, "RS_Rifle");
        assert_eq!(tex, "Things/Item/Equipment/WeaponRanged/RS_Rifle");
        assert_eq!(
            l.texture_file(&tex),
            "1.6/Textures/Things/Item/Equipment/WeaponRanged/RS_Rifle.png"
        );
        assert_eq!(
            l.weapon_texture_path(ItemKind::Melee, "RS_Blade"),
            "Things/Item/Equipment/WeaponMelee/RS_Blade"
        );
        assert_eq!(
            l.projectile_texture_path("RS_Bullet"),
            "Things/Projectile/RS_Bullet"
        );
    }

    #[test]
    fn a_legacy_ce_folder_is_reported_and_used() {
        let l = ProjectLayout {
            ce_folder: "CE".into(),
            ..ProjectLayout::default()
        };
        assert!(l.has_legacy_ce_folder());
        assert_eq!(l.ce_patch_path("Apparel"), "CE/Patches/Apparel.xml");
    }

    #[rstest]
    #[case("RS_Test", "RS_Test")]
    #[case("a/b", "a_b")]
    #[case("..", "__")]
    #[case("a b.c", "a_b_c")]
    #[case("", "Unnamed")]
    fn stems_are_file_safe(#[case] def_name: &str, #[case] stem: &str) {
        assert_eq!(file_stem(def_name), stem);
    }

    #[test]
    fn bad_layouts_are_reported() {
        assert!(ProjectLayout::default().validate().is_empty());
        let l = ProjectLayout {
            defs_dir: "../Defs".into(),
            version_folder: Some("/abs".into()),
            ..ProjectLayout::default()
        };
        assert_eq!(l.validate().len(), 2);
        let l = ProjectLayout {
            weapons_dir: "a/../../b".into(),
            ..ProjectLayout::default()
        };
        assert_eq!(l.validate().len(), 1);
    }
}
