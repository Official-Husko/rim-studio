//! Patch operations as data: the 13 vanilla classes, unknown classes and custom hooks.
//!
//! [`parse_patch_file`] turns the `Patch` element of a patch file into [`PatchOp`]s the way the game's
//! `DirectXmlToObject` does: the `Class` attribute selects the class (case insensitive, with or
//! without the `Verse.` prefix), fields match by exact name and then ignoring case (with an error),
//! `li` entries of the `operations` and `mods` lists honour `MayRequire` and `MayRequireAnyOf`, and
//! an unknown class becomes the base `PatchOperation`, which always fails but whose `success` field
//! still applies. The raw `Operation` element of an unknown class is kept, so a higher layer can read
//! its parameters (`MakeGunCECompatible` stays an unknown operation with its raw parameters).
//!
//! Classes registered in a [`CustomRegistry`] become [`PatchKind::Custom`] and are applied through
//! the registered handler. Operations are plain serialisable data; XPath text is parsed when the
//! operation is applied.

use rimstudio_core::diag::{DiagnosticSink, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

use crate::custom_ops::CustomRegistry;
use crate::diag_codes::{self, Site};

/// How the `success` field rewrites the result of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Success {
    /// The worker's result is kept.
    #[default]
    Normal,
    /// The result is negated.
    Invert,
    /// The result is true whatever happened.
    Always,
    /// The result is false whatever happened (the change is not undone).
    Never,
}

impl Success {
    /// Applies the rewrite to a worker result.
    #[must_use]
    pub fn rewrite(self, worker: bool) -> bool {
        match self {
            Self::Normal => worker,
            Self::Invert => !worker,
            Self::Always => true,
            Self::Never => false,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "Normal" => Some(Self::Normal),
            "Invert" => Some(Self::Invert),
            "Always" => Some(Self::Always),
            "Never" => Some(Self::Never),
            _ => None,
        }
    }
}

/// Where `Add` and `Insert` put the copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// After the existing children (`Add`) or after the target (`Insert`).
    Append,
    /// Before the existing children (`Add`) or before the target (`Insert`).
    Prepend,
}

/// One patch operation: the common `success` wrapper around a class specific [`PatchKind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOp {
    /// The `success` field.
    pub success: Success,
    /// The `Class` attribute as written, `None` when it is missing.
    pub declared_class: Option<String>,
    /// The patch file the operation was read from, when known.
    pub file: Option<FileId>,
    /// The class specific part.
    pub kind: PatchKind,
}

/// The class specific part of a [`PatchOp`]. A missing text field is `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PatchKind {
    /// `PatchOperation` itself (a missing `Class`): it always fails.
    Base,
    /// `PatchOperationAdd`.
    Add {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The `value` element whose children are copied.
        value: Option<Node>,
        /// Append (default) or Prepend.
        order: Order,
    },
    /// `PatchOperationInsert`.
    Insert {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The `value` element whose children are copied.
        value: Option<Node>,
        /// Prepend (default) or Append.
        order: Order,
    },
    /// `PatchOperationReplace`.
    Replace {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The `value` element whose children replace each target.
        value: Option<Node>,
    },
    /// `PatchOperationRemove`.
    Remove {
        /// The XPath of the targets.
        xpath: Option<String>,
    },
    /// `PatchOperationAddModExtension`.
    AddModExtension {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The `value` element whose children are appended to `modExtensions`.
        value: Option<Node>,
    },
    /// `PatchOperationAttributeAdd`.
    AttributeAdd {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The attribute name.
        attribute: Option<String>,
        /// The attribute value.
        value: Option<String>,
    },
    /// `PatchOperationAttributeSet`.
    AttributeSet {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The attribute name.
        attribute: Option<String>,
        /// The attribute value.
        value: Option<String>,
    },
    /// `PatchOperationAttributeRemove`.
    AttributeRemove {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The attribute name.
        attribute: Option<String>,
    },
    /// `PatchOperationSetName`.
    SetName {
        /// The XPath of the targets.
        xpath: Option<String>,
        /// The new element name.
        name: Option<String>,
    },
    /// `PatchOperationTest`.
    Test {
        /// The XPath that must select something.
        xpath: Option<String>,
    },
    /// `PatchOperationConditional`.
    Conditional {
        /// The XPath to test.
        xpath: Option<String>,
        /// Runs when the XPath selects something.
        #[serde(rename = "match")]
        match_op: Option<Box<PatchOp>>,
        /// Runs when it selects nothing.
        #[serde(rename = "nomatch")]
        nomatch_op: Option<Box<PatchOp>>,
    },
    /// `PatchOperationFindMod`: compares mod display names, not package ids.
    FindMod {
        /// The display names to look for (any one is enough).
        mods: Vec<String>,
        /// Runs when one of the mods is active.
        #[serde(rename = "match")]
        match_op: Option<Box<PatchOp>>,
        /// Runs when none is.
        #[serde(rename = "nomatch")]
        nomatch_op: Option<Box<PatchOp>>,
    },
    /// `PatchOperationSequence`: `None` when the `operations` field is missing (an exception).
    Sequence {
        /// The steps, in order.
        operations: Option<Vec<PatchOp>>,
    },
    /// A class the engine does not know and no handler is registered for.
    Unknown {
        /// The `Class` attribute as written.
        class: String,
        /// The whole `Operation` element, so its parameters stay readable.
        raw: Node,
    },
    /// A class with a registered handler ([`crate::custom_ops::CustomPatchOp`]).
    Custom {
        /// The `Class` attribute as written.
        class: String,
        /// The whole `Operation` element, handed to the handler.
        raw: Node,
    },
}

impl PatchOp {
    /// An operation of the given kind with `success` Normal and no source file.
    #[must_use]
    pub fn new(kind: PatchKind) -> Self {
        Self {
            success: Success::Normal,
            declared_class: None,
            file: None,
            kind,
        }
    }

    /// The canonical class name (`PatchOperationAdd`); the declared name for unknown and custom
    /// classes.
    #[must_use]
    pub fn class_name(&self) -> &str {
        match &self.kind {
            PatchKind::Base => "PatchOperation",
            PatchKind::Add { .. } => "PatchOperationAdd",
            PatchKind::Insert { .. } => "PatchOperationInsert",
            PatchKind::Replace { .. } => "PatchOperationReplace",
            PatchKind::Remove { .. } => "PatchOperationRemove",
            PatchKind::AddModExtension { .. } => "PatchOperationAddModExtension",
            PatchKind::AttributeAdd { .. } => "PatchOperationAttributeAdd",
            PatchKind::AttributeSet { .. } => "PatchOperationAttributeSet",
            PatchKind::AttributeRemove { .. } => "PatchOperationAttributeRemove",
            PatchKind::SetName { .. } => "PatchOperationSetName",
            PatchKind::Test { .. } => "PatchOperationTest",
            PatchKind::Conditional { .. } => "PatchOperationConditional",
            PatchKind::FindMod { .. } => "PatchOperationFindMod",
            PatchKind::Sequence { .. } => "PatchOperationSequence",
            PatchKind::Unknown { class, .. } | PatchKind::Custom { class, .. } => class,
        }
    }

    /// The class as written in the file, or the canonical name when `Class` is missing.
    #[must_use]
    pub fn display_class(&self) -> String {
        self.declared_class
            .clone()
            .unwrap_or_else(|| self.class_name().to_owned())
    }

    /// The XPath of the operation, when its class has one and it is present.
    #[must_use]
    pub fn xpath(&self) -> Option<&str> {
        match &self.kind {
            PatchKind::Add { xpath, .. }
            | PatchKind::Insert { xpath, .. }
            | PatchKind::Replace { xpath, .. }
            | PatchKind::Remove { xpath }
            | PatchKind::AddModExtension { xpath, .. }
            | PatchKind::AttributeAdd { xpath, .. }
            | PatchKind::AttributeSet { xpath, .. }
            | PatchKind::AttributeRemove { xpath, .. }
            | PatchKind::SetName { xpath, .. }
            | PatchKind::Test { xpath }
            | PatchKind::Conditional { xpath, .. } => xpath.as_deref(),
            _ => None,
        }
    }

    /// A one line description for messages: `Class(xpath)` with white space collapsed.
    #[must_use]
    pub fn describe(&self) -> String {
        let collapse = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        match &self.kind {
            PatchKind::FindMod { mods, .. } => {
                format!("{}({})", self.class_name(), mods.join(", "))
            }
            PatchKind::AttributeAdd {
                xpath, attribute, ..
            }
            | PatchKind::AttributeSet {
                xpath, attribute, ..
            }
            | PatchKind::AttributeRemove { xpath, attribute } => format!(
                "{}({})({})",
                self.class_name(),
                collapse(xpath.as_deref().unwrap_or("")),
                attribute.as_deref().unwrap_or("")
            ),
            _ => match self.xpath() {
                Some(x) => format!("{}({})", self.class_name(), collapse(x)),
                None => self.class_name().to_owned(),
            },
        }
    }

    /// The raw `Operation` element of an unknown or custom class.
    #[must_use]
    pub fn raw(&self) -> Option<&Node> {
        match &self.kind {
            PatchKind::Unknown { raw, .. } | PatchKind::Custom { raw, .. } => Some(raw),
            _ => None,
        }
    }

    /// True for operations of a class the engine cannot run by itself.
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        matches!(self.kind, PatchKind::Unknown { .. })
    }
}

/// What the parser needs to know about the load: the registered custom classes and the active mods
/// (for `MayRequire` on list entries).
#[derive(Debug, Clone, Copy)]
pub struct ParseEnv<'a> {
    /// Custom classes; their operations parse as [`PatchKind::Custom`].
    pub registry: &'a CustomRegistry,
    /// The active package ids.
    pub active: &'a ActiveSet,
}

/// The vanilla classes, as a parse time tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Base,
    Add,
    Insert,
    Replace,
    Remove,
    AddModExtension,
    AttributeAdd,
    AttributeSet,
    AttributeRemove,
    SetName,
    Test,
    Conditional,
    FindMod,
    Sequence,
}

impl Tag {
    fn from_class(class: &str) -> Option<Tag> {
        let lower = class.to_lowercase();
        let name = lower.strip_prefix("verse.").unwrap_or(&lower);
        Some(match name {
            "patchoperation" => Tag::Base,
            "patchoperationadd" => Tag::Add,
            "patchoperationinsert" => Tag::Insert,
            "patchoperationreplace" => Tag::Replace,
            "patchoperationremove" => Tag::Remove,
            "patchoperationaddmodextension" => Tag::AddModExtension,
            "patchoperationattributeadd" => Tag::AttributeAdd,
            "patchoperationattributeset" => Tag::AttributeSet,
            "patchoperationattributeremove" => Tag::AttributeRemove,
            "patchoperationsetname" => Tag::SetName,
            "patchoperationtest" => Tag::Test,
            "patchoperationconditional" => Tag::Conditional,
            "patchoperationfindmod" => Tag::FindMod,
            "patchoperationsequence" => Tag::Sequence,
            _ => return None,
        })
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Tag::Base => &["success"],
            Tag::Add | Tag::Insert => &["success", "xpath", "value", "order"],
            Tag::Replace | Tag::AddModExtension => &["success", "xpath", "value"],
            Tag::Remove | Tag::Test => &["success", "xpath"],
            Tag::AttributeAdd | Tag::AttributeSet => &["success", "xpath", "attribute", "value"],
            Tag::AttributeRemove => &["success", "xpath", "attribute"],
            Tag::SetName => &["success", "xpath", "name"],
            Tag::Conditional => &["success", "xpath", "match", "nomatch"],
            Tag::FindMod => &["success", "mods", "match", "nomatch"],
            Tag::Sequence => &["success", "operations"],
        }
    }

    fn default_kind(self) -> PatchKind {
        match self {
            Tag::Base => PatchKind::Base,
            Tag::Add => PatchKind::Add {
                xpath: None,
                value: None,
                order: Order::Append,
            },
            Tag::Insert => PatchKind::Insert {
                xpath: None,
                value: None,
                order: Order::Prepend,
            },
            Tag::Replace => PatchKind::Replace {
                xpath: None,
                value: None,
            },
            Tag::Remove => PatchKind::Remove { xpath: None },
            Tag::AddModExtension => PatchKind::AddModExtension {
                xpath: None,
                value: None,
            },
            Tag::AttributeAdd => PatchKind::AttributeAdd {
                xpath: None,
                attribute: None,
                value: None,
            },
            Tag::AttributeSet => PatchKind::AttributeSet {
                xpath: None,
                attribute: None,
                value: None,
            },
            Tag::AttributeRemove => PatchKind::AttributeRemove {
                xpath: None,
                attribute: None,
            },
            Tag::SetName => PatchKind::SetName {
                xpath: None,
                name: None,
            },
            Tag::Test => PatchKind::Test { xpath: None },
            Tag::Conditional => PatchKind::Conditional {
                xpath: None,
                match_op: None,
                nomatch_op: None,
            },
            Tag::FindMod => PatchKind::FindMod {
                mods: Vec::new(),
                match_op: None,
                nomatch_op: None,
            },
            Tag::Sequence => PatchKind::Sequence { operations: None },
        }
    }
}

/// The mutable state of one parse: where diagnostics go and what they point at.
struct Parser<'a, 'b> {
    env: &'a ParseEnv<'a>,
    sink: &'b mut dyn DiagnosticSink,
    site: Site,
    file: Option<FileId>,
}

impl Parser<'_, '_> {
    fn emit(&mut self, code: rimstudio_core::diag::DiagCode, sev: Severity, msg: String) {
        self.sink.emit(self.site.diag(code, sev, msg));
    }

    fn operation(&mut self, el: &Node) -> PatchOp {
        let declared = el.attr("Class").map(str::to_owned);
        let mut op = PatchOp {
            success: Success::Normal,
            declared_class: declared.clone(),
            file: self.file,
            kind: PatchKind::Base,
        };
        let (tag, custom, unknown) = match declared.as_deref() {
            None => (Some(Tag::Base), false, false),
            Some(class) => match Tag::from_class(class) {
                Some(t) => (Some(t), false, false),
                None if self.env.registry.contains(class) => (None, true, false),
                None => (None, false, true),
            },
        };
        let class_text = declared.unwrap_or_default();
        if let Some(t) = tag {
            op.kind = t.default_kind();
        } else if custom {
            op.kind = PatchKind::Custom {
                class: class_text.clone(),
                raw: el.clone(),
            };
        } else if unknown {
            op.kind = PatchKind::Unknown {
                class: class_text.clone(),
                raw: el.clone(),
            };
            self.emit(
                diag_codes::PATCH_UNKNOWN_CLASS,
                Severity::Warning,
                format!(
                    "Could not find type named {class_text}: the operation becomes the base \
                     PatchOperation (always fails unless success is Always or Invert)"
                ),
            );
        }
        let fields: &[&str] = match tag {
            Some(t) => t.fields(),
            None => &["success"],
        };
        let mut seen: Vec<&str> = Vec::new();
        for child in el.elements() {
            let name = child.tag.as_str();
            if seen.contains(&name) {
                self.emit(
                    diag_codes::PATCH_DUPLICATE_FIELD,
                    Severity::Error,
                    format!("{} defines the same field twice: {name}", op.class_name()),
                );
            }
            seen.push(name);
            let Some(field) = self.find_field(fields, name, op.class_name()) else {
                if !custom {
                    let sev = if unknown {
                        Severity::Info
                    } else {
                        Severity::Error
                    };
                    self.emit(
                        diag_codes::PATCH_UNKNOWN_FIELD,
                        sev,
                        format!("{} has no field {name:?}", op.class_name()),
                    );
                }
                continue;
            };
            self.set_field(&mut op, field, child);
        }
        op
    }

    fn find_field(
        &mut self,
        fields: &[&'static str],
        name: &str,
        class: &str,
    ) -> Option<&'static str> {
        if let Some(f) = fields.iter().find(|f| **f == name) {
            return Some(f);
        }
        let hit = fields.iter().find(|f| f.eq_ignore_ascii_case(name))?;
        let hit: &'static str = hit;
        self.emit(
            diag_codes::PATCH_FIELD_CASE_MISMATCH,
            Severity::Error,
            format!("{class}: xml tags are case-sensitive: {name}"),
        );
        Some(hit)
    }

    fn set_field(&mut self, op: &mut PatchOp, field: &str, el: &Node) {
        let text = el.text_content();
        if field == "success" {
            match Success::parse(&text) {
                Some(s) => op.success = s,
                None => self.parse_error(op, field, &text, "Success"),
            }
            return;
        }
        if field == "order" {
            let parsed = match text.as_str() {
                "Append" => Order::Append,
                "Prepend" => Order::Prepend,
                _ => {
                    self.parse_error(op, field, &text, "Order");
                    return;
                }
            };
            if let PatchKind::Add { order, .. } | PatchKind::Insert { order, .. } = &mut op.kind {
                *order = parsed;
            }
            return;
        }
        match (&mut op.kind, field) {
            (
                PatchKind::Add { value, .. }
                | PatchKind::Insert { value, .. }
                | PatchKind::Replace { value, .. }
                | PatchKind::AddModExtension { value, .. },
                "value",
            ) => *value = Some(el.clone()),
            (
                PatchKind::Add { xpath, .. }
                | PatchKind::Insert { xpath, .. }
                | PatchKind::Replace { xpath, .. }
                | PatchKind::Remove { xpath }
                | PatchKind::AddModExtension { xpath, .. }
                | PatchKind::AttributeAdd { xpath, .. }
                | PatchKind::AttributeSet { xpath, .. }
                | PatchKind::AttributeRemove { xpath, .. }
                | PatchKind::SetName { xpath, .. }
                | PatchKind::Test { xpath }
                | PatchKind::Conditional { xpath, .. },
                "xpath",
            ) => *xpath = Some(text),
            (
                PatchKind::AttributeAdd { attribute, .. }
                | PatchKind::AttributeSet { attribute, .. }
                | PatchKind::AttributeRemove { attribute, .. },
                "attribute",
            ) => *attribute = Some(text),
            (
                PatchKind::AttributeAdd { value, .. } | PatchKind::AttributeSet { value, .. },
                "value",
            ) => *value = Some(text),
            (PatchKind::SetName { name, .. }, "name") => *name = Some(text),
            (
                PatchKind::Conditional { match_op, .. } | PatchKind::FindMod { match_op, .. },
                "match",
            ) => *match_op = Some(Box::new(self.operation(el))),
            (
                PatchKind::Conditional { nomatch_op, .. } | PatchKind::FindMod { nomatch_op, .. },
                "nomatch",
            ) => *nomatch_op = Some(Box::new(self.operation(el))),
            (PatchKind::FindMod { mods, .. }, "mods") => {
                *mods = self
                    .list_items(el)
                    .into_iter()
                    .map(Node::text_content)
                    .collect();
            }
            (PatchKind::Sequence { operations }, "operations") => {
                let items = self.list_items(el);
                *operations = Some(items.into_iter().map(|li| self.operation(li)).collect());
            }
            _ => {}
        }
    }

    fn parse_error(&mut self, op: &PatchOp, field: &str, text: &str, what: &str) {
        self.emit(
            diag_codes::PATCH_FIELD_PARSE_ERROR,
            Severity::Error,
            format!(
                "{}.{field}: {text:?} is not a valid value for {what}",
                op.class_name()
            ),
        );
    }

    /// The `li` entries of a list field that pass `MayRequire` and `MayRequireAnyOf`.
    fn list_items<'n>(&mut self, el: &'n Node) -> Vec<&'n Node> {
        let mut out = Vec::new();
        for c in el.elements() {
            if c.tag != "li" {
                self.emit(
                    diag_codes::PATCH_LIST_ITEM_NOT_LI,
                    Severity::Error,
                    format!("List item found with name {}", c.tag),
                );
                continue;
            }
            if li_requirements_met(c, self.env.active) {
                out.push(c);
            }
        }
        out
    }
}

/// `MayRequire` (all of) and `MayRequireAnyOf` (any of) on a list entry: an empty value is ignored,
/// and when `MayRequire` is present and non-empty it is the only one tested.
pub(crate) fn li_requirements_met(li: &Node, active: &ActiveSet) -> bool {
    if let Some(mr) = li.attr("MayRequire").filter(|v| !v.is_empty()) {
        let ids: Vec<&str> = mr.split(',').collect();
        return active.all_active(&ids);
    }
    if let Some(any) = li.attr("MayRequireAnyOf").filter(|v| !v.is_empty()) {
        let ids: Vec<&str> = any.split(',').collect();
        return active.any_active(&ids);
    }
    true
}

/// Parses one `Operation` element (or a nested `li`, `match` or `nomatch` element).
pub fn parse_operation(
    el: &Node,
    env: &ParseEnv<'_>,
    site: Site,
    file: Option<FileId>,
    sink: &mut dyn DiagnosticSink,
) -> PatchOp {
    let mut p = Parser {
        env,
        sink,
        site,
        file,
    };
    p.operation(el)
}

/// Parses a patch file: the document element must be `Patch` and its element children `Operation`.
///
/// Other roots and children are reported (`defs.patch-bad-root`, `defs.patch-bad-element`) and
/// skipped. `mod_idx` and `file` tag the diagnostics and the operations.
pub fn parse_patch_file(
    root: &Node,
    env: &ParseEnv<'_>,
    mod_idx: Option<ModIdx>,
    file: Option<FileId>,
    sink: &mut dyn DiagnosticSink,
) -> Vec<PatchOp> {
    let site = Site::new(mod_idx, file);
    let mut ops = Vec::new();
    if root.tag != "Patch" {
        sink.emit(site.diag(
            diag_codes::PATCH_BAD_ROOT,
            Severity::Error,
            format!(
                "Unexpected document element in patch XML; got {}, expected 'Patch'",
                root.tag
            ),
        ));
        return ops;
    }
    let mut p = Parser {
        env,
        sink,
        site,
        file,
    };
    for c in root.elements() {
        if c.tag != "Operation" {
            p.emit(
                diag_codes::PATCH_BAD_ELEMENT,
                Severity::Error,
                format!(
                    "Unexpected element in patch XML; got {}, expected 'Operation'",
                    c.tag
                ),
            );
            continue;
        }
        ops.push(p.operation(c));
    }
    ops
}

/// Parses `Operation` elements without a registry, an active set or diagnostics: every class that
/// is not one of the 13 vanilla classes is [`PatchKind::Unknown`], and list entries with a
/// `MayRequire` are skipped (nothing is active). A convenience for tools and tests; the load
/// pipeline uses [`parse_patch_file`].
#[must_use]
pub fn parse_patches(operations: &[Node]) -> Vec<PatchOp> {
    let registry = CustomRegistry::new();
    let active = ActiveSet::new();
    let env = ParseEnv {
        registry: &registry,
        active: &active,
    };
    let mut sink = rimstudio_core::diag::DiscardSink;
    operations
        .iter()
        .map(|el| parse_operation(el, &env, Site::default(), None, &mut sink))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::diag::DiagSink;
    use rimstudio_core::tree::NodeBuilder;

    fn op_node(class: Option<&str>) -> NodeBuilder {
        let b = NodeBuilder::new("Operation");
        match class {
            Some(c) => b.attr("Class", c),
            None => b,
        }
    }

    fn parse_one(node: Node) -> (PatchOp, DiagSink) {
        let registry = CustomRegistry::new();
        let active = ActiveSet::from_ids(["rs.on"]);
        let env = ParseEnv {
            registry: &registry,
            active: &active,
        };
        let mut sink = DiagSink::new();
        let op = parse_operation(&node, &env, Site::default(), None, &mut sink);
        (op, sink)
    }

    #[test]
    fn add_has_append_as_default_order_and_keeps_the_value_node() {
        let node = op_node(Some("PatchOperationAdd"))
            .text_elem("xpath", "Defs/RS_A")
            .child(NodeBuilder::new("value").text_elem("x", "1"))
            .build();
        let (op, sink) = parse_one(node);
        assert!(sink.is_empty());
        match op.kind {
            PatchKind::Add {
                xpath,
                value,
                order,
            } => {
                assert_eq!(xpath.as_deref(), Some("Defs/RS_A"));
                assert_eq!(value.map(|v| v.tag), Some("value".to_owned()));
                assert_eq!(order, Order::Append);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn insert_defaults_to_prepend() {
        let (op, _) = parse_one(op_node(Some("PatchOperationInsert")).build());
        assert!(matches!(
            op.kind,
            PatchKind::Insert {
                order: Order::Prepend,
                ..
            }
        ));
    }

    #[test]
    fn class_names_ignore_case_and_the_verse_prefix() {
        for class in [
            "PatchOperationadd",
            "Verse.PatchOperationAdd",
            "verse.patchoperationADD",
        ] {
            let (op, sink) = parse_one(op_node(Some(class)).build());
            assert!(matches!(op.kind, PatchKind::Add { .. }), "{class}");
            assert!(sink.is_empty(), "{class}");
        }
    }

    #[test]
    fn a_field_with_the_wrong_case_is_used_with_an_error() {
        let node = op_node(Some("PatchOperationRemove"))
            .text_elem("xPath", "Defs/X")
            .build();
        let (op, sink) = parse_one(node);
        assert_eq!(op.xpath(), Some("Defs/X"));
        assert_eq!(sink.count(&diag_codes::PATCH_FIELD_CASE_MISMATCH), 1);
    }

    #[test]
    fn unknown_classes_keep_the_raw_element_and_warn() {
        let node = op_node(Some("RS_Mod.RS_Op"))
            .text_elem("success", "Always")
            .text_elem("param", "7")
            .build();
        let (op, sink) = parse_one(node.clone());
        assert_eq!(op.success, Success::Always);
        assert!(op.is_unknown());
        assert_eq!(op.raw(), Some(&node));
        assert_eq!(sink.count(&diag_codes::PATCH_UNKNOWN_CLASS), 1);
        assert_eq!(sink.count(&diag_codes::PATCH_UNKNOWN_FIELD), 1);
    }

    #[test]
    fn a_missing_class_is_the_base_operation() {
        let (op, sink) = parse_one(op_node(None).text_elem("xpath", "/x").build());
        assert!(matches!(op.kind, PatchKind::Base));
        assert_eq!(op.display_class(), "PatchOperation");
        assert_eq!(sink.count(&diag_codes::PATCH_UNKNOWN_FIELD), 1);
    }

    #[test]
    fn invalid_success_and_order_values_are_reported_and_ignored() {
        let node = op_node(Some("PatchOperationAdd"))
            .text_elem("success", "Maybe")
            .text_elem("order", "Middle")
            .build();
        let (op, sink) = parse_one(node);
        assert_eq!(op.success, Success::Normal);
        assert_eq!(sink.count(&diag_codes::PATCH_FIELD_PARSE_ERROR), 2);
    }

    #[test]
    fn list_entries_honour_may_require() {
        let li = |class: &str, attr: Option<(&str, &str)>| {
            let mut b = NodeBuilder::new("li").attr("Class", class);
            if let Some((k, v)) = attr {
                b = b.attr(k, v);
            }
            b.build()
        };
        let node = op_node(Some("PatchOperationSequence"))
            .child(
                NodeBuilder::new("operations")
                    .child(li("PatchOperationTest", None))
                    .child(li("PatchOperationTest", Some(("MayRequire", "rs.off"))))
                    .child(li("PatchOperationTest", Some(("MayRequire", "RS.ON"))))
                    .child(li(
                        "PatchOperationTest",
                        Some(("MayRequireAnyOf", "rs.off, rs.on")),
                    ))
                    .child(li("PatchOperationTest", Some(("MayRequire", ""))))
                    .child(NodeBuilder::new("notli")),
            )
            .build();
        let (op, sink) = parse_one(node);
        match op.kind {
            PatchKind::Sequence {
                operations: Some(ops),
            } => assert_eq!(ops.len(), 4),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(sink.count(&diag_codes::PATCH_LIST_ITEM_NOT_LI), 1);
    }

    #[test]
    fn a_missing_operations_list_is_none_and_an_empty_one_is_some_empty() {
        let (missing, _) = parse_one(op_node(Some("PatchOperationSequence")).build());
        assert!(matches!(
            missing.kind,
            PatchKind::Sequence { operations: None }
        ));
        let (empty, _) = parse_one(
            op_node(Some("PatchOperationSequence"))
                .child(NodeBuilder::new("operations"))
                .build(),
        );
        assert!(matches!(
            empty.kind,
            PatchKind::Sequence {
                operations: Some(ref v)
            } if v.is_empty()
        ));
    }

    #[test]
    fn patch_files_report_bad_roots_and_elements() {
        let registry = CustomRegistry::new();
        let active = ActiveSet::new();
        let env = ParseEnv {
            registry: &registry,
            active: &active,
        };
        let mut sink = DiagSink::new();
        let bad_root = NodeBuilder::new("Defs").build();
        assert!(parse_patch_file(&bad_root, &env, None, None, &mut sink).is_empty());
        let root = NodeBuilder::new("Patch")
            .child(NodeBuilder::new("Other"))
            .child(op_node(Some("PatchOperationTest")))
            .build();
        let ops = parse_patch_file(&root, &env, Some(ModIdx(1)), Some(FileId(2)), &mut sink);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].file, Some(FileId(2)));
        assert_eq!(sink.count(&diag_codes::PATCH_BAD_ROOT), 1);
        assert_eq!(sink.count(&diag_codes::PATCH_BAD_ELEMENT), 1);
    }

    #[test]
    fn describe_collapses_white_space() {
        let node = op_node(Some("PatchOperationRemove"))
            .text_elem("xpath", "\n  Defs/RS_A\n   [defName='x']\n")
            .build();
        let (op, _) = parse_one(node);
        assert_eq!(
            op.describe(),
            "PatchOperationRemove(Defs/RS_A [defName='x'])"
        );
    }

    #[test]
    fn ops_round_trip_through_json() {
        let node = op_node(Some("PatchOperationFindMod"))
            .child(NodeBuilder::new("mods").child(NodeBuilder::new("li").text("RS Mod")))
            .child(
                NodeBuilder::new("match")
                    .attr("Class", "PatchOperationTest")
                    .text_elem("xpath", "Defs"),
            )
            .build();
        let (op, _) = parse_one(node);
        let json = serde_json::to_string(&op).unwrap();
        let back: PatchOp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, op);
    }

    #[test]
    fn parse_patches_without_context_gives_unknown_for_custom_classes() {
        let ops = parse_patches(&[op_node(Some("RS.Custom")).build()]);
        assert!(ops[0].is_unknown());
    }
}
