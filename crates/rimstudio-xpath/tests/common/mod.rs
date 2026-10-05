//! Shared fixtures and helpers for the integration tests. All names and numbers are fictional.
#![allow(dead_code, unreachable_pub, clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::{ArenaDoc, NodeBuilder, NodeId, NodeKind, OriginId};
use rimstudio_xpath::{Value, XNode, XPath, coerce};

/// Builds an arena document from a builder rooted at the document element.
pub fn doc_of(root: NodeBuilder) -> ArenaDoc {
    ArenaDoc::from_node(&root.build(), OriginId::NONE).unwrap()
}

/// A small fictional Defs document. Counts used by the tests:
///
/// - 7 `RS_ThingDef` elements (one abstract, two without defName, two sharing `RS_RifleA`)
/// - 1 `RS_RecipeDef`, 1 `RS_KindDef`
pub fn defs_doc() -> ArenaDoc {
    let b = NodeBuilder::new("Defs")
        .elem("RS_ThingDef", |b| {
            b.attr("Name", "RS_BaseGun")
                .attr("Abstract", "True")
                .text_elem("label", "base")
                .elem("statBases", |b| b.text_elem("Mass", "1"))
        })
        .elem("RS_ThingDef", |b| {
            b.attr("ParentName", "RS_BaseGun")
                .text_elem("defName", "RS_RifleA")
                .text_elem("label", "rifle a")
                .elem("statBases", |b| {
                    b.text_elem("Mass", "3.5").text_elem("MarketValue", "100")
                })
                .elem("tools", |b| {
                    b.elem("li", |b| {
                        b.text_elem("label", "stock").text_elem("power", "9")
                    })
                    .elem("li", |b| {
                        b.text_elem("label", "barrel").text_elem("power", "5")
                    })
                })
                .elem("comps", |b| {
                    b.elem("li", |b| b.attr("Class", "RS_CompA").text_elem("x", "1"))
                        .elem("li", |b| b.attr("Class", "RS_CompB"))
                })
                .elem("weaponTags", |b| b.li("Gun").li("Ranged"))
        })
        .elem("RS_ThingDef", |b| {
            b.text_elem("defName", "RS_RifleB")
                .elem("statBases", |b| b.text_elem("Mass", "4"))
                .elem("comps", |b| {
                    b.elem("li", |b| b.attr("Class", "RS_CompA").text_elem("x", "2"))
                })
        })
        .elem("RS_ThingDef", |b| {
            b.text_elem("defName", "RS_Knife")
                .text_elem("label", "knife")
                .elem("tools", |b| {
                    b.elem("li", |b| {
                        b.text_elem("label", "blade").text_elem("power", "12")
                    })
                })
                .elem("weaponTags", |b| b.li("Melee"))
        })
        .elem("RS_RecipeDef", |b| {
            b.text_elem("defName", "RS_Make")
                .text_elem("label", "make rifle")
                .elem("recipeUsers", |b| b.li("RS_Bench"))
        })
        .elem("RS_ThingDef", |b| {
            b.text_elem("defName", "RS_RifleA")
                .text_elem("label", "duplicate")
        })
        .elem("RS_ThingDef", |b| b.text_elem("label", "nameless"))
        .elem("RS_ThingDef", |b| {
            b.text_elem("defName", "RS_Empty").empty_elem("label")
        })
        .elem("RS_KindDef", |b| {
            b.text_elem("defName", "RS_Kind")
                .elem("weights", |b| b.li("1").li("2").li("3"))
        });
    doc_of(b)
}

/// The document of the examples in the XPath recommendation (fictional content).
pub fn book_doc() -> ArenaDoc {
    let para = |t: &str, ty: Option<&str>| {
        let mut b = NodeBuilder::new("para");
        if let Some(ty) = ty {
            b = b.attr("type", ty);
        }
        b.text(t)
    };
    let b = NodeBuilder::new("doc")
        .elem("chapter", |b| {
            b.elem("title", |b| b.text("Introduction"))
                .child(para("p1", None))
                .child(para("p2", Some("warning")))
                .child(para("p3", Some("warning")))
                .child(para("p4", Some("warning")))
                .child(para("p5", Some("warning")))
                .child(para("p6", Some("warning")))
                .child(para("p7", Some("warning")))
        })
        .elem("chapter", |b| {
            b.elem("title", |b| b.text("Body"))
                .child(para("q1", Some("note")))
                .elem("section", |b| {
                    b.child(para("s1", Some("warning")))
                        .child(para("s2", None))
                        .elem("olist", |b| {
                            b.elem("item", |b| b.text("i1"))
                                .elem("item", |b| b.text("i2"))
                        })
                })
        })
        .elem("chapter", |b| {
            b.elem("title", |b| b.text("Conclusion"))
                .elem("div", |b| b.child(para("d1", None)))
        })
        .elem("employee", |b| {
            b.attr("name", "RS_Ann")
                .attr("secretary", "RS_Bo")
                .attr("assistant", "RS_Cy")
        })
        .elem("employee", |b| {
            b.attr("name", "RS_Dee").attr("secretary", "RS_Ed")
        })
        .elem("employee", |b| b.attr("name", "RS_Fay"));
    doc_of(b)
}

/// A readable description of a node.
pub fn describe(doc: &ArenaDoc, n: XNode) -> String {
    match n {
        XNode::Attr { owner, index } => {
            let (k, v) = doc.attr_at(owner, index as usize).unwrap();
            format!("@{k}={v}")
        }
        XNode::Node(id) => describe_id(doc, id),
    }
}

pub fn describe_id(doc: &ArenaDoc, id: NodeId) -> String {
    match doc.kind(id).unwrap() {
        NodeKind::Document => "#doc".to_owned(),
        NodeKind::Text => format!("text:{}", doc.text(id).unwrap()),
        NodeKind::Element => {
            let tag = doc.name(id).unwrap();
            if let Some(d) = doc.child_named(id, "defName") {
                format!("{tag}#{}", doc.string_value(d).unwrap())
            } else if let Some(t) = doc.attr(id, "type") {
                format!("{tag}[{t}]:{}", doc.string_value(id).unwrap())
            } else if tag == "para" || tag == "item" || tag == "title" {
                format!("{tag}:{}", doc.string_value(id).unwrap())
            } else {
                tag.to_owned()
            }
        }
    }
}

/// Evaluates with the document node as context and describes the selected nodes.
pub fn select(doc: &ArenaDoc, expr: &str) -> Vec<String> {
    let xp = XPath::parse(expr).unwrap_or_else(|e| panic!("{expr}: {e}"));
    let nodes = xp
        .select_nodes(doc, doc.root())
        .unwrap_or_else(|e| panic!("{expr}: {e}"));
    nodes.into_iter().map(|n| describe(doc, n)).collect()
}

/// Number of nodes selected from the document node.
pub fn count(doc: &ArenaDoc, expr: &str) -> usize {
    select(doc, expr).len()
}

pub fn eval(doc: &ArenaDoc, expr: &str) -> Value {
    XPath::parse(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e}"))
        .eval_document(doc)
        .unwrap_or_else(|e| panic!("{expr}: {e}"))
}

pub fn eval_string(doc: &ArenaDoc, expr: &str) -> String {
    coerce::to_string(doc, &eval(doc, expr))
}

pub fn eval_bool(doc: &ArenaDoc, expr: &str) -> bool {
    coerce::to_boolean(&eval(doc, expr))
}

pub fn eval_number(doc: &ArenaDoc, expr: &str) -> f64 {
    coerce::to_number(doc, &eval(doc, expr))
}

/// An empty document, for expressions that need no nodes.
pub fn empty_doc() -> ArenaDoc {
    ArenaDoc::new()
}
