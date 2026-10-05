//! Where the files of a project go: the file layout rule for planned files.

use serde::{Deserialize, Serialize};

use rimstudio_core::diag::Diagnostic;

use crate::validation::codes;

use super::builder::is_safe_relative_path;

/// The project conventions that decide a planned file's path. All paths are relative to the project root
/// with `/` separators. The defaults follow the layout of the specification
/// (`Defs/Weapons/<Name>.xml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayout {
    /// The version folder (for example `1.6`) when the project keeps one folder per game version. Every
    /// planned file then lives inside it, except `LoadFolders.xml` and `About`, which stay at the root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_folder: Option<String>,
    /// The folder of definitions, `Defs` by default.
    pub defs_dir: String,
    /// The subfolder of weapon definitions, `Weapons` by default.
    pub weapons_dir: String,
    /// The subfolder of projectile definitions, `Projectiles` by default.
    pub projectiles_dir: String,
    /// The folder selected by `LoadFolders.xml` for Combat Extended, `CE` by default. Used only by the CE
    /// patch generator.
    pub ce_folder: String,
    /// A short slug of the mod used in CE patch file names (`<slug>_Weapons_Ranged.xml`). Empty omits it.
    pub mod_slug: String,
}

impl Default for ProjectLayout {
    fn default() -> Self {
        Self {
            version_folder: None,
            defs_dir: "Defs".into(),
            weapons_dir: "Weapons".into(),
            projectiles_dir: "Projectiles".into(),
            ce_folder: "CE".into(),
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

    /// Prefixes a path with the version folder, when there is one.
    #[must_use]
    pub fn in_version(&self, relative: &str) -> String {
        match &self.version_folder {
            Some(v) => format!("{v}/{relative}"),
            None => relative.to_owned(),
        }
    }

    /// The path of a weapon definition file: `[<version>/]Defs/Weapons/<defName>.xml`.
    #[must_use]
    pub fn weapon_def_path(&self, def_name: &str) -> String {
        self.in_version(&format!(
            "{}/{}/{}.xml",
            self.defs_dir,
            self.weapons_dir,
            file_stem(def_name)
        ))
    }

    /// The path of a projectile definition file: `[<version>/]Defs/Projectiles/<defName>.xml`.
    #[must_use]
    pub fn projectile_def_path(&self, def_name: &str) -> String {
        self.in_version(&format!(
            "{}/{}/{}.xml",
            self.defs_dir,
            self.projectiles_dir,
            file_stem(def_name)
        ))
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
        self.in_version(&format!("{}/Patches/{name}", self.ce_folder))
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
            ("/layout/weaponsDir", &self.weapons_dir),
            ("/layout/projectilesDir", &self.projectiles_dir),
            ("/layout/ceFolder", &self.ce_folder),
        ];
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
            l.weapon_def_path("RS_TestRifle"),
            "Defs/Weapons/RS_TestRifle.xml"
        );
        assert_eq!(
            l.projectile_def_path("RS_Bullet"),
            "Defs/Projectiles/RS_Bullet.xml"
        );
        assert_eq!(
            l.ce_patch_path("Weapons_Ranged"),
            "CE/Patches/Weapons_Ranged.xml"
        );
        assert_eq!(l.load_folders_path(), "LoadFolders.xml");
    }

    #[test]
    fn version_folder_prefixes_content_but_not_load_folders() {
        let mut l = ProjectLayout::with_version_folder("1.6");
        l.mod_slug = "RS_Mod".into();
        assert_eq!(l.weapon_def_path("RS_X"), "1.6/Defs/Weapons/RS_X.xml");
        assert_eq!(
            l.ce_patch_path("Weapons_Melee"),
            "1.6/CE/Patches/RS_Mod_Weapons_Melee.xml"
        );
        assert_eq!(l.load_folders_path(), "LoadFolders.xml");
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
    }
}
