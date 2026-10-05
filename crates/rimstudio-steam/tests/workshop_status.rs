//! Workshop status through a fake environment: the third ACF fixture of the research (a current item,
//! a stale item and a folder the ACF does not know) plus a missing folder.

use std::time::Duration;

use camino::Utf8Path;
use rimstudio_steam::workshop::{ItemState, status_from_env};
use rimstudio_testing::detect_env::FakeDetectEnv;

const CONTENT: &str = "/rs/lib/steamapps/workshop/content/294100";
const ACF: &str = "/rs/lib/steamapps/workshop/appworkshop_294100.acf";
const D: Duration = Duration::from_millis(50);

fn fixture() -> FakeDetectEnv {
    let env = FakeDetectEnv::empty(false);
    env.fs.add_file(
        ACF,
        "\"AppWorkshop\"\n{\n\t\"appid\"\t\t\"294100\"\n\t\"NeedsUpdate\"\t\t\"0\"\n\t\"WorkshopItemsInstalled\"\n\t{\n\t\t\"111\"\t{ \"size\" \"10\" \"timeupdated\" \"100\" \"manifest\" \"1\" }\n\t\t\"222\"\t{ \"size\" \"10\" \"timeupdated\" \"100\" \"manifest\" \"2\" }\n\t\t\"444\"\t{ \"size\" \"10\" \"timeupdated\" \"100\" \"manifest\" \"4\" }\n\t}\n\t\"WorkshopItemDetails\"\n\t{\n\t\t\"111\"\t{ \"manifest\" \"1\" \"timeupdated\" \"100\" \"latest_timeupdated\" \"100\" \"latest_manifest\" \"1\" }\n\t\t\"222\"\t{ \"manifest\" \"2\" \"timeupdated\" \"100\" \"latest_timeupdated\" \"250\" \"latest_manifest\" \"3\" }\n\t}\n}\n",
    );
    for id in [111, 222, 333] {
        env.fs
            .add_file(format!("{CONTENT}/{id}/About/About.xml"), "x");
    }
    env
}

#[test]
fn statuses_follow_the_acf_and_the_folders() {
    let env = fixture();
    let statuses = status_from_env(&env, Utf8Path::new(CONTENT), Utf8Path::new(ACF), D).unwrap();
    let got: Vec<(u64, ItemState, bool)> = statuses
        .iter()
        .map(|s| (s.id, s.state, s.on_disk))
        .collect();
    assert_eq!(
        got,
        [
            (111, ItemState::Current, true),
            (222, ItemState::UpdateAvailable, true),
            (333, ItemState::Orphan, true),
            (444, ItemState::NotDownloaded, false),
        ]
    );
    rimstudio_testing::golden_json!("workshop-status", &statuses);
}

#[test]
fn a_missing_or_broken_manifest_is_an_error_with_a_code() {
    let env = fixture();
    let err = status_from_env(
        &env,
        Utf8Path::new(CONTENT),
        Utf8Path::new("/rs/none.acf"),
        D,
    )
    .unwrap_err();
    assert_eq!(err.code(), "port.not-found");
    env.fs.add_file("/rs/bad.acf", "\"AppWorkshop\" {");
    let err = status_from_env(
        &env,
        Utf8Path::new(CONTENT),
        Utf8Path::new("/rs/bad.acf"),
        D,
    )
    .unwrap_err();
    assert_eq!(err.code(), "steam.vdf-unclosed-object");
    env.fs.add_file("/rs/other.acf", "\"x\" { }");
    let err = status_from_env(
        &env,
        Utf8Path::new(CONTENT),
        Utf8Path::new("/rs/other.acf"),
        D,
    )
    .unwrap_err();
    assert_eq!(err.code(), "steam.shape");
}

#[test]
fn a_hung_content_folder_counts_as_empty_and_a_hung_manifest_is_a_timeout() {
    let env = fixture();
    env.fs.hang(CONTENT);
    let statuses = status_from_env(&env, Utf8Path::new(CONTENT), Utf8Path::new(ACF), D).unwrap();
    assert!(statuses.iter().all(|s| s.state == ItemState::NotDownloaded));
    let env = fixture();
    env.fs.hang(ACF);
    let err = status_from_env(&env, Utf8Path::new(CONTENT), Utf8Path::new(ACF), D).unwrap_err();
    assert_eq!(err.code(), "port.timed-out");
}
