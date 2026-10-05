//! One import for tests: `use rimstudio_testing::prelude::*;`.

pub use crate::detect_env::{FakeDetectEnv, SteamPreset};
pub use crate::fake_fs::{FakeFs, RecordingFs, WriteOp};
pub use crate::fakes::{
    FakeClock, FakeCredentialStore, FakeEnv, FakeInstallSource, FakeLauncher, FakeLinkBackend,
    FakeProcessProbe, FakeRegistry, FakeSandbox, LinkOp,
};
pub use crate::fixtures::{ModBuilder, game_version, mod_index, package_id, rs_package_id};
pub use crate::golden::{assert_json_golden, to_canonical_json, update_requested};
