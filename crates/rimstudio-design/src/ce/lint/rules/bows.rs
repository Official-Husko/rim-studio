//! The rules about bow conversions (CEP030 to CEP033), called from the gun conversion check of
//! [`super`].
//!
//! A conversion is a bow when its tags, or the projectile of its verb, say bow
//! ([`crate::ce::reader::bows::is_bow_shape`]). A bow has no magazine, so CEP008 does not ask for one; these
//! rules check what a bow has instead.

use rimstudio_core::tree::Node;

use super::super::codes::{CEP030, CEP031, CEP032, CEP033};
use super::Cx;
use crate::ce::patchgen::bow::serves_arrows;
use crate::ce::reader::MakeGunSpec;
use crate::reader::access::text_of;

impl Cx<'_> {
    /// Checks one bow conversion: the ammo set serves arrows (CEP030), the fire modes are empty (CEP031),
    /// the run and gun flag is false (CEP032) and the ammo spawn count is set (CEP033).
    pub(super) fn bow_rules(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        spec: &MakeGunSpec,
        pointer: &str,
        def: &str,
    ) {
        let ammo_field = |name: &str| -> Option<String> {
            spec.ammo_user
                .as_ref()?
                .iter()
                .find(|n| n.tag == name)
                .and_then(text_of)
        };
        if let Some(set) = ammo_field("ammoSet")
            && let Some(info) = self.model.ammo_set(&set)
        {
            let used = self
                .model
                .bows()
                .any(|b| b.ammo_set.as_deref() == Some(set.as_str()));
            if !serves_arrows(info, used) {
                self.push(
                    key,
                    &CEP030,
                    path,
                    &format!("{pointer}/AmmoUser/ammoSet"),
                    &[("def", def), ("set", &set)],
                );
            }
        }
        if let Some(modes) = &spec.fire_modes
            && modes
                .iter()
                .any(|m: &Node| m.tag != "aiAimMode" || text_of(m).is_none_or(|t| t != "AimedShot"))
        {
            self.push(
                key,
                &CEP031,
                path,
                &format!("{pointer}/FireModes"),
                &[("def", def)],
            );
        }
        if spec.allow_with_run_and_gun != Some(false) {
            self.push(key, &CEP032, path, pointer, &[("def", def)]);
        }
        if spec.ammo_user.is_some()
            && ammo_field("AmmoGenPerMagOverride").is_none()
            && ammo_field("magazineSize").is_none()
        {
            self.push(
                key,
                &CEP033,
                path,
                &format!("{pointer}/AmmoUser"),
                &[("def", def)],
            );
        }
    }
}
