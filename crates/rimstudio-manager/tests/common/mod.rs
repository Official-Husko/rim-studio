//! Shared fixtures: data roots in a temporary folder, a fictional install on disk and a detection
//! environment made of fakes over the real file system.

#![allow(dead_code, unreachable_pub, clippy::unwrap_used, clippy::expect_used)]

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{Clock, DetectEnv, EnvProbe, FsProbe, RegistryProbe};
use rimstudio_core::tree::NodeBuilder;
use rimstudio_io::real_fs::RealFs;
use rimstudio_io::roots::DataRoots;
use rimstudio_manager::Ctx;
use rimstudio_manager::detect::{
    DetectRunResponse, DetectSetOverrideRequest, OverrideField, set_override,
};
use rimstudio_testing::fakes::{FakeClock, FakeEnv, FakeRegistry};
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder, TempInstall};

pub struct TestEnv {
    pub clock: FakeClock,
    pub env: FakeEnv,
    pub registry: FakeRegistry,
    pub fs: RealFs,
}

impl DetectEnv for TestEnv {
    fn clock(&self) -> &dyn Clock {
        &self.clock
    }
    fn env(&self) -> &dyn EnvProbe {
        &self.env
    }
    fn registry(&self) -> &dyn RegistryProbe {
        &self.registry
    }
    fn fs(&self) -> &dyn FsProbe {
        &self.fs
    }
}

pub struct Harness {
    pub _roots_dir: tempfile::TempDir,
    pub roots: DataRoots,
    pub env: TestEnv,
    pub install: TempInstall,
}

pub fn def(name: &str) -> rimstudio_core::tree::Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", "rs test")
        .build()
}

pub fn standard_install() -> TempInstall {
    InstallBuilder::new()
        .core_def(def("RS_CoreThing"))
        .mod_folder(
            ModFolder::new("RS_ModsA", "rs.mods.a")
                .name("RS Mods A")
                .supports(&["1.6"])
                .def(def("RS_A1")),
        )
        .workshop_mod(
            111,
            ModFolder::new("ignored", "rs.workshop.one")
                .name("RS Workshop One")
                .def(def("RS_W1")),
        )
        .custom_mod(
            ModFolder::new("RS_Custom", "rs.custom.one")
                .name("RS Custom One")
                .def(def("RS_C1")),
        )
        .build_temp()
        .unwrap()
}

impl Harness {
    /// Empty roots and an environment that finds nothing by itself.
    pub fn bare() -> Harness {
        Harness::with_install(standard_install())
    }

    pub fn with_install(install: TempInstall) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let roots = DataRoots::under_base(&base);
        Harness {
            _roots_dir: dir,
            roots,
            env: TestEnv {
                clock: FakeClock::new(1_800_000_000_000),
                env: FakeEnv::new(),
                registry: FakeRegistry::new(),
                fs: RealFs::new(),
            },
            install,
        }
    }

    /// A harness whose game install and Workshop folder are set through overrides.
    pub fn with_overrides() -> Harness {
        let h = Harness::bare();
        h.pin_install();
        h
    }

    pub fn pin_install(&self) -> DetectRunResponse {
        let ctx = self.ctx();
        set_override(
            &ctx,
            DetectSetOverrideRequest {
                field: OverrideField::GameInstall,
                path: Some(self.install.game_dir.to_string()),
                pinned: true,
            },
        )
        .unwrap();
        set_override(
            &ctx,
            DetectSetOverrideRequest {
                field: OverrideField::WorkshopDir,
                path: Some(self.install.workshop_dir.to_string()),
                pinned: false,
            },
        )
        .unwrap()
    }

    pub fn ctx(&self) -> Ctx<'_> {
        Ctx::new(&self.roots, &self.env, &self.env.fs, &self.env.clock).with_os(Os::Linux)
    }

    pub fn settings_path(&self) -> Utf8PathBuf {
        self.roots.config.join("settings.jsonc")
    }

    pub fn workspace_path(&self) -> Utf8PathBuf {
        self.roots.config.join("workspace.jsonc")
    }

    pub fn read(&self, path: &Utf8Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }
}
