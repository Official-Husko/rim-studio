//! Recognition of the dominant patch shape `Defs/Type[defName="x"]`, so the def engine can answer
//! it through a hash index instead of scanning every definition.
//!
//! The analysis is purely syntactic. A hint is only valid for evaluation with the document node as
//! the context node (the way patches are applied), where `Defs/...` and `/Defs/...` mean the same.
//! The lookup also assumes that the document element is named `Defs`, as in the unified document
//! the game builds; a caller that cannot guarantee this should fall back to full evaluation.
//! Recognised, with literal strings only:
//!
//! - `Defs/Type[defName="x"]` and the reversed comparison `["x"=defName]`
//! - `Defs/Type[defName="x" or defName="y"]`
//! - unions of such paths, `Defs/A[defName="x"] | Defs/B[defName="y"]`
//!
//! Anything the index cannot answer exactly (other predicates, `and`, a bare string literal
//! operand of `or`, other steps before `Type`) gives no hint. Trailing steps after the predicate are
//! reported by [`IndexPlan::rest`] so a caller can look the definitions up and then continue with
//! [`crate::XPath::select_with_defs`].

use crate::ast::{Axis, BinOp, Expr, NodeTest, PathBase, PathExpr, Step};

/// One definition: its type element name and its `defName` text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefKey {
    /// The element name of the definition, for example the `ThingDef` of `Defs/ThingDef`.
    pub def_type: String,
    /// The text the `defName` child must equal.
    pub name: String,
}

/// What the expression selects, as a lookup the def engine can serve from an index.
///
/// The definitions are the children of the document element `Defs` that have the type's element name
/// and a `defName` child element whose string value equals the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexHint {
    /// One definition by type and name.
    DefByName {
        /// The element name of the definition.
        def_type: String,
        /// The `defName` text.
        name: String,
    },
    /// Several definitions (an `or` chain or a union). Names are unique; the matches must be
    /// returned in document order without duplicates, which is the caller's responsibility when
    /// the keys name different types.
    DefsByNames(Vec<DefKey>),
}

impl IndexHint {
    /// The lookup keys, in expression order.
    #[must_use]
    pub fn keys(&self) -> Vec<DefKey> {
        match self {
            Self::DefByName { def_type, name } => vec![DefKey {
                def_type: def_type.clone(),
                name: name.clone(),
            }],
            Self::DefsByNames(keys) => keys.clone(),
        }
    }
}

/// A hint plus the steps that remain after the indexed definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexPlan<'a> {
    /// The definitions to look up.
    pub hint: IndexHint,
    /// The steps to apply to the definitions found, empty when the hint answers the whole
    /// expression.
    pub rest: &'a [Step],
}

/// Analyses an expression; see the module documentation.
#[must_use]
pub fn plan(expr: &Expr) -> Option<IndexPlan<'_>> {
    match expr {
        Expr::Path(p) => {
            let (keys, rest) = analyse_path(p)?;
            Some(IndexPlan {
                hint: make_hint(keys),
                rest,
            })
        }
        Expr::Chain { first, rest } if rest.iter().all(|(op, _)| *op == BinOp::Union) => {
            let mut keys = Vec::new();
            for e in std::iter::once(&**first).chain(rest.iter().map(|(_, e)| e)) {
                let Expr::Path(p) = e else { return None };
                let (k, tail) = analyse_path(p)?;
                if !tail.is_empty() {
                    return None;
                }
                keys.extend(k);
            }
            Some(IndexPlan {
                hint: make_hint(dedup(keys)),
                rest: &[],
            })
        }
        _ => None,
    }
}

fn dedup(keys: Vec<DefKey>) -> Vec<DefKey> {
    let mut out: Vec<DefKey> = Vec::with_capacity(keys.len());
    for k in keys {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

fn make_hint(mut keys: Vec<DefKey>) -> IndexHint {
    if keys.len() == 1
        && let Some(k) = keys.pop()
    {
        return IndexHint::DefByName {
            def_type: k.def_type,
            name: k.name,
        };
    }
    IndexHint::DefsByNames(keys)
}

fn plain_child(step: &Step) -> Option<&str> {
    match (&step.axis, &step.test) {
        (
            Axis::Child,
            NodeTest::Name {
                prefix: None,
                local,
            },
        ) => Some(local.as_str()),
        _ => None,
    }
}

fn analyse_path(p: &PathExpr) -> Option<(Vec<DefKey>, &[Step])> {
    if !matches!(p.base, PathBase::Root | PathBase::Context) {
        return None;
    }
    let [defs, typ, rest @ ..] = p.steps.as_slice() else {
        return None;
    };
    if plain_child(defs)? != "Defs" || !defs.predicates.is_empty() {
        return None;
    }
    let def_type = plain_child(typ)?;
    let [pred] = typ.predicates.as_slice() else {
        return None;
    };
    let mut names = Vec::new();
    collect_names(pred, &mut names)?;
    let keys = dedup(
        names
            .into_iter()
            .map(|name| DefKey {
                def_type: def_type.to_owned(),
                name,
            })
            .collect(),
    );
    Some((keys, rest))
}

/// Collects the literals of `defName = "x" (or defName = "y")*`.
fn collect_names(pred: &Expr, out: &mut Vec<String>) -> Option<()> {
    match pred {
        Expr::Chain { first, rest }
            if !rest.is_empty() && rest.iter().all(|(op, _)| *op == BinOp::Or) =>
        {
            for e in std::iter::once(&**first).chain(rest.iter().map(|(_, e)| e)) {
                out.push(name_equality(e)?);
            }
            Some(())
        }
        other => {
            out.push(name_equality(other)?);
            Some(())
        }
    }
}

fn name_equality(e: &Expr) -> Option<String> {
    let Expr::Chain { first, rest } = e else {
        return None;
    };
    let [(BinOp::Eq, right)] = rest.as_slice() else {
        return None;
    };
    let lit = match (is_def_name(first), is_def_name(right)) {
        (true, false) => right,
        (false, true) => &**first,
        _ => return None,
    };
    match lit {
        Expr::Literal(s) => Some(s.clone()),
        _ => None,
    }
}

fn is_def_name(e: &Expr) -> bool {
    if let Expr::Path(PathExpr {
        base: PathBase::Context,
        steps,
    }) = e
        && let [s] = steps.as_slice()
    {
        return s.predicates.is_empty() && plain_child(s) == Some("defName");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use rstest::rstest;

    fn hint(src: &str) -> Option<IndexHint> {
        let e = parse(src).ok()?;
        plan(&e).filter(|p| p.rest.is_empty()).map(|p| p.hint)
    }

    #[rstest]
    #[case("Defs/RS_ThingDef[defName=\"RS_A\"]", "RS_ThingDef", &["RS_A"])]
    #[case("/Defs/RS_ThingDef[defName='RS_A']", "RS_ThingDef", &["RS_A"])]
    #[case("Defs/RS_ThingDef['RS_A' = defName]", "RS_ThingDef", &["RS_A"])]
    #[case("Defs/T[defName='a' or defName='b' or defName='a']", "T", &["a", "b"])]
    fn single_type_shapes_give_a_hint(#[case] src: &str, #[case] ty: &str, #[case] names: &[&str]) {
        let h = hint(src);
        let keys = h.map(|h| h.keys()).unwrap_or_default();
        let got: Vec<(&str, &str)> = keys
            .iter()
            .map(|k| (k.def_type.as_str(), k.name.as_str()))
            .collect();
        let want: Vec<(&str, &str)> = names.iter().map(|n| (ty, *n)).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn one_name_is_def_by_name() {
        assert_eq!(
            hint("Defs/T[defName='a']"),
            Some(IndexHint::DefByName {
                def_type: "T".into(),
                name: "a".into()
            })
        );
    }

    #[test]
    fn unions_of_hint_paths_give_one_hint() {
        let h = hint("Defs/A[defName='x'] | Defs/B[defName='y']");
        assert_eq!(h.map(|h| h.keys().len()), Some(2));
    }

    #[rstest]
    #[case("Defs/T")]
    #[case("Defs/T[defName='a' and x]")]
    #[case("Defs/T[defName='a' or 'b']")]
    #[case("Defs/T[defName='a'][x]")]
    #[case("Defs/T[defName!='a']")]
    #[case("Defs/T[defName=other]")]
    #[case("Defs/T[@defName='a']")]
    #[case("Defs/*[defName='a']")]
    #[case("Defs//T[defName='a']")]
    #[case("Other/T[defName='a']")]
    #[case("Defs/T[defName='a']/x")]
    #[case("Defs/T[defName='a'] | Defs/B")]
    #[case("1")]
    fn other_shapes_give_no_whole_expression_hint(#[case] src: &str) {
        assert_eq!(hint(src), None, "{src}");
    }

    #[test]
    fn trailing_steps_are_reported_as_rest() {
        let e = parse("Defs/T[defName='a']/statBases/Mass");
        let p = e.as_ref().ok().and_then(plan);
        assert_eq!(p.map(|p| p.rest.len()), Some(2));
    }
}
