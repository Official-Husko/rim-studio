//! A fictional install on a real temporary disk and a way to run the built binary against it with a
//! temporary HOME and XDG folders. Every name starts with `RS_` and every number is invented; nothing
//! here comes from a game install.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder, TempInstall};
use serde_json::Value;

/// A small fictional def type table (the format of `rimstudio-defs`).
pub const TYPE_TABLE: &str = r#"{
  "format": 1,
  "root": "Verse.Def",
  "assemblies": [],
  "types": {
    "Verse.Def": {"base": null},
    "Verse.ThingDef": {"base": "Verse.Def"},
    "Verse.StatDef": {"base": "Verse.Def"},
    "Verse.DamageDef": {"base": "Verse.Def"},
    "CombatExtended.AmmoSetDef": {"base": "Verse.Def"}
  },
  "short_name_collisions": {}
}"#;

pub const CE_ID: &str = "ceteam.combatextended";

fn stat_def(name: &str) -> Node {
    NodeBuilder::new("StatDef")
        .text_elem("defName", name)
        .text_elem("defaultBaseValue", "0")
        .build()
}

fn projectile(name: &str, damage: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("projectile", |p| {
            p.text_elem("damageDef", "RS_Damage")
                .text_elem("damageAmountBase", damage.to_string())
                .text_elem("speed", "55")
        })
        .build()
}

fn ingredient(name: &str, value: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("category", "Item")
        .elem("statBases", |s| {
            s.text_elem("MarketValue", value.to_string())
        })
        .build()
}

fn gun(name: &str, projectile: &str, i: u32, tier: &str, tag: &str) -> Node {
    let f = f64::from(i);
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseGun")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", (1.5 + 0.3 * f).to_string())
                .text_elem("RangedWeapon_Cooldown", (1.0 + 0.1 * f).to_string())
                .text_elem("AccuracyTouch", "0.7")
                .text_elem("AccuracyShort", (0.8 - 0.01 * f).to_string())
                .text_elem("AccuracyMedium", (0.65 - 0.01 * f).to_string())
                .text_elem("AccuracyLong", (0.5 - 0.01 * f).to_string())
                .text_elem("WorkToMake", (6000.0 + 700.0 * f).to_string())
        })
        .elem("costList", |c| c.text_elem("RS_Steel", "30"))
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", projectile)
                    .text_elem("warmupTime", (0.8 + 0.07 * f).to_string())
                    .text_elem("range", (18.0 + 1.5 * f).to_string())
            })
        })
        .elem("weaponTags", |w| w.li(tag))
        .build()
}

fn melee(name: &str, i: u32, tier: &str, cap: &str) -> Node {
    let f = f64::from(i);
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_BaseMelee")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", (0.8 + 0.2 * f).to_string())
                .text_elem("WorkToMake", (3000.0 + 400.0 * f).to_string())
        })
        .elem("weaponTags", |w| w.li("RS_Melee"))
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "head")
                    .elem("capacities", |c| c.li(cap))
                    .text_elem("power", (7.0 + 1.1 * f).to_string())
                    .text_elem("cooldownTime", (1.8 + 0.1 * f).to_string())
            })
        })
        .build()
}

fn abstract_base(name: &str) -> Node {
    NodeBuilder::new("ThingDef")
        .attr("Name", name)
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .build()
}

fn core_defs() -> Vec<Node> {
    let mut defs = vec![
        stat_def("Mass"),
        stat_def("RangedWeapon_Cooldown"),
        NodeBuilder::new("DamageDef")
            .text_elem("defName", "RS_Damage")
            .text_elem("defaultDamage", "10")
            .text_elem("defaultArmorPenetration", "0.2")
            .build(),
        ingredient("RS_Steel", 2.0),
        ingredient("RS_Part", 40.0),
    ];
    for i in 0..14_u32 {
        defs.push(projectile(&format!("RS_Shot{i:02}"), 8.0 + f64::from(i)));
        let tag = if i % 3 == 2 { "RS_Pistol" } else { "RS_Rifle" };
        let tier = if i < 8 { "Industrial" } else { "Spacer" };
        defs.push(gun(
            &format!("RS_Gun{i:02}"),
            &format!("RS_Shot{i:02}"),
            i,
            tier,
            tag,
        ));
    }
    for i in 0..8_u32 {
        let cap = if i % 2 == 0 { "Cut" } else { "Blunt" };
        let tier = if i < 4 { "Medieval" } else { "Industrial" };
        defs.push(melee(&format!("RS_Blade{i:02}"), i, tier, cap));
    }
    for base in ["RS_BaseGun", "RS_BaseMelee", "RS_BaseBullet"] {
        defs.push(abstract_base(base));
    }
    defs
}

fn ce_mod() -> ModFolder {
    let ammo_set = NodeBuilder::new("CombatExtended.AmmoSetDef")
        .text_elem("defName", "RS_AmmoSetA")
        .elem("ammoTypes", |t| t.text_elem("RS_AmmoA1", "RS_CeBullet1"))
        .build();
    let bullet = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_CeBullet1")
        .elem("projectile", |p| {
            p.text_elem("damageAmountBase", "9")
                .text_elem("armorPenetrationSharp", "2.5")
                .text_elem("armorPenetrationBlunt", "14")
                .text_elem("speed", "120")
        })
        .build();
    ModFolder::new("RS_CE", CE_ID)
        .name("Combat Extended")
        .defs_file("RS_Ce.xml", vec![ammo_set, bullet])
}

/// The text of an `About.xml`.
pub fn about(id: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ModMetaData>\n  <name>RS Test Mod</name>\n  <packageId>{id}</packageId>\n  <author>RS Author</author>\n  <supportedVersions>\n    <li>1.6</li>\n  </supportedVersions>\n  <description>fictional</description>\n</ModMetaData>\n"
    )
}

/// A gun definition of a mod that has not been converted.
pub const PROJECT_DEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Defs>
  <ThingDef>
    <defName>RS_ProjGun</defName>
    <label>proj gun</label>
    <category>Item</category>
    <techLevel>Industrial</techLevel>
    <statBases>
      <Mass>3</Mass>
      <RangedWeapon_Cooldown>1.5</RangedWeapon_Cooldown>
      <AccuracyTouch>0.7</AccuracyTouch>
      <AccuracyShort>0.7</AccuracyShort>
      <AccuracyMedium>0.6</AccuracyMedium>
      <AccuracyLong>0.5</AccuracyLong>
    </statBases>
    <verbs>
      <li>
        <verbClass>Verb_Shoot</verbClass>
        <defaultProjectile>RS_Shot00</defaultProjectile>
        <warmupTime>1</warmupTime>
        <range>25</range>
      </li>
    </verbs>
    <weaponTags><li>RS_Rifle</li></weaponTags>
  </ThingDef>
</Defs>
"#;

/// The result of one run of the binary.
pub struct Run {
    pub output: Output,
}

impl Run {
    pub fn code(&self) -> i32 {
        self.output.status.code().unwrap_or(-1)
    }

    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    /// The standard output as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.output.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not JSON ({e}): {}\nstderr: {}",
                self.stdout(),
                self.stderr()
            )
        })
    }

    /// Fails with the whole output unless the exit code is `want`.
    pub fn expect(self, want: i32) -> Self {
        assert_eq!(
            self.code(),
            want,
            "stdout:\n{}\nstderr:\n{}",
            self.stdout(),
            self.stderr()
        );
        self
    }
}

/// A temporary home, a fictional install and a place for projects.
pub struct Env {
    pub tmp: tempfile::TempDir,
    pub install: TempInstall,
    pub home: PathBuf,
    pub type_table: PathBuf,
}

impl Env {
    /// With the fictional install, and the fictional Combat Extended mod when asked.
    pub fn new(with_ce: bool) -> Self {
        let mut b = InstallBuilder::new().core_defs_file("RS_Core.xml", core_defs());
        if with_ce {
            b = b.mod_folder(ce_mod());
        }
        let install = b.build_temp().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let type_table = tmp.path().join("type-table.json");
        std::fs::write(&type_table, TYPE_TABLE).unwrap();
        Self {
            tmp,
            install,
            home,
            type_table,
        }
    }

    pub fn game_dir(&self) -> String {
        self.install.game_dir.as_str().to_owned()
    }

    /// A path under the temporary folder.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.tmp.path().join(rel)
    }

    /// Runs the binary with this environment.
    pub fn run(&self, args: &[&str]) -> Run {
        let mut cmd = self.command();
        cmd.args(args);
        Run {
            output: cmd.output().unwrap(),
        }
    }

    /// The binary with this environment and no arguments yet.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rimstudio-cli"));
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("RIMSTUDIO_TYPE_TABLE", &self.type_table)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_STATE_HOME", self.home.join(".local/state"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .current_dir(self.tmp.path());
        cmd
    }

    /// Points the application at the fictional install and scans it.
    pub fn select_install(&self) {
        let game = self.game_dir();
        self.run(&["detect", "--install", &game]).expect(0);
        self.run(&["scan"]);
    }

    /// A mod folder with an About file and the given extra files; returns its path.
    pub fn mod_folder(&self, name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = self.path("projects").join(name);
        std::fs::create_dir_all(root.join("About")).unwrap();
        std::fs::write(root.join("About/About.xml"), about("rs.testmod")).unwrap();
        for (rel, text) in files {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }
}

/// All files below a folder, relative, sorted.
pub fn files_under(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}
