//! The state of the target def's containers: what the generator checks before it picks Add, Replace or
//! the ensure-container idiom, and whether the target already carries a Combat Extended conversion.
//!
//! Patch operations run on the merged document before inheritance, so the state that matters is the state of
//! the **raw** def node (its own children, patches applied), not the resolved one: a `statBases` list that the
//! def only inherits from an abstract parent does not exist for an xpath. [`Container::from_node`] therefore
//! takes the raw node. [`Container::from_def`] takes the resolved record for the conversion markers and, when
//! no raw node is at hand, falls back to the resolved children, which can only over-report containers (the
//! dry run then catches a Replace on a path that is not really there).

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::tree::Node;
use rimstudio_defs::DefRecord;
use serde::{Deserialize, Serialize};

use crate::ce::reader::{CeClassNames, CeMarkers, ce_block_from_def, detect_markers};
use crate::model::{CePatchSpec, DesignSpec, ValueSource};
use crate::plan::weapon_def;
use crate::reader::access::{class_attr, list_items, list_texts};

/// Where the existing conversion of a target came from. It decides where update mode may write.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "file")]
pub enum ConversionSource {
    /// A file RimStudio wrote earlier (the given project path, when known).
    RimStudio(Option<String>),
    /// A file of the open project written by hand.
    Project(Option<String>),
    /// A file of another mod, Combat Extended itself included. Never rewritten.
    Foreign(Option<String>),
    /// Not known. Treated like a foreign file: nothing is rewritten.
    #[default]
    Unknown,
}

impl ConversionSource {
    /// True when the conversion lives in a file RimStudio wrote.
    #[must_use]
    pub fn is_rimstudio(&self) -> bool {
        matches!(self, Self::RimStudio(_))
    }

    /// A short word for messages.
    #[must_use]
    pub fn describe(&self) -> &'static str {
        match self {
            Self::RimStudio(_) => "a RimStudio file",
            Self::Project(_) => "a file of this project",
            Self::Foreign(_) => "a file of another mod",
            Self::Unknown => "an unknown source",
        }
    }
}

/// A Combat Extended conversion that the target already carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingConversion {
    /// Which of the conversion markers the target has.
    pub markers: CeMarkers,
    /// The values the conversion holds, read from the resolved def.
    pub block: CePatchSpec,
    /// Where the conversion came from.
    pub source: ConversionSource,
    /// The child names of the converted verb entry.
    pub verb_fields: BTreeSet<String>,
    /// The child names of the ammo user component.
    pub ammo_fields: BTreeSet<String>,
    /// The weapon tags of the resolved def.
    pub weapon_tags: Vec<String>,
}

impl ExistingConversion {
    /// Reads the conversion of a resolved def, `None` when the def carries no marker.
    #[must_use]
    pub fn from_def(
        def: &DefRecord,
        classes: &CeClassNames,
        source: ConversionSource,
    ) -> Option<Self> {
        let block = ce_block_from_def(def, classes, ValueSource::Anchor)?;
        let node = &def.node;
        let fields_of = |list: &str, class: &str| -> BTreeSet<String> {
            list_items(node, list)
                .into_iter()
                .find(|li| class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(class)))
                .map(|li| li.elements().map(|e| e.tag.clone()).collect())
                .unwrap_or_default()
        };
        Some(Self {
            markers: detect_markers(node, classes),
            block,
            source,
            verb_fields: fields_of("verbs", &classes.verb_properties),
            ammo_fields: fields_of("comps", &classes.ammo_user),
            weapon_tags: list_texts(node, "weaponTags"),
        })
    }
}

/// The containers of the target def and its existing conversion. See the module documentation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    /// The containers the def has, by element name, with the names of their child elements.
    pub containers: BTreeMap<String, Vec<String>>,
    /// The def inherits from a parent (so lists it does not hold itself may still be inherited).
    pub has_parent: bool,
    /// The conversion the target already carries, when it does.
    pub existing: Option<ExistingConversion>,
    /// The components the def lists itself: the `Class` attribute of each entry, else its `compClass`.
    /// The under barrel conversion looks here for the vanilla component it replaces.
    #[serde(default)]
    pub comp_classes: Vec<String>,
}

impl Container {
    /// A container state in which nothing exists: every optional container gets the ensure idiom.
    #[must_use]
    pub fn unknown() -> Self {
        Self::default()
    }

    /// The state of a raw def node (its own children).
    #[must_use]
    pub fn from_node(raw: &Node) -> Self {
        let containers = raw
            .elements()
            .map(|e| {
                (
                    e.tag.clone(),
                    e.elements().map(|c| c.tag.clone()).collect::<Vec<_>>(),
                )
            })
            .collect();
        Self {
            containers,
            has_parent: raw.attr("ParentName").is_some(),
            existing: None,
            comp_classes: crate::ce::reader::platform::comp_classes(raw),
        }
    }

    /// The state of the vanilla definition that the designer writes for `spec` (what a new item's patch
    /// targets).
    #[must_use]
    pub fn from_spec(spec: &DesignSpec) -> Self {
        Self::from_node(&weapon_def(spec))
    }

    /// The state of a def of the loaded set. `raw` is the unresolved node when the caller has it; without
    /// it the resolved children stand in (see the module documentation). The existing conversion is read
    /// from the resolved record.
    #[must_use]
    pub fn from_def(
        def: &DefRecord,
        raw: Option<&Node>,
        classes: &CeClassNames,
        source: ConversionSource,
    ) -> Self {
        let mut out = Self::from_node(raw.unwrap_or(&def.node));
        out.has_parent |= !def.parents.is_empty();
        out.existing = ExistingConversion::from_def(def, classes, source);
        out
    }

    /// The same state with an existing conversion attached.
    #[must_use]
    pub fn with_existing(mut self, existing: ExistingConversion) -> Self {
        self.existing = Some(existing);
        self
    }

    /// True when the def has the container.
    #[must_use]
    pub fn has(&self, container: &str) -> bool {
        self.containers.contains_key(container)
    }

    /// True when the container has a child element with that name (a stat, an offset).
    #[must_use]
    pub fn has_entry(&self, container: &str, entry: &str) -> bool {
        self.containers
            .get(container)
            .is_some_and(|c| c.iter().any(|e| e == entry))
    }

    /// True when the def lists a component with that class (or `compClass`).
    #[must_use]
    pub fn has_comp(&self, class: &str) -> bool {
        self.comp_classes.iter().any(|c| c == class)
    }

    /// True when the target already carries a Combat Extended conversion.
    #[must_use]
    pub fn is_converted(&self) -> bool {
        self.existing.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::tree::NodeBuilder;

    #[test]
    fn a_raw_node_gives_containers_and_entries() {
        let node = NodeBuilder::new("ThingDef")
            .attr("ParentName", "RS_Base")
            .text_elem("defName", "RS_X")
            .elem("statBases", |s| {
                s.text_elem("Mass", "2").text_elem("MarketValue", "30")
            })
            .empty_elem("tools")
            .build();
        let c = Container::from_node(&node);
        assert!(c.has("statBases"));
        assert!(c.has("tools"));
        assert!(!c.has("comps"));
        assert!(c.has_entry("statBases", "Mass"));
        assert!(!c.has_entry("statBases", "Bulk"));
        assert!(c.has_parent);
        assert!(!c.is_converted());
        assert_eq!(Container::unknown(), Container::default());
    }
}
