//! Turning the changes of a mod basics request into edits of the text of `About.xml`.
//!
//! The values are checked here (a single line field holds no line break, a package id is not empty, a
//! version key looks like a version) and every edit goes through [`rimstudio_xml::about_edit::AboutEditor`],
//! so comments, element order, unknown elements and every field that is not edited stay byte for byte as
//! they were. Nothing is read from or written to disk.

use rimstudio_core::mods::ModDependency;
use rimstudio_core::version::parse_major_minor;
use rimstudio_ipc_types::project_about::{
    AboutChangeDto, AboutDependencyChangeDto, AboutDependencyDto, AboutTextFieldDto,
};
use rimstudio_xml::XmlError;
use rimstudio_xml::about_edit::{AboutEditor, DependencyPatch};

use crate::error::{ToolkitError, ToolkitResult};

/// The most changes one request may carry.
pub const MAX_CHANGES: usize = 500;
/// The longest text of one field (characters).
pub const MAX_FIELD_CHARS: usize = 100_000;

fn invalid(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::EditInvalid {
        reason: reason.into(),
    }
}

fn xml_error(e: XmlError) -> ToolkitError {
    match e {
        XmlError::EditPathMissing { path } => invalid(format!(
            "the file has no such entry ({path}); read the file again"
        )),
        XmlError::EditUnsupported { path, reason } => {
            invalid(format!("the entry at {path} cannot be edited: {reason}"))
        }
        XmlError::InvalidValue { what, reason } => {
            invalid(format!("{what} cannot be written: {reason}"))
        }
        other => ToolkitError::Xml(other),
    }
}

fn check_chars(what: &str, value: &str, multiline: bool) -> ToolkitResult<()> {
    if value.chars().count() > MAX_FIELD_CHARS {
        return Err(invalid(format!(
            "{what} is longer than {MAX_FIELD_CHARS} characters"
        )));
    }
    for c in value.chars() {
        let allowed_break = multiline && matches!(c, '\n' | '\r' | '\t');
        if c.is_control() && !allowed_break {
            return Err(invalid(if matches!(c, '\n' | '\r') {
                format!("{what} cannot hold a line break")
            } else {
                format!("{what} holds a control character")
            }));
        }
    }
    Ok(())
}

fn single(what: &str, value: &str) -> ToolkitResult<String> {
    check_chars(what, value, false)?;
    Ok(value.trim().to_owned())
}

fn version_key(version: &str) -> ToolkitResult<String> {
    let v = version.trim();
    if parse_major_minor(v, true).is_none() || v.chars().any(char::is_whitespace) {
        return Err(invalid(format!(
            "\"{v}\" is not a game version; write it as major.minor, for example 1.6"
        )));
    }
    Ok(v.to_owned())
}

fn dependency(dto: &AboutDependencyDto) -> ToolkitResult<ModDependency> {
    let package_id = single("the dependency package id", &dto.package_id)?;
    if package_id.is_empty() {
        return Err(invalid("a dependency needs a package id"));
    }
    let optional = |what: &str, v: &Option<String>| -> ToolkitResult<Option<String>> {
        match v {
            Some(t) => {
                let t = single(what, t)?;
                Ok((!t.is_empty()).then_some(t))
            }
            None => Ok(None),
        }
    };
    let mut alternatives = Vec::new();
    for a in &dto.alternatives {
        let a = single("an alternative package id", a)?;
        if !a.is_empty() {
            alternatives.push(a);
        }
    }
    Ok(ModDependency {
        package_id,
        display_name: single("the dependency name", &dto.display_name)?,
        steam_workshop_url: optional("the Workshop URL", &dto.steam_workshop_url)?,
        download_url: optional("the download URL", &dto.download_url)?,
        alternatives,
    })
}

fn patch(dto: &AboutDependencyChangeDto) -> ToolkitResult<DependencyPatch> {
    let opt = |what: &str, v: &Option<String>| -> ToolkitResult<Option<String>> {
        v.as_deref().map(|t| single(what, t)).transpose()
    };
    let package_id = opt("the dependency package id", &dto.package_id)?;
    if package_id.as_deref() == Some("") {
        return Err(invalid("a dependency needs a package id"));
    }
    let alternatives = match &dto.alternatives {
        Some(list) => {
            let mut out = Vec::new();
            for a in list {
                let a = single("an alternative package id", a)?;
                if !a.is_empty() {
                    out.push(a);
                }
            }
            Some(out)
        }
        None => None,
    };
    Ok(DependencyPatch {
        package_id,
        display_name: opt("the dependency name", &dto.display_name)?,
        steam_workshop_url: opt("the Workshop URL", &dto.steam_workshop_url)?,
        download_url: opt("the download URL", &dto.download_url)?,
        alternatives,
    })
}

fn position(n: u32) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

fn items(what: &str, list: &[String]) -> ToolkitResult<Vec<String>> {
    let mut out = Vec::new();
    for i in list {
        let i = single(what, i)?;
        if !i.is_empty() {
            out.push(i);
        }
    }
    Ok(out)
}

/// Applies `changes` in order to the text of an About file and returns the new text.
///
/// # Errors
///
/// [`ToolkitError::EditInvalid`] for a value that cannot be written or an entry the file does not have,
/// [`ToolkitError::Xml`] when the text is not a well formed document.
pub fn apply_changes(text: &str, changes: &[AboutChangeDto]) -> ToolkitResult<String> {
    if changes.len() > MAX_CHANGES {
        return Err(invalid(format!(
            "a request may carry at most {MAX_CHANGES} changes"
        )));
    }
    let mut ed = AboutEditor::new(text).map_err(xml_error)?;
    for change in changes {
        apply_one(&mut ed, change)?;
    }
    Ok(ed.into_text())
}

fn apply_one(ed: &mut AboutEditor, change: &AboutChangeDto) -> ToolkitResult<()> {
    match change {
        AboutChangeDto::Set { field, value } => {
            if *field == AboutTextFieldDto::Description {
                check_chars("the description", value, true)?;
                ed.set_text(field.tag(), value).map_err(xml_error)
            } else {
                let v = single(field.tag(), value)?;
                ed.set_text(field.tag(), &v).map_err(xml_error)
            }
        }
        AboutChangeDto::Clear { field } => ed.clear(field.tag()).map_err(xml_error),
        AboutChangeDto::ListSet { field, items: list } => {
            let list = items(field.tag(), list)?;
            ed.set_list(field.tag(), &list).map_err(xml_error)
        }
        AboutChangeDto::ListAdd { field, value, at } => {
            let v = single(field.tag(), value)?;
            ed.list_add(field.tag(), &v, at.map(position))
                .map_err(xml_error)
        }
        AboutChangeDto::ListRemove { field, value } => {
            let v = single(field.tag(), value)?;
            ed.list_remove(field.tag(), &v).map_err(xml_error)
        }
        AboutChangeDto::ListMove { field, value, to } => {
            let v = single(field.tag(), value)?;
            ed.list_move(field.tag(), &v, position(*to))
                .map_err(xml_error)
        }
        AboutChangeDto::DependencyAdd { dependency: d, at } => {
            let dep = dependency(d)?;
            ed.dependency_add(&dep, at.map(position)).map_err(xml_error)
        }
        AboutChangeDto::DependencyUpdate { package_id, change } => {
            let id = single("the package id", package_id)?;
            ed.dependency_update(&id, &patch(change)?)
                .map_err(xml_error)
        }
        AboutChangeDto::DependencyRemove { package_id } => {
            let id = single("the package id", package_id)?;
            ed.dependency_remove(&id).map_err(xml_error)
        }
        AboutChangeDto::DependencyMove { package_id, to } => {
            let id = single("the package id", package_id)?;
            ed.dependency_move(&id, position(*to)).map_err(xml_error)
        }
        AboutChangeDto::ByVersionListSet {
            field,
            version,
            items: list,
        } => {
            let key = version_key(version)?;
            let list = items(field.tag(), list)?;
            ed.by_version_set_list(field.tag(), &key, &list)
                .map_err(xml_error)
        }
        AboutChangeDto::ByVersionDescriptionSet { version, value } => {
            let key = version_key(version)?;
            if let Some(v) = value {
                check_chars("the description", v, true)?;
            }
            ed.by_version_set_description(&key, value.as_deref())
                .map_err(xml_error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_ipc_types::project_about::AboutListFieldDto;

    const BASE: &str =
        "<ModMetaData>\n  <name>A</name>\n  <packageId>a.b</packageId>\n</ModMetaData>\n";

    #[test]
    fn changes_apply_in_order() {
        let out = apply_changes(
            BASE,
            &[
                AboutChangeDto::Set {
                    field: AboutTextFieldDto::Name,
                    value: "  New  ".into(),
                },
                AboutChangeDto::ListAdd {
                    field: AboutListFieldDto::SupportedVersions,
                    value: "1.6".into(),
                    at: None,
                },
                AboutChangeDto::Clear {
                    field: AboutTextFieldDto::PackageId,
                },
            ],
        )
        .unwrap();
        assert!(out.contains("<name>New</name>"));
        assert!(out.contains("<li>1.6</li>"));
        assert!(!out.contains("packageId"));
    }

    #[test]
    fn bad_values_are_refused_with_a_plain_reason() {
        let set = |field, value: &str| AboutChangeDto::Set {
            field,
            value: value.into(),
        };
        let err = apply_changes(BASE, &[set(AboutTextFieldDto::Name, "a\nb")]).unwrap_err();
        assert_eq!(err.code(), "project.edit-invalid");
        assert!(err.to_string().contains("line break"));
        assert!(apply_changes(BASE, &[set(AboutTextFieldDto::Description, "a\nb")]).is_ok());
        assert!(apply_changes(BASE, &[set(AboutTextFieldDto::Name, "a\u{7}")]).is_err());
        let err = apply_changes(
            BASE,
            &[AboutChangeDto::ByVersionDescriptionSet {
                version: "latest".into(),
                value: Some("x".into()),
            }],
        )
        .unwrap_err();
        assert!(err.to_string().contains("not a game version"));
        let err = apply_changes(
            BASE,
            &[AboutChangeDto::DependencyRemove {
                package_id: "  ".into(),
            }],
        );
        assert!(err.is_ok(), "removing nothing is not an error");
        let err = apply_changes(
            BASE,
            &[AboutChangeDto::DependencyMove {
                package_id: "x.y".into(),
                to: 0,
            }],
        )
        .unwrap_err();
        assert_eq!(err.code(), "project.edit-invalid");
    }

    #[test]
    fn a_text_that_is_not_xml_is_an_xml_error() {
        let err = apply_changes("<a><b></a>", &[]).unwrap_err();
        assert!(matches!(err, ToolkitError::Xml(_)));
        assert!(
            apply_changes(
                BASE,
                &vec![
                    AboutChangeDto::Clear {
                        field: AboutTextFieldDto::Url
                    };
                    MAX_CHANGES + 1
                ]
            )
            .is_err()
        );
    }
}
