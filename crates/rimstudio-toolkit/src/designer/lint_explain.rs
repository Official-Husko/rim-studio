//! The plain English explanation of each Combat Extended lint rule: what it checks and how to fix it.
//!
//! The text is shown next to a finding of `designer_lint_files`. The short message of a finding says what is
//! wrong in this file; the explanation says why the rule exists. The table is keyed by rule id (`CEP013`).

/// The explanation of a rule, `None` for an id that has none.
#[must_use]
pub fn explain(rule_id: &str) -> Option<&'static str> {
    let text = match rule_id {
        "CEP001" => {
            "FindMod compares the display name of a mod, never its package id. An entry that looks like \
             a package id matches nothing, so every operation it guards is skipped without a message. \
             Write the exact name of the mod."
        }
        "CEP002" => {
            "FindMod compares names exactly, letter case included. An entry that spells the mod name \
             differently matches nothing. Copy the name from the name field of the mod's About file."
        }
        "CEP003" => {
            "FindMod does not trim its entries. A space before or after the name makes the comparison \
             fail, so the guarded operations never run. Remove the white space."
        }
        "CEP004" => {
            "A file that loads without Combat Extended cannot use a Combat Extended class: the class \
             does not resolve and the game logs an error on every start. Put the file in a folder that \
             LoadFolders.xml loads only when Combat Extended is active, or guard the operation."
        }
        "CEP005" => {
            "MayRequire is honoured only on list items. On an operation the game ignores it, so the \
             operation runs whether or not the other mod is active. Guard the operation with a find mod \
             or a conditional operation instead."
        }
        "CEP007" => {
            "Converting a def twice appends its components, verbs and tags a second time. Keep one \
             conversion per def, or change the existing one with field operations."
        }
        "CEP008" => {
            "A gun conversion has to give the pieces Combat Extended reads: the ammo user, the fire modes, \
             the verb properties and the key fields. A missing piece leaves the weapon half converted \
             and often unusable. Add the missing piece."
        }
        "CEP009" => {
            "The shooting verb of a converted gun has to use a Combat Extended verb class, or the game \
             falls back to the vanilla shooting code and ignores the ballistics. Use the verb class that \
             the converted guns of Combat Extended use."
        }
        "CEP010" => {
            "A class attribute names a type that the installed Combat Extended does not have. The game \
             cannot load the element. Check the spelling, or the version of Combat Extended the patch \
             was written for."
        }
        "CEP011" => {
            "A tool of the Combat Extended class that gives neither sharp nor blunt penetration is \
             stopped by every armor. Add the penetration values."
        }
        "CEP012" => {
            "Replacing the tools of a weapon without the Combat Extended tool class loads plain tools \
             with no penetration. Give each replaced tool the Combat Extended class."
        }
        "CEP013" => {
            "The ammo set has to be defined by Combat Extended, the patched mod or this project. An \
             unknown ammo set leaves the gun without ammunition. Use an existing set or define it."
        }
        "CEP014" => {
            "The default projectile has to be defined by Combat Extended, the game, the patched mod or \
             this project. An unknown projectile makes the gun fire nothing. Use an existing projectile \
             or define it."
        }
        "CEP015" => {
            "A field name that the section does not know is ignored by the game, usually a spelling \
             mistake. Check the name against the converted defs of Combat Extended."
        }
        "CEP016" => {
            "A weapon tag that the installed Combat Extended does not use has no effect on its AI \
             class or on the ammunition pawns carry. Pick a tag from the converted weapons of the same \
             class."
        }
        "CEP017" => {
            "A patch file is a Patch element that holds Operation elements. A file the game cannot \
             parse, or that has another shape, is skipped by the game with a log message. Fix the file \
             or remove it."
        }
        "CEP018" => {
            "LoadFolders.xml decides which folders the game reads. A patch in a folder it never loads \
             is never applied. Add the folder to the block of the game version, or move the file."
        }
        "CEP019" => {
            "The game compares an IfModActive id with the ids of the active mods and ignores a suffix on \
             the active mod's id, not on the id you wrote. An id with a suffix of its own never matches. \
             Write the plain package id."
        }
        "CEP020" => {
            "Folder and file names are case sensitive on Linux and macOS. A path that differs in case \
             from the folder the game expects works on Windows and breaks elsewhere. Use the exact \
             case."
        }
        "CEP021" => {
            "The operation repeats an earlier one exactly. Applied twice it adds the same entries twice \
             or fails the second time. Remove the repeat."
        }
        "CEP022" => {
            "The xpath cannot be parsed, so the operation never matches and the game logs an error. Fix \
             the expression."
        }
        "CEP023" => {
            "A gun conversion that nothing guards runs again when another patch or a second copy of the \
             file has already converted the def, and appends a second verb, ammo component and set of \
             tags. Guard it with a conditional operation that tests for the ammo user."
        }
        "CEP024" => {
            "A burst weapon whose fire modes give no aimed burst size fires the whole burst when aimed. \
             Add aimedBurstShotCount to the fire modes if that is not what you want."
        }
        "CEP030" => {
            "A bow shoots arrows or bolts, so its ammo set should serve those. Another kind of set \
             gives the bow ammunition that does not fit it."
        }
        "CEP031" => {
            "Bows are usually converted with an empty fire modes element, because a bow has no burst or \
             automatic mode. Check that the fire modes are what you want."
        }
        "CEP032" => {
            "Without AllowWithRunAndGun set to false a bow can be fired while the pawn runs, when the Run \
             and Gun mod is active. Set it to false unless that is intended."
        }
        "CEP033" => {
            "Without an ammo spawn count or a magazine size, pawns spawn with a single arrow per \
             magazine. Set AmmoGenPerMagOverride or a magazine size."
        }
        "CEP040" => {
            "An under barrel unit is a second gun on the weapon. Without its own ammo set and default \
             projectile it has nothing to fire when the wielder switches to it."
        }
        "CEP041" => {
            "The ammo set of the unit is not defined by Combat Extended or by any mod in the load order, \
             so the unit cannot be loaded. Check the spelling or add the mod that defines the set."
        }
        "CEP042" => {
            "The default projectile of the unit should be one of the projectiles of the unit's own ammo \
             set; otherwise the loaded ammo and the shot disagree."
        }
        "CEP043" => {
            "The vanilla ability component of the weapon stands in for the plain equippable component. \
             When the patch replaces it, the plain component must be added back or the weapon cannot be \
             equipped."
        }
        "CEP044" => {
            "Every attachment link names the attachment def that may be fitted. A link without one does \
             nothing."
        }
        "CEP045" => {
            "The attachment def of the link is not defined by Combat Extended, by the project or by a \
             mod in the load order. Check the spelling, or add the mod that defines it."
        }
        "CEP046" => {
            "A weapon platform with no attachment links and no default graphic parts accepts no \
             attachment. Add at least one link, or leave the weapon a plain gun."
        }
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_of_the_registry_has_an_explanation() {
        for info in rimstudio_design::ce::lint::REGISTRY {
            let id = rimstudio_design::ce::lint::rule_id(info.code).unwrap_or_default();
            assert!(explain(&id).is_some(), "{id} has no explanation");
        }
    }

    #[test]
    fn explanations_follow_the_house_style() {
        for info in rimstudio_design::ce::lint::REGISTRY {
            let id = rimstudio_design::ce::lint::rule_id(info.code).unwrap_or_default();
            let text = explain(&id).unwrap_or_default();
            assert!(
                !text.contains('\u{2014}') && !text.contains('\u{2013}'),
                "{id}"
            );
            assert!(text.ends_with('.'), "{id}");
        }
    }

    #[test]
    fn an_unknown_rule_has_none() {
        assert_eq!(explain("CEP999"), None);
    }
}
