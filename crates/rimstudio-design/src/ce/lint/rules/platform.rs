//! The rules about weapon platforms and under barrel units (CEP040 to CEP046), called from the operation
//! visitor of [`super`].
//!
//! A platform is a gun conversion operation that carries `isWeaponPlatform`, attachment links or default
//! graphic parts. An under barrel unit is a component entry of the under barrel class inside the value of an
//! add or a replace operation. The rules read only what the patch says plus the ammo sets, projectiles and
//! attachment defs of the user's install; none of them needs a table of values.

use rimstudio_core::tree::Node;

use super::super::codes::{CEP040, CEP041, CEP042, CEP043, CEP044, CEP045, CEP046};
use super::{Cx, class_attr, same_class, walk};
use crate::ce::patchgen::platform::{EQUIPPABLE_ABILITY_PREFIX, EQUIPPABLE_COMP};
use crate::ce::reader::MakeGunSpec;
use crate::reader::access::text_of;

/// True when the conversion operation does nothing but make the def a platform (the update mode form): no
/// verb, no components, no stats and no tags, so the checks about a missing gun conversion do not apply.
pub(super) fn platform_only(spec: &MakeGunSpec) -> bool {
    let asks_platform = spec.is_weapon_platform
        || spec.attachment_links.is_some()
        || spec.default_graphic_parts.is_some();
    asks_platform
        && spec.properties.is_empty()
        && spec.ammo_user.is_none()
        && spec.fire_modes.is_none()
        && spec.stat_bases.is_empty()
        && spec.weapon_tags.is_empty()
}

fn child_text_of(node: &Node, parent: &str, tag: &str) -> Option<String> {
    node.child(parent)?.child(tag).and_then(text_of)
}

impl Cx<'_> {
    /// The platform checks of one conversion operation: a link without an attachment (CEP044), a link to an
    /// attachment def that does not exist (CEP045), and a platform that nothing can be fitted to (CEP046).
    pub(super) fn platform_rules(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        spec: &MakeGunSpec,
        pointer: &str,
        def: &str,
    ) {
        let links = spec.attachment_links.as_deref().unwrap_or_default();
        let parts = spec.default_graphic_parts.as_deref().unwrap_or_default();
        if spec.is_weapon_platform && links.is_empty() && parts.is_empty() {
            self.push(key, &CEP046, path, pointer, &[("def", def)]);
        }
        let knows_any =
            !self.model.platform.attachments.is_empty() || !self.ctx.known_defs.is_empty();
        for (i, link) in links.iter().enumerate() {
            let field = format!("{pointer}/attachmentLinks/li[{}]/attachment", i + 1);
            match link.child("attachment").and_then(text_of) {
                None => self.push(key, &CEP044, path, &field, &[("def", def)]),
                Some(name) => {
                    if knows_any
                        && !self.model.platform.knows_attachment(&name)
                        && !self.ctx.known_defs.contains(&name)
                    {
                        self.push(
                            key,
                            &CEP045,
                            path,
                            &field,
                            &[("def", def), ("attachment", &name)],
                        );
                    }
                }
            }
        }
    }

    /// The under barrel checks of one operation: every unit entry in its value (CEP040, CEP041, CEP042), and
    /// the replacement of an ability component that does not bring the plain equippable component back
    /// (CEP043).
    pub(super) fn under_barrel_rules(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        op: &Node,
        pointer: &str,
    ) {
        let Some(value) = op.child("value") else {
            return;
        };
        let class = self.model.classes.under_barrel_comp.clone();
        let mut all = vec![(format!("{pointer}/value"), value)];
        walk(value, &format!("{pointer}/value"), &mut all);
        let units: Vec<(String, &Node)> = all
            .into_iter()
            .filter(|(_, n)| class_attr(n).is_some_and(|c| same_class(c, &class)))
            .collect();
        if units.is_empty() {
            return;
        }
        let xpath = op.child_text("xpath").unwrap_or_default();
        let def = def_of_xpath(xpath);
        for (p, unit) in &units {
            self.unit_rules(key, path, unit, p, &def);
        }
        if xpath.contains(EQUIPPABLE_ABILITY_PREFIX) {
            let brings_back = value.children_named("li").any(|li| {
                li.child("compClass").and_then(text_of).as_deref() == Some(EQUIPPABLE_COMP)
            });
            if !brings_back {
                self.push(
                    key,
                    &CEP043,
                    path,
                    &format!("{pointer}/value"),
                    &[("def", &def)],
                );
            }
        }
    }

    fn unit_rules(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        unit: &Node,
        pointer: &str,
        def: &str,
    ) {
        if unit.elements().next().is_none() {
            // The bare slot form (a unique weapon's trait carrier) has no unit data to check.
            return;
        }
        // A unit that shares the main gun's ammo holder has no ammo set of its own.
        let one_holder = unit
            .child("oneAmmoHolder")
            .and_then(text_of)
            .is_some_and(|t| t.eq_ignore_ascii_case("true"));
        let set = child_text_of(unit, "propsUnderBarrel", "ammoSet");
        let projectile = child_text_of(unit, "verbPropsUnderBarrel", "defaultProjectile");
        for (what, found, at) in [
            (
                "an ammo set",
                set.is_some() || one_holder,
                "propsUnderBarrel",
            ),
            (
                "a default projectile",
                projectile.is_some(),
                "verbPropsUnderBarrel",
            ),
        ] {
            if !found {
                self.push(
                    key,
                    &CEP040,
                    path,
                    &format!("{pointer}/{at}"),
                    &[("def", def), ("what", what)],
                );
            }
        }
        let Some(set) = set else { return };
        if self.model.ammo_sets.is_empty() {
            return;
        }
        match self.model.ammo_set(&set) {
            None => {
                if !self.ctx.known_defs.contains(&set) {
                    self.push(
                        key,
                        &CEP041,
                        path,
                        &format!("{pointer}/propsUnderBarrel/ammoSet"),
                        &[("def", def), ("set", &set)],
                    );
                }
            }
            Some(info) => {
                if let Some(p) = projectile
                    && !info.has_projectile(&p)
                {
                    self.push(
                        key,
                        &CEP042,
                        path,
                        &format!("{pointer}/verbPropsUnderBarrel/defaultProjectile"),
                        &[("def", def), ("projectile", &p), ("set", &set)],
                    );
                }
            }
        }
    }
}

/// The def name an xpath of the form `Defs/ThingDef[defName="X"]/...` addresses, or the whole xpath when it
/// has another shape (the message then names the xpath).
fn def_of_xpath(xpath: &str) -> String {
    let marker = "defName=";
    let Some(at) = xpath.find(marker) else {
        return xpath.to_owned();
    };
    let rest = &xpath[at + marker.len()..];
    let mut chars = rest.chars();
    let Some(quote) = chars.next().filter(|c| *c == '"' || *c == '\'') else {
        return xpath.to_owned();
    };
    let name: String = chars.take_while(|c| *c != quote).collect();
    if name.is_empty() {
        xpath.to_owned()
    } else {
        name
    }
}
