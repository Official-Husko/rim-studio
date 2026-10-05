//! Helpers of the layout fix tests: a world with an app data root, fixture mods copied into temporary folders,
//! snapshots of a folder, and short wrappers over the plan, apply, undo and history calls.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout
)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project::{LayoutIssueDto, ProjectLayoutCheckRequest};
use rimstudio_ipc_types::project_fix::{
    LayoutFixItemDto, LayoutFixSelectionDto, ProjectLayoutFixApplyDto,
    ProjectLayoutFixApplyRequest, ProjectLayoutFixHistoryDto, ProjectLayoutFixHistoryRequest,
    ProjectLayoutFixPlanDto, ProjectLayoutFixPlanRequest, ProjectLayoutFixUndoDto,
    ProjectLayoutFixUndoRequest,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitResult;
use rimstudio_toolkit::project::fix::plan;
use rimstudio_toolkit::project::fix_apply::{ApplyHooks, apply, apply_with};
use rimstudio_toolkit::project::history::history;
use rimstudio_toolkit::project::undo::undo;
use rimstudio_toolkit::project::{layout_check, open_project};
use rimstudio_toolkit::shared::env::ProjectEnv;

pub(crate) struct World {
    pub(crate) _tmp: tempfile::TempDir,
    pub(crate) base: Utf8PathBuf,
    pub(crate) env: ProjectEnv,
    pub(crate) roots: DataRoots,
}

pub(crate) fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    World {
        _tmp: tmp,
        base,
        env,
        roots,
    }
}

pub(crate) fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_dir(&entry.path(), &to.join(entry.file_name()));
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

/// Copies a committed fixture mod into the world and opens it.
pub(crate) fn fixture(w: &World, name: &str) -> (Utf8PathBuf, String) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/layout_fix")
        .join(name);
    let root = w.base.join(name);
    copy_dir(&src, root.as_std_path());
    let id = open_project(&w.env, &root)
        .unwrap()
        .record
        .id
        .as_str()
        .to_owned();
    (root, id)
}

pub(crate) const ABOUT: &str = "<ModMetaData><name>RS Fixture</name><packageId>rs.fixture</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";

pub(crate) fn put(root: &Utf8PathBuf, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(path.as_std_path(), text).unwrap();
}

/// A fictional mod made of the given files (About included).
pub(crate) fn mod_at(w: &World, name: &str, files: &[(&str, &str)]) -> (Utf8PathBuf, String) {
    let root = w.base.join(name);
    put(&root, "About/About.xml", ABOUT);
    for (rel, text) in files {
        put(&root, rel, text);
    }
    let id = open_project(&w.env, &root)
        .unwrap()
        .record
        .id
        .as_str()
        .to_owned();
    (root, id)
}

pub(crate) const CE_PATCH: &str = "<Patch><Operation Class=\"PatchOperationSequence\"><success>Always</success><operations>\
<li Class=\"CombatExtended.PatchOperationFindMod\"><modName>Combat Extended</modName></li>\
<li Class=\"PatchOperationReplace\"><xpath>/Defs/ThingDef[defName=\"RS_Rifle\"]/tools</xpath>\
<value><tools><li Class=\"CombatExtended.ToolCE\"><label>stock</label></li></tools></value></li>\
</operations></Operation></Patch>";

pub(crate) fn rifle(name: &str, tex: &str) -> String {
    format!(
        "<Defs><ThingDef ParentName=\"BaseBullet\"><defName>B_{name}</defName><projectile/></ThingDef>\
         <ThingDef ParentName=\"BaseGun\"><defName>{name}</defName><techLevel>Industrial</techLevel>\
         <graphicData><texPath>{tex}</texPath></graphicData>\
         <verbs><li><verbClass>Verb_Shoot</verbClass></li></verbs></ThingDef></Defs>"
    )
}

/// Every file and folder below a root, by relative path: the bytes of a file, `None` for a folder.
pub(crate) fn snapshot(root: &Utf8PathBuf) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().into_string().unwrap();
            let child = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let kind = e.file_type().unwrap();
            if kind.is_dir() {
                out.insert(child.clone(), None);
                walk(&e.path(), &child, out);
            } else if kind.is_file() {
                out.insert(child, Some(std::fs::read(e.path()).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root.as_std_path(), "", &mut out);
    out
}

pub(crate) fn plan_of(w: &World, id: &str) -> ProjectLayoutFixPlanDto {
    plan(
        &w.env,
        &ProjectLayoutFixPlanRequest {
            project_id: id.to_owned(),
            fixes: None,
        },
        &[],
    )
    .unwrap()
}

pub(crate) fn select_all(p: &ProjectLayoutFixPlanDto) -> Vec<LayoutFixSelectionDto> {
    p.items
        .iter()
        .filter(|i| i.applicable)
        .map(|i| LayoutFixSelectionDto {
            id: i.id.clone(),
            rename_on_conflict: false,
        })
        .collect()
}

pub(crate) fn apply_request(
    id: &str,
    p: &ProjectLayoutFixPlanDto,
    items: Vec<LayoutFixSelectionDto>,
) -> ProjectLayoutFixApplyRequest {
    ProjectLayoutFixApplyRequest {
        project_id: id.to_owned(),
        plan_id: p.plan_id.clone(),
        items,
    }
}

pub(crate) fn apply_all(w: &World, id: &str) -> ProjectLayoutFixApplyDto {
    let p = plan_of(w, id);
    apply(
        &w.env,
        &apply_request(id, &p, select_all(&p)),
        &[],
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap()
}

pub(crate) fn apply_selected(
    w: &World,
    id: &str,
    p: &ProjectLayoutFixPlanDto,
    items: Vec<LayoutFixSelectionDto>,
) -> ToolkitResult<ProjectLayoutFixApplyDto> {
    apply(
        &w.env,
        &apply_request(id, p, items),
        &[],
        &NoopProgress,
        &CancelToken::new(),
    )
}

pub(crate) fn apply_hooked(
    w: &World,
    id: &str,
    p: &ProjectLayoutFixPlanDto,
    hook: &dyn Fn(usize) -> bool,
) -> ToolkitResult<ProjectLayoutFixApplyDto> {
    apply_with(
        &w.env,
        &apply_request(id, p, select_all(p)),
        &[],
        &NoopProgress,
        &CancelToken::new(),
        &ApplyHooks {
            after_change: Some(hook),
        },
    )
}

pub(crate) fn undo_of(
    w: &World,
    id: &str,
    apply_id: &str,
) -> ToolkitResult<ProjectLayoutFixUndoDto> {
    undo(
        &w.env,
        &ProjectLayoutFixUndoRequest {
            project_id: id.to_owned(),
            apply_id: apply_id.to_owned(),
        },
        &[],
    )
}

pub(crate) fn history_of(w: &World, id: &str) -> ProjectLayoutFixHistoryDto {
    history(
        &w.env,
        &ProjectLayoutFixHistoryRequest {
            project_id: id.to_owned(),
        },
        &[],
    )
    .unwrap()
}

pub(crate) fn issues_of(w: &World, id: &str) -> Vec<LayoutIssueDto> {
    layout_check(
        &w.env,
        &ProjectLayoutCheckRequest {
            project_id: id.to_owned(),
        },
    )
    .unwrap()
    .issues
}

pub(crate) fn item<'a>(p: &'a ProjectLayoutFixPlanDto, issue_code: &str) -> &'a LayoutFixItemDto {
    p.items
        .iter()
        .find(|i| {
            i.issue_code == issue_code
                && i.kind == rimstudio_ipc_types::project_fix::LayoutFixItemKindDto::MoveFile
        })
        .unwrap_or_else(|| panic!("no item for {issue_code} in {:?}", p.items))
}

pub(crate) fn codes(issues: &[LayoutIssueDto]) -> Vec<&str> {
    issues.iter().map(|i| i.code.as_str()).collect()
}
