//! Tests of the assembly reader and the type table builder.
//!
//! The images are written by `assembly_support` (fictional types, no game data). The two tests at
//! the end read a real install, are `#[ignore]`, only read, and need `RIMSTUDIO_GAME_DIR` (and
//! `RIMSTUDIO_CE_DIR` for the second). Run them with
//! `cargo test -p rimstudio-defs --test assembly -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod assembly_support;

use assembly_support::{ABSTRACT, Extends, INTERFACE, Image, ImageBuilder, SEALED};
use camino::Utf8PathBuf;
use proptest::prelude::*;
use rimstudio_defs::assembly::{
    AssemblyError, AssemblyTypes, BaseType, ExternalType, build_type_table, managed_dirs,
    read_assembly,
};
use rimstudio_defs::type_table::TypeTable;
use rstest::rstest;

const ROOT: &str = "Verse.Def";

fn find<'a>(asm: &'a AssemblyTypes, full: &str) -> &'a rimstudio_defs::assembly::TypeDefInfo {
    asm.types
        .iter()
        .find(|t| t.full_name() == full)
        .unwrap_or_else(|| panic!("type {full} not read"))
}

fn external(ns: &str, name: &str, assembly: &str) -> BaseType {
    BaseType::External(ExternalType {
        namespace: ns.into(),
        name: name.into(),
        assembly: Some(assembly.into()),
    })
}

/// The fictional game assembly of the fixtures.
fn game_builder() -> ImageBuilder {
    let mut b = ImageBuilder::new("RS_Game");
    let corlib = b.assembly_ref("mscorlib");
    let object = b.type_ref(corlib, "System", "Object");
    let editable = b.add_type("Verse", "RS_Editable", 0, Extends::Ref(object)); // 0
    let def = b.add_type("Verse", "Def", 0, Extends::Def(editable)); // 1
    b.add_type("Verse", "RS_ThingDef", 0, Extends::Def(def)); // 2
    b.add_type("", "RS_AbstractDef", ABSTRACT, Extends::Def(def)); // 3
    b.add_type("Verse", "RS_Plain", 0, Extends::Def(editable)); // 4
    b.add_type("RimWorld", "RS_Dup", 0, Extends::Def(def)); // 5
    let outer = b.add_type("Verse", "RS_Outer", 0, Extends::Ref(object)); // 6
    b.add_nested(outer, "RS_Inner", Extends::Def(def)); // 7
    b.add_type("Verse", "RS_IFace", INTERFACE, Extends::None); // 8
    let generic = b.add_type("Verse", "RS_Gen`1", ABSTRACT, Extends::Def(def)); // 9
    b.add_type("Verse", "RS_GenDef", SEALED, Extends::GenericDef(generic)); // 10
    b
}

/// The fictional mod assembly of the fixtures, referencing the game assembly.
fn mod_builder() -> ImageBuilder {
    let mut b = ImageBuilder::new("RS_Mod");
    let game = b.assembly_ref("RS_Game");
    let corlib = b.assembly_ref("mscorlib");
    let thing = b.type_ref(game, "Verse", "RS_ThingDef");
    let def = b.type_ref(game, "Verse", "Def");
    let generic = b.type_ref(game, "Verse", "RS_Gen`1");
    let outer = b.type_ref(game, "Verse", "RS_Outer");
    let inner = b.nested_type_ref(outer, "RS_Inner");
    let missing = b.type_ref(corlib, "Elsewhere", "RS_Missing");
    let object = b.type_ref(corlib, "System", "Object");
    b.add_type("RS_Mod", "RS_AmmoDef", 0, Extends::Ref(thing));
    b.add_type("RS_Mod", "RS_BoxedDef", 0, Extends::GenericRef(generic));
    b.add_type("Verse", "RS_Dup", 0, Extends::Ref(def));
    b.add_type("RS_Mod", "RS_Orphan", 0, Extends::Ref(missing));
    b.add_type("RS_Mod", "RS_Tool", 0, Extends::Ref(object));
    b.add_type("RS_Mod", "RS_InnerDef", 0, Extends::Ref(inner));
    b
}

#[test]
fn reads_names_flags_and_bases_of_a_minimal_assembly() {
    let asm = read_assembly(&game_builder().build()).unwrap();
    assert_eq!(asm.name, "RS_Game");
    assert_eq!(asm.references, vec!["mscorlib"]);
    assert_eq!(asm.skipped, 0);
    // the module type comes first, then the types in the order they were written
    assert_eq!(asm.types.len(), 12);
    assert_eq!(asm.types[0].name, "<Module>");
    assert_eq!(asm.types[0].base, BaseType::None);

    let thing = find(&asm, "Verse.RS_ThingDef");
    assert_eq!(thing.namespace, "Verse");
    assert_eq!(thing.name, "RS_ThingDef");
    assert_eq!(thing.base, BaseType::Local("Verse.Def".into()));
    assert!(!thing.is_abstract() && !thing.is_interface() && !thing.is_sealed());

    let editable = find(&asm, "Verse.RS_Editable");
    assert_eq!(editable.base, external("System", "Object", "mscorlib"));
    assert_eq!(editable.base.full_name().as_deref(), Some("System.Object"));

    let global = find(&asm, "RS_AbstractDef");
    assert_eq!(global.namespace, "");
    assert!(global.is_abstract());

    let iface = find(&asm, "Verse.RS_IFace");
    assert!(iface.is_interface() && iface.is_abstract());
    assert_eq!(iface.base, BaseType::None);

    assert!(find(&asm, "Verse.RS_GenDef").is_sealed());
}

#[test]
fn nested_types_are_named_outer_plus_inner() {
    let asm = read_assembly(&game_builder().build()).unwrap();
    let inner = find(&asm, "Verse.RS_Outer+RS_Inner");
    assert!(inner.is_nested());
    assert_eq!(inner.namespace, "Verse");
    assert_eq!(inner.name, "RS_Outer+RS_Inner");
    assert_eq!(inner.short_name(), "RS_Inner");
    assert_eq!(inner.base, BaseType::Local("Verse.Def".into()));
}

#[test]
fn generic_bases_resolve_to_the_generic_class() {
    let game = read_assembly(&game_builder().build()).unwrap();
    assert_eq!(
        find(&game, "Verse.RS_GenDef").base,
        BaseType::Local("Verse.RS_Gen`1".into())
    );
    let modasm = read_assembly(&mod_builder().build()).unwrap();
    assert_eq!(
        find(&modasm, "RS_Mod.RS_BoxedDef").base,
        external("Verse", "RS_Gen`1", "RS_Game")
    );
}

#[test]
fn a_generic_instance_of_a_value_type_is_not_a_class_base() {
    let mut b = ImageBuilder::new("RS_Odd");
    let asm = b.assembly_ref("RS_Game");
    let value = b.type_ref(asm, "Verse", "RS_Pair`1");
    b.add_type("RS_Odd", "RS_OddDef", 0, Extends::GenericValueRef(value));
    let read = read_assembly(&b.build()).unwrap();
    assert_eq!(find(&read, "RS_Odd.RS_OddDef").base, BaseType::Unresolved);
}

#[test]
fn external_bases_carry_the_referenced_assembly_and_nested_references() {
    let asm = read_assembly(&mod_builder().build()).unwrap();
    assert_eq!(asm.references, vec!["RS_Game", "mscorlib"]);
    assert_eq!(
        find(&asm, "RS_Mod.RS_AmmoDef").base,
        external("Verse", "RS_ThingDef", "RS_Game")
    );
    assert_eq!(
        find(&asm, "RS_Mod.RS_InnerDef").base,
        external("Verse", "RS_Outer+RS_Inner", "RS_Game")
    );
}

#[test]
fn the_pe32_plus_variant_reads_the_same_types() {
    let mut b = game_builder();
    let narrow = read_assembly(&b.build()).unwrap();
    b.plus = true;
    let image = b.build();
    assert_eq!(read_assembly(&image).unwrap(), narrow);
}

#[rstest]
#[case::wide_strings(true, false, false)]
#[case::wide_blobs(false, false, true)]
#[case::wide_guids(false, true, false)]
#[case::all_wide(true, true, true)]
fn heap_size_flags_change_row_sizes_but_not_the_result(
    #[case] strings: bool,
    #[case] guids: bool,
    #[case] blobs: bool,
) {
    let expected = read_assembly(&mod_builder().build()).unwrap();
    let mut b = mod_builder();
    b.wide_strings = strings;
    b.wide_guids = guids;
    b.wide_blobs = blobs;
    assert_eq!(read_assembly(&b.build()).unwrap(), expected);
    let mut g = game_builder();
    let expected = read_assembly(&g.build()).unwrap();
    g.wide_strings = strings;
    g.wide_guids = guids;
    g.wide_blobs = blobs;
    assert_eq!(read_assembly(&g.build()).unwrap(), expected);
}

#[test]
fn the_unoptimized_tables_stream_is_read() {
    let mut b = game_builder();
    b.tables_stream = "#-";
    let image = b.build();
    assert_eq!(
        read_assembly(&image).unwrap(),
        read_assembly(&game_builder().build()).unwrap()
    );
}

#[test]
fn wide_coded_indexes_appear_with_many_type_rows() {
    // 2 bits of tag leave 14 bits: 16384 rows make TypeDefOrRef indexes 4 bytes wide
    let mut b = ImageBuilder::new("RS_Big");
    let corlib = b.assembly_ref("mscorlib");
    let object = b.type_ref(corlib, "System", "Object");
    let root = b.add_type("Verse", "Def", 0, Extends::Ref(object));
    for i in 0..17_000 {
        b.add_type("RS_Big", &format!("RS_T{i}"), 0, Extends::Def(root));
    }
    let asm = read_assembly(&b.build()).unwrap();
    assert_eq!(asm.types.len(), 17_002);
    let last = find(&asm, "RS_Big.RS_T16999");
    assert_eq!(last.base, BaseType::Local("Verse.Def".into()));
}

#[test]
fn wide_table_indexes_appear_with_many_field_rows() {
    // more than 65535 Field rows make the FieldList column of TypeDef 4 bytes wide
    let mut b = ImageBuilder::new("RS_Fields");
    let root = b.add_type("Verse", "Def", 0, Extends::None);
    let first = b.add_type("RS_Fields", "RS_Heavy", 0, Extends::Def(root));
    b.add_type("RS_Fields", "RS_After", 0, Extends::Def(root));
    if let Some(t) = b.types.get_mut(first) {
        t.fields = 70_000;
        t.methods = 3;
    }
    let asm = read_assembly(&b.build()).unwrap();
    assert_eq!(
        find(&asm, "RS_Fields.RS_After").base,
        BaseType::Local("Verse.Def".into())
    );
}

#[test]
fn the_assembly_name_falls_back_to_the_module_name() {
    let mut b = game_builder();
    b.assembly_name = None;
    b.module_name = "RS_Netmodule.dll".into();
    assert_eq!(read_assembly(&b.build()).unwrap().name, "RS_Netmodule");
}

#[test]
fn input_that_is_not_an_assembly_is_a_typed_error() {
    assert_eq!(
        read_assembly(&[]),
        Err(AssemblyError::NotPortableExecutable)
    );
    assert_eq!(
        read_assembly(b"not an executable at all"),
        Err(AssemblyError::NotPortableExecutable)
    );
    let err = read_assembly(b"MZ").unwrap_err();
    assert_eq!(err.code(), "defs.assembly-truncated");
    let mut image = game_builder().build();
    // clear the CLI data directory: a native PE file
    let cli_dir = 0x98 + 96 + 14 * 8;
    image[cli_dir..cli_dir + 8].fill(0);
    let err = read_assembly(&image).unwrap_err();
    assert_eq!(err, AssemblyError::NoCliMetadata);
    assert_eq!(err.code(), "defs.assembly-no-metadata");
}

#[test]
fn every_truncation_of_a_valid_image_is_handled() {
    let image = game_builder().build_image();
    assert!(read_assembly(&image.bytes).is_ok());
    for len in 0..image.bytes.len() {
        let result = read_assembly(&image.bytes[..len]);
        if len < image.metadata_end {
            assert!(
                result.is_err(),
                "a cut at {len} inside the metadata read ok"
            );
        }
    }
}

#[test]
fn every_truncation_of_a_wide_pe32_plus_image_is_handled() {
    let mut b = mod_builder();
    b.plus = true;
    b.wide_strings = true;
    b.wide_blobs = true;
    let image = b.build_image();
    for len in 0..image.bytes.len() {
        let _ = read_assembly(&image.bytes[..len]);
    }
}

fn row_count_offset(image: &Image, table: u8) -> usize {
    let at = image.tables_offset;
    let valid = u64::from_le_bytes(image.bytes[at + 8..at + 16].try_into().unwrap());
    let before = (0..table).filter(|t| valid >> t & 1 == 1).count();
    at + 24 + 4 * before
}

#[rstest]
#[case::module(0x00)]
#[case::type_ref(0x01)]
#[case::type_def(0x02)]
#[case::type_spec(0x1B)]
#[case::assembly_ref(0x23)]
fn absurd_row_counts_fail_before_allocating(#[case] table: u8) {
    let mut image = mod_builder().build_image();
    let at = row_count_offset(&image, table);
    image.bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    match read_assembly(&image.bytes) {
        Err(AssemblyError::Truncated { .. }) => {}
        other => panic!("expected a truncation error, got {other:?}"),
    }
}

#[test]
fn an_unknown_table_id_is_unsupported() {
    let mut image = game_builder().build_image();
    let at = image.tables_offset + 8;
    let mut valid = u64::from_le_bytes(image.bytes[at..at + 8].try_into().unwrap());
    valid |= 1 << 0x30;
    image.bytes[at..at + 8].copy_from_slice(&valid.to_le_bytes());
    let err = read_assembly(&image.bytes).unwrap_err();
    assert_eq!(err.code(), "defs.assembly-unsupported");
}

#[test]
fn nesting_loops_and_endless_base_chains_do_not_hang() {
    // two types nested in each other are skipped and counted
    let mut b = ImageBuilder::new("RS_Loop");
    let a = b.add_type("RS_Loop", "RS_A", 0, Extends::None);
    let c = b.add_nested(a, "RS_B", Extends::None);
    if let Some(t) = b.types.get_mut(a) {
        t.nested_in = Some(c);
    }
    let asm = read_assembly(&b.build()).unwrap();
    assert_eq!(asm.skipped, 2);
    assert_eq!(asm.types.len(), 1);

    // a base cycle between two definitions yields no def type
    let mut b = ImageBuilder::new("RS_Cycle");
    let x = b.add_type("RS_Cycle", "RS_X", 0, Extends::Def(1));
    b.add_type("RS_Cycle", "RS_Y", 0, Extends::Def(x));
    let asm = read_assembly(&b.build()).unwrap();
    let table = build_type_table(&[asm], ROOT).unwrap();
    assert!(table.is_empty());
}

fn mutation_strategy(image: &Image) -> impl Strategy<Value = Vec<(usize, u8)>> {
    let len = image.bytes.len();
    let meta = image.metadata_offset..image.metadata_end;
    let header = 0usize..0x400;
    let pos = prop_oneof![0..len, meta, header];
    prop::collection::vec((pos, any::<u8>()), 1..24)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    #[test]
    fn random_byte_mutations_never_panic(
        muts in mutation_strategy(&mod_builder().build_image()),
        plus in any::<bool>(),
    ) {
        let mut b = mod_builder();
        b.plus = plus;
        let mut image = b.build();
        for (at, value) in muts {
            if let Some(slot) = image.get_mut(at) {
                *slot = value;
            }
        }
        if let Ok(asm) = read_assembly(&image) {
            let _ = build_type_table(&[asm], ROOT);
        }
    }

    #[test]
    fn random_garbage_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = read_assembly(&bytes);
    }

    #[test]
    fn the_table_of_a_random_hierarchy_is_consistent(
        bases in prop::collection::vec(0usize..40, 1..40),
    ) {
        // type i gets the base bases[i] % (i + 1) (itself means no base): an arbitrary forest
        let mut b = ImageBuilder::new("RS_Forest");
        b.add_type("Verse", "Def", 0, Extends::None);
        for (i, base) in bases.iter().enumerate() {
            let parent = base % (i + 1);
            b.add_type("RS_Forest", &format!("RS_T{i}"), 0, Extends::Def(parent));
        }
        let asm = read_assembly(&b.build()).unwrap();
        let table = build_type_table(&[asm], ROOT).unwrap();
        prop_assert_eq!(table.len(), bases.len() + 1);
        for (name, _) in table.iter() {
            prop_assert!(table.is_a(name, ROOT));
        }
    }
}

fn two_assembly_table() -> TypeTable {
    let game = read_assembly(&game_builder().build()).unwrap();
    let modasm = read_assembly(&mod_builder().build()).unwrap();
    build_type_table(&[game, modasm], ROOT).unwrap()
}

#[test]
fn the_table_keeps_types_deriving_from_the_root_across_assemblies() {
    let table = two_assembly_table();
    let names: Vec<&str> = table.iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        vec![
            "Verse.Def",
            "Verse.RS_ThingDef",
            "RS_AbstractDef",
            "RimWorld.RS_Dup",
            "Verse.RS_Outer+RS_Inner",
            "Verse.RS_Gen`1",
            "Verse.RS_GenDef",
            "RS_Mod.RS_AmmoDef",
            "RS_Mod.RS_BoxedDef",
            "Verse.RS_Dup",
            "RS_Mod.RS_InnerDef",
        ]
    );
    assert_eq!(table.root(), ROOT);
    // the plain class, the interface, the orphan and the tool are not defs
    for gone in [
        "Verse.RS_Plain",
        "Verse.RS_IFace",
        "RS_Mod.RS_Orphan",
        "RS_Mod.RS_Tool",
    ] {
        assert!(table.info(gone).is_none(), "{gone}");
    }
}

#[test]
fn the_table_records_base_abstract_flag_and_assembly() {
    let table = two_assembly_table();
    let root = table.info("Verse.Def").unwrap();
    // the parent of the root is not a def type but stays named
    assert_eq!(root.base.as_deref(), Some("Verse.RS_Editable"));
    assert_eq!(root.assembly.as_deref(), Some("RS_Game"));
    let abstract_def = table.info("RS_AbstractDef").unwrap();
    assert!(abstract_def.is_abstract);
    assert!(!table.info("Verse.RS_ThingDef").unwrap().is_abstract);
    let ammo = table.info("RS_Mod.RS_AmmoDef").unwrap();
    assert_eq!(ammo.base.as_deref(), Some("Verse.RS_ThingDef"));
    assert_eq!(ammo.assembly.as_deref(), Some("RS_Mod"));
    assert_eq!(
        table.info("RS_Mod.RS_BoxedDef").unwrap().base.as_deref(),
        Some("Verse.RS_Gen`1")
    );
    assert_eq!(
        table.info("Verse.RS_GenDef").unwrap().base.as_deref(),
        Some("Verse.RS_Gen`1")
    );
    assert_eq!(
        table.info("RS_Mod.RS_InnerDef").unwrap().base.as_deref(),
        Some("Verse.RS_Outer+RS_Inner")
    );
    assert_eq!(
        table.ancestors("RS_Mod.RS_AmmoDef"),
        vec!["RS_Mod.RS_AmmoDef", "Verse.RS_ThingDef", "Verse.Def",]
    );
}

#[test]
fn short_name_collisions_resolve_to_the_later_assembly() {
    let table = two_assembly_table();
    let json = table.to_json_value();
    assert_eq!(
        json["short_name_collisions"],
        serde_json::json!({"RS_Dup": ["RimWorld.RS_Dup", "Verse.RS_Dup"]})
    );
    assert_eq!(json["assemblies"], serde_json::json!(["RS_Game", "RS_Mod"]));
    assert_eq!(table.lookup("RS_Dup"), Some("Verse.RS_Dup"));
    // a type that is only in the mod namespace is found by full name only
    assert_eq!(table.lookup("RS_AmmoDef"), None);
    assert_eq!(table.lookup("rs_mod.rs_ammodef"), Some("RS_Mod.RS_AmmoDef"));
    // nested types answer to their own short name
    assert_eq!(table.lookup("RS_Inner"), Some("Verse.RS_Outer+RS_Inner"));
}

#[test]
fn a_collision_with_a_plain_class_is_listed_only_when_a_def_is_involved() {
    // a plain class sharing a short name with another plain class is no collision
    let mut b = ImageBuilder::new("RS_Quiet");
    let root = b.add_type("Verse", "Def", 0, Extends::None);
    b.add_type("Verse", "RS_Same", 0, Extends::None);
    b.add_type("RimWorld", "RS_Same", 0, Extends::None);
    b.add_type("Verse", "RS_Both", 0, Extends::Def(root));
    b.add_type("RimWorld", "RS_Both", 0, Extends::None);
    let asm = read_assembly(&b.build()).unwrap();
    let table = build_type_table(&[asm], ROOT).unwrap();
    let json = table.to_json_value();
    assert_eq!(
        json["short_name_collisions"],
        serde_json::json!({"RS_Both": ["Verse.RS_Both", "RimWorld.RS_Both"]})
    );
}

#[test]
fn a_full_name_defined_twice_is_listed_once_with_the_later_data() {
    let mut first = ImageBuilder::new("RS_One");
    let root = first.add_type("Verse", "Def", 0, Extends::None);
    first.add_type("RS_Ns", "RS_Twice", ABSTRACT, Extends::Def(root));
    let mut second = ImageBuilder::new("RS_Two");
    let asm = second.assembly_ref("RS_One");
    let def = second.type_ref(asm, "Verse", "Def");
    second.add_type("RS_Ns", "RS_Twice", 0, Extends::Ref(def));
    let table = build_type_table(
        &[
            read_assembly(&first.build()).unwrap(),
            read_assembly(&second.build()).unwrap(),
        ],
        ROOT,
    )
    .unwrap();
    let names: Vec<&str> = table.iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["Verse.Def", "RS_Ns.RS_Twice"]);
    let info = table.info("RS_Ns.RS_Twice").unwrap();
    assert!(!info.is_abstract);
    assert_eq!(info.assembly.as_deref(), Some("RS_Two"));
}

#[test]
fn the_table_is_deterministic_and_survives_a_json_round_trip() {
    let a = two_assembly_table().to_json_value();
    let b = two_assembly_table().to_json_value();
    assert_eq!(a.to_string(), b.to_string());
    let again = TypeTable::from_json_value(a.clone()).unwrap();
    assert_eq!(again.to_json_value().to_string(), a.to_string());
    assert_eq!(a["format"], 1);
}

#[test]
fn the_load_order_decides_which_short_name_wins() {
    let game = read_assembly(&game_builder().build()).unwrap();
    let modasm = read_assembly(&mod_builder().build()).unwrap();
    // the mod assembly first: its base lives in a later assembly, which is still followed
    let table = build_type_table(&[modasm, game], ROOT).unwrap();
    assert_eq!(table.lookup("RS_Dup"), Some("RimWorld.RS_Dup"));
    assert!(table.is_a("RS_Mod.RS_AmmoDef", ROOT));
}

#[test]
fn no_assemblies_or_no_root_give_an_empty_table() {
    assert!(build_type_table(&[], ROOT).unwrap().is_empty());
    let asm = read_assembly(&game_builder().build()).unwrap();
    assert!(
        build_type_table(&[asm], "Nowhere.Missing")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn managed_dirs_list_the_layout_of_every_operating_system() {
    let dirs = managed_dirs(camino::Utf8Path::new("/games/RimWorld"));
    let rel: Vec<String> = dirs
        .iter()
        .map(|d| d.strip_prefix("/games/RimWorld").unwrap().to_string())
        .collect();
    assert_eq!(
        rel,
        vec![
            "RimWorldLinux_Data/Managed",
            "RimWorldWin64_Data/Managed",
            "RimWorldMac.app/Contents/Resources/Data/Managed",
            "RimWorld_Data/Managed",
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Real installs (ignored, read only)
// ---------------------------------------------------------------------------------------------

fn env_dir(name: &str) -> Option<Utf8PathBuf> {
    let dir = Utf8PathBuf::from(std::env::var(name).ok()?);
    dir.is_dir().then_some(dir)
}

fn read_dll(path: &camino::Utf8Path) -> AssemblyTypes {
    let bytes = std::fs::read(path).unwrap();
    read_assembly(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn game_assembly(install: &camino::Utf8Path) -> Option<AssemblyTypes> {
    managed_dirs(install)
        .into_iter()
        .map(|d| d.join("Assembly-CSharp.dll"))
        .find(|p| p.is_file())
        .map(|p| read_dll(&p))
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR (a RimWorld install)"]
fn real_game_assembly_matches_the_research_prototype_table() {
    let Some(install) = env_dir("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; skipped");
        return;
    };
    let asm = game_assembly(&install).expect("Assembly-CSharp.dll under a Managed folder");
    println!(
        "{}: {} type definitions, {} skipped, {} references",
        asm.name,
        asm.types.len(),
        asm.skipped,
        asm.references.len()
    );
    let started = std::time::Instant::now();
    let table = build_type_table(&[asm], ROOT).unwrap();
    println!("built {} def types in {:?}", table.len(), started.elapsed());
    assert!(table.len() > 100);

    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data/def-engine/data/def_types_vanilla.json");
    let Ok(text) = std::fs::read_to_string(&reference) else {
        println!("the research table is absent; nothing to compare");
        return;
    };
    let expected: serde_json::Value = serde_json::from_str(&text).unwrap();
    let expected_types = expected["types"].as_object().unwrap();
    let mut missing = Vec::new();
    let mut different = Vec::new();
    for (name, info) in expected_types {
        match table.info(name) {
            None => missing.push(name.clone()),
            Some(got) => {
                let base = info["base"].as_str();
                let abstract_flag = info["abstract"].as_bool().unwrap_or(false);
                if got.base.as_deref() != base || got.is_abstract != abstract_flag {
                    different.push(format!(
                        "{name}: base {:?} vs {:?}, abstract {} vs {}",
                        got.base, base, got.is_abstract, abstract_flag
                    ));
                }
            }
        }
    }
    let extra: Vec<&str> = table
        .iter()
        .map(|(n, _)| n)
        .filter(|n| !expected_types.contains_key(*n))
        .collect();
    println!(
        "prototype {} types, ours {}: {} missing, {} different, {} extra",
        expected_types.len(),
        table.len(),
        missing.len(),
        different.len(),
        extra.len()
    );
    for line in missing.iter().take(20) {
        println!("missing: {line}");
    }
    for line in different.iter().take(20) {
        println!("different: {line}");
    }
    for line in extra.iter().take(20) {
        println!("extra: {line}");
    }
    let ours: Vec<&str> = table.iter().map(|(n, _)| n).collect();
    let theirs: Vec<&str> = expected_types.keys().map(String::as_str).collect();
    if extra.is_empty() && missing.is_empty() {
        println!("same order as the prototype: {}", ours == theirs);
    }
    assert!(missing.is_empty(), "types the prototype found are missing");
    assert!(different.is_empty(), "bases or flags differ");
}

fn collect_dlls(dir: &camino::Utf8Path, depth: usize, out: &mut Vec<Utf8PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path.is_dir() && depth > 0 {
            collect_dlls(&path, depth - 1, out);
        } else if path.extension() == Some("dll")
            && path.parent().and_then(|p| p.file_name()) == Some("Assemblies")
        {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn real_combat_extended_assembly_adds_its_def_types() {
    let (Some(install), Some(ce)) = (env_dir("RIMSTUDIO_GAME_DIR"), env_dir("RIMSTUDIO_CE_DIR"))
    else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CE_DIR is not set; skipped");
        return;
    };
    let game = game_assembly(&install).expect("Assembly-CSharp.dll");
    let mut dlls = Vec::new();
    collect_dlls(&ce, 3, &mut dlls);
    println!("assemblies under the mod folder: {dlls:?}");
    let mut loaded = vec![game];
    for dll in dlls.iter().filter(|p| {
        p.file_stem()
            .is_some_and(|s| s.eq_ignore_ascii_case("CombatExtended"))
    }) {
        let asm = read_dll(dll);
        println!("{dll}: {} types, {} skipped", asm.types.len(), asm.skipped);
        loaded.push(asm);
    }
    assert!(loaded.len() > 1, "no CombatExtended assembly found");
    let vanilla_only = build_type_table(&loaded[..1], ROOT).unwrap();
    let table = build_type_table(&loaded, ROOT).unwrap();
    println!(
        "vanilla {} def types, with CE {}",
        vanilla_only.len(),
        table.len()
    );
    let added: Vec<&str> = table
        .iter()
        .filter(|(_, info)| info.assembly.as_deref() == Some("CombatExtended"))
        .map(|(n, _)| n)
        .collect();
    for name in &added {
        println!("CE def type: {name}");
    }
    assert!(added.iter().any(|n| n.ends_with(".AmmoDef")));
    let ammo = added.iter().find(|n| n.ends_with(".AmmoDef")).unwrap();
    assert!(table.is_a(ammo, ROOT));
    println!("lookup of AmmoDef by full name: {:?}", table.lookup(ammo));
}
