//! A golden of the file tree each Steam preset creates, so a change to a preset is reviewed.

use std::collections::BTreeMap;

use rimstudio_testing::prelude::*;

#[test]
fn preset_trees_match_the_golden() {
    let mut all: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for preset in SteamPreset::ALL {
        let d = FakeDetectEnv::from_preset(preset);
        let paths = d.fs.paths().into_iter().map(|p| p.to_string()).collect();
        all.insert(preset.name(), paths);
    }
    rimstudio_testing::golden_json!("steam-preset-trees", &all);
}
