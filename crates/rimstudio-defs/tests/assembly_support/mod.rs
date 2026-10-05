//! A small writer of minimal managed assemblies (PE image plus ECMA-335 metadata) for tests.
//!
//! Only what the reader needs is real: the PE and CLI headers, the metadata root with its
//! streams, and the tables Module, TypeRef, TypeDef, Field, MethodDef, TypeSpec, Assembly,
//! AssemblyRef and NestedClass. Names are fictional. The writer is deliberately independent of
//! the reader (it re-implements the width rules from the specification).
//!
//! The first TypeDef row is always the `<Module>` type, as in real assemblies, so a type added
//! with [`ImageBuilder::add_type`] has the table row `index + 2`.

#![allow(dead_code, unreachable_pub, clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

/// `TypeAttributes`: public class.
pub const PUBLIC: u32 = 0x1;
/// `TypeAttributes`: nested public class.
pub const NESTED_PUBLIC: u32 = 0x2;
/// `TypeAttributes`: interface (and abstract).
pub const INTERFACE: u32 = 0x20 | 0x80;
/// `TypeAttributes`: abstract.
pub const ABSTRACT: u32 = 0x80;
/// `TypeAttributes`: sealed.
pub const SEALED: u32 = 0x100;

/// Where a type definition gets its base type.
#[derive(Debug, Clone, Copy)]
pub enum Extends {
    /// No base type.
    None,
    /// A type definition of the same image (0 based, as given to `add_type`).
    Def(usize),
    /// A type reference (0 based, as given to `type_ref`).
    Ref(usize),
    /// A generic instantiation (one int argument) of a type definition.
    GenericDef(usize),
    /// A generic instantiation (one int argument) of a type reference.
    GenericRef(usize),
    /// A generic instantiation of a value type reference: not a class base.
    GenericValueRef(usize),
    /// A raw TypeDefOrRef coded value, for damaged input.
    Raw(u32),
}

/// The resolution scope of a type reference.
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// An assembly reference (0 based).
    Assembly(usize),
    /// The enclosing type reference (0 based), for a nested type.
    Type(usize),
    /// The module itself.
    Module,
}

/// A type definition to write.
#[derive(Debug, Clone)]
pub struct TypeDefSpec {
    pub namespace: String,
    pub name: String,
    pub flags: u32,
    pub extends: Extends,
    pub nested_in: Option<usize>,
    pub fields: u32,
    pub methods: u32,
}

/// A type reference to write.
#[derive(Debug, Clone)]
pub struct TypeRefSpec {
    pub namespace: String,
    pub name: String,
    pub scope: Scope,
}

/// A written image and where its parts are.
#[derive(Debug, Clone)]
pub struct Image {
    pub bytes: Vec<u8>,
    /// File offset of the metadata root.
    pub metadata_offset: usize,
    /// File offset one past the last metadata byte.
    pub metadata_end: usize,
    /// File offset of the `#~` stream.
    pub tables_offset: usize,
}

/// Builds an image.
#[derive(Debug, Clone)]
pub struct ImageBuilder {
    pub assembly_name: Option<String>,
    pub module_name: String,
    pub plus: bool,
    pub wide_strings: bool,
    pub wide_guids: bool,
    pub wide_blobs: bool,
    pub tables_stream: &'static str,
    pub assembly_refs: Vec<String>,
    pub type_refs: Vec<TypeRefSpec>,
    pub types: Vec<TypeDefSpec>,
}

impl ImageBuilder {
    /// An image of a named assembly (PE32).
    pub fn new(name: &str) -> Self {
        Self {
            assembly_name: Some(name.to_owned()),
            module_name: format!("{name}.dll"),
            plus: false,
            wide_strings: false,
            wide_guids: false,
            wide_blobs: false,
            tables_stream: "#~",
            assembly_refs: Vec::new(),
            type_refs: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Adds an assembly reference and returns its 0 based index.
    pub fn assembly_ref(&mut self, name: &str) -> usize {
        self.assembly_refs.push(name.to_owned());
        self.assembly_refs.len() - 1
    }

    /// Adds a type reference to an assembly and returns its 0 based index.
    pub fn type_ref(&mut self, asm: usize, namespace: &str, name: &str) -> usize {
        self.type_refs.push(TypeRefSpec {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            scope: Scope::Assembly(asm),
        });
        self.type_refs.len() - 1
    }

    /// Adds a nested type reference and returns its 0 based index.
    pub fn nested_type_ref(&mut self, outer: usize, name: &str) -> usize {
        self.type_refs.push(TypeRefSpec {
            namespace: String::new(),
            name: name.to_owned(),
            scope: Scope::Type(outer),
        });
        self.type_refs.len() - 1
    }

    /// Adds a public class and returns its 0 based index.
    pub fn add_type(&mut self, namespace: &str, name: &str, flags: u32, extends: Extends) -> usize {
        self.types.push(TypeDefSpec {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            flags: flags | PUBLIC,
            extends,
            nested_in: None,
            fields: 0,
            methods: 0,
        });
        self.types.len() - 1
    }

    /// Adds a nested class and returns its 0 based index.
    pub fn add_nested(&mut self, outer: usize, name: &str, extends: Extends) -> usize {
        self.types.push(TypeDefSpec {
            namespace: String::new(),
            name: name.to_owned(),
            flags: NESTED_PUBLIC,
            extends,
            nested_in: Some(outer),
            fields: 0,
            methods: 0,
        });
        self.types.len() - 1
    }

    /// Writes the image.
    pub fn build(&self) -> Vec<u8> {
        self.build_image().bytes
    }

    /// Writes the image and reports where the metadata is.
    pub fn build_image(&self) -> Image {
        let mut strings = Heap::new(true);
        let mut blobs = Heap::new(false);
        let md = self.metadata(&mut strings, &mut blobs);
        if strings.bytes.len() > 0xFFFF && !self.wide_strings
            || blobs.bytes.len() > 0xFFFF && !self.wide_blobs
        {
            // the heaps outgrew 16 bit indexes: write again with wide ones
            let mut wider = self.clone();
            wider.wide_strings |= strings.bytes.len() > 0xFFFF;
            wider.wide_blobs |= blobs.bytes.len() > 0xFFFF;
            return wider.build_image();
        }
        self.wrap_pe(md)
    }
}

struct Heap {
    bytes: Vec<u8>,
    seen: HashMap<Vec<u8>, u32>,
    strings: bool,
}

impl Heap {
    fn new(strings: bool) -> Self {
        Self {
            bytes: vec![0],
            seen: HashMap::new(),
            strings,
        }
    }

    fn add_string(&mut self, s: &str) -> u32 {
        debug_assert!(self.strings);
        if s.is_empty() {
            return 0;
        }
        if let Some(&at) = self.seen.get(s.as_bytes()) {
            return at;
        }
        let at = self.bytes.len() as u32;
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0);
        self.seen.insert(s.as_bytes().to_vec(), at);
        at
    }

    fn add_blob(&mut self, data: &[u8]) -> u32 {
        let at = self.bytes.len() as u32;
        compress(&mut self.bytes, data.len() as u32);
        self.bytes.extend_from_slice(data);
        at
    }
}

fn compress(out: &mut Vec<u8>, v: u32) {
    if v < 0x80 {
        out.push(v as u8);
    } else if v < 0x4000 {
        out.push(0x80 | (v >> 8) as u8);
        out.push(v as u8);
    } else {
        out.push(0xC0 | (v >> 24) as u8);
        out.push((v >> 16) as u8);
        out.push((v >> 8) as u8);
        out.push(v as u8);
    }
}

fn align(buf: &mut Vec<u8>, to: usize) {
    while !buf.len().is_multiple_of(to) {
        buf.push(0);
    }
}

fn put16(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&(v as u16).to_le_bytes());
}

fn put32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Writes a table index or coded index with the width the row counts demand.
fn put_wide(buf: &mut Vec<u8>, v: u32, wide: bool) {
    if wide {
        put32(buf, v);
    } else {
        put16(buf, v);
    }
}

impl ImageBuilder {
    fn metadata(&self, strings: &mut Heap, blobs: &mut Heap) -> (Vec<u8>, usize) {
        let n_types = self.types.len() as u32 + 1;
        let n_refs = self.type_refs.len() as u32;
        let n_asm_refs = self.assembly_refs.len() as u32;
        let total_fields: u32 = self.types.iter().map(|t| t.fields).sum();
        let total_methods: u32 = self.types.iter().map(|t| t.methods).sum();

        // TypeSpec rows come from the generic bases, in type order.
        let mut specs: Vec<Vec<u8>> = Vec::new();
        let mut extends_coded: Vec<u32> = vec![0];
        for t in &self.types {
            let coded = match t.extends {
                Extends::None => 0,
                Extends::Def(i) => (i as u32 + 2) << 2,
                Extends::Ref(i) => ((i as u32 + 1) << 2) | 1,
                Extends::Raw(v) => v,
                Extends::GenericDef(i) => {
                    specs.push(generic_sig(0x12, (i as u32 + 2) << 2));
                    ((specs.len() as u32) << 2) | 2
                }
                Extends::GenericRef(i) => {
                    specs.push(generic_sig(0x12, ((i as u32 + 1) << 2) | 1));
                    ((specs.len() as u32) << 2) | 2
                }
                Extends::GenericValueRef(i) => {
                    specs.push(generic_sig(0x11, ((i as u32 + 1) << 2) | 1));
                    ((specs.len() as u32) << 2) | 2
                }
            };
            extends_coded.push(coded);
        }
        let n_specs = specs.len() as u32;
        let nested: Vec<(u32, u32)> = self
            .types
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.nested_in.map(|o| (i as u32 + 2, o as u32 + 2)))
            .collect();
        let has_assembly = u32::from(self.assembly_name.is_some());

        let wide = |max: u32, bits: u32| u64::from(max) >= (1u64 << (16 - bits));
        let wide_idx = |rows: u32| rows > 0xFFFF;
        let tdr_wide = wide(n_types.max(n_refs).max(n_specs), 2);
        let scope_wide = wide(1.max(n_refs).max(n_asm_refs), 2);
        let ws = self.wide_strings;
        let wg = self.wide_guids;
        let wb = self.wide_blobs;

        let mut tables: Vec<(u8, u32, Vec<u8>)> = Vec::new();

        let mut t = Vec::new();
        put16(&mut t, 0);
        put_wide(&mut t, strings.add_string(&self.module_name), ws);
        put_wide(&mut t, 1, wg);
        put_wide(&mut t, 0, wg);
        put_wide(&mut t, 0, wg);
        tables.push((0x00, 1, t));

        let mut t = Vec::new();
        for r in &self.type_refs {
            let scope = match r.scope {
                Scope::Module => 0,
                Scope::Assembly(i) => ((i as u32 + 1) << 2) | 2,
                Scope::Type(i) => ((i as u32 + 1) << 2) | 3,
            };
            put_wide(&mut t, scope, scope_wide);
            put_wide(&mut t, strings.add_string(&r.name), ws);
            put_wide(&mut t, strings.add_string(&r.namespace), ws);
        }
        tables.push((0x01, n_refs, t));

        let mut t = Vec::new();
        let mut field_list = 1u32;
        let mut method_list = 1u32;
        let module_type = TypeDefSpec {
            namespace: String::new(),
            name: "<Module>".to_owned(),
            flags: 0,
            extends: Extends::None,
            nested_in: None,
            fields: 0,
            methods: 0,
        };
        for (i, ty) in std::iter::once(&module_type)
            .chain(self.types.iter())
            .enumerate()
        {
            put32(&mut t, ty.flags);
            put_wide(&mut t, strings.add_string(&ty.name), ws);
            put_wide(&mut t, strings.add_string(&ty.namespace), ws);
            put_wide(&mut t, extends_coded.get(i).copied().unwrap_or(0), tdr_wide);
            put_wide(&mut t, field_list, wide_idx(total_fields));
            put_wide(&mut t, method_list, wide_idx(total_methods));
            field_list += ty.fields;
            method_list += ty.methods;
        }
        tables.push((0x02, n_types, t));

        let field_sig = blobs.add_blob(&[0x06, 0x08]);
        let mut t = Vec::new();
        for i in 0..total_fields {
            put16(&mut t, 6);
            put_wide(&mut t, strings.add_string(&format!("RS_f{i}")), ws);
            put_wide(&mut t, field_sig, wb);
        }
        tables.push((0x04, total_fields, t));

        let method_sig = blobs.add_blob(&[0x00, 0x00, 0x01]);
        let mut t = Vec::new();
        for i in 0..total_methods {
            put32(&mut t, 0);
            put16(&mut t, 0);
            put16(&mut t, 6);
            put_wide(&mut t, strings.add_string(&format!("RS_m{i}")), ws);
            put_wide(&mut t, method_sig, wb);
            put_wide(&mut t, 1, false);
        }
        tables.push((0x06, total_methods, t));

        let mut t = Vec::new();
        for sig in &specs {
            put_wide(&mut t, blobs.add_blob(sig), wb);
        }
        tables.push((0x1B, n_specs, t));

        let mut t = Vec::new();
        if let Some(name) = &self.assembly_name {
            put32(&mut t, 0x8004);
            for part in [1, 0, 0, 0] {
                put16(&mut t, part);
            }
            put32(&mut t, 0);
            put_wide(&mut t, 0, wb);
            put_wide(&mut t, strings.add_string(name), ws);
            put_wide(&mut t, 0, ws);
        }
        tables.push((0x20, has_assembly, t));

        let mut t = Vec::new();
        for name in &self.assembly_refs {
            for part in [1, 0, 0, 0] {
                put16(&mut t, part);
            }
            put32(&mut t, 0);
            put_wide(&mut t, 0, wb);
            put_wide(&mut t, strings.add_string(name), ws);
            put_wide(&mut t, 0, ws);
            put_wide(&mut t, 0, wb);
        }
        tables.push((0x23, n_asm_refs, t));

        let mut t = Vec::new();
        for (inner, outer) in &nested {
            put_wide(&mut t, *inner, wide_idx(n_types));
            put_wide(&mut t, *outer, wide_idx(n_types));
        }
        tables.push((0x29, nested.len() as u32, t));

        // The #~ stream.
        let mut valid = 0u64;
        for (id, rows, _) in &tables {
            if *rows > 0 {
                valid |= 1u64 << id;
            }
        }
        let mut heap_flags = 0u8;
        if ws {
            heap_flags |= 1;
        }
        if wg {
            heap_flags |= 2;
        }
        if wb {
            heap_flags |= 4;
        }
        let mut stream = Vec::new();
        put32(&mut stream, 0);
        stream.push(2);
        stream.push(0);
        stream.push(heap_flags);
        stream.push(1);
        stream.extend_from_slice(&valid.to_le_bytes());
        stream.extend_from_slice(&0u64.to_le_bytes());
        for (_, rows, _) in tables.iter().filter(|(_, rows, _)| *rows > 0) {
            put32(&mut stream, *rows);
        }
        for (_, _, data) in &tables {
            stream.extend_from_slice(data);
        }
        align(&mut stream, 4);

        // Metadata root.
        let mut streams: Vec<(&str, Vec<u8>)> = vec![
            (self.tables_stream, stream),
            ("#Strings", {
                let mut s = strings.bytes.clone();
                align(&mut s, 4);
                s
            }),
            ("#US", vec![0, 0, 0, 0]),
            ("#GUID", vec![0xA5; 16]),
            ("#Blob", {
                let mut s = blobs.bytes.clone();
                align(&mut s, 4);
                s
            }),
        ];
        let mut header = Vec::new();
        put32(&mut header, 0x424A_5342);
        put16(&mut header, 1);
        put16(&mut header, 1);
        put32(&mut header, 0);
        let version = b"v4.0.30319\0\0";
        put32(&mut header, version.len() as u32);
        header.extend_from_slice(version);
        put16(&mut header, 0);
        put16(&mut header, streams.len() as u32);
        let headers_len: usize = streams
            .iter()
            .map(|(n, _)| 8 + ((n.len() + 1 + 3) & !3))
            .sum();
        let mut offset = header.len() + headers_len;
        let tables_rel = offset;
        for (name, data) in &streams {
            put32(&mut header, offset as u32);
            put32(&mut header, data.len() as u32);
            header.extend_from_slice(name.as_bytes());
            header.push(0);
            align(&mut header, 4);
            offset += data.len();
        }
        for (_, data) in streams.drain(..) {
            header.extend_from_slice(&data);
        }
        (header, tables_rel)
    }

    fn wrap_pe(&self, (md, tables_rel): (Vec<u8>, usize)) -> Image {
        let optional_size: usize = if self.plus { 240 } else { 224 };
        let dirs_at: usize = if self.plus { 112 } else { 96 };
        let mut f = vec![0u8; 0x80];
        f[0] = b'M';
        f[1] = b'Z';
        f[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        f.extend_from_slice(b"PE\0\0");
        put16(&mut f, if self.plus { 0x8664 } else { 0x14C });
        put16(&mut f, 2);
        put32(&mut f, 0);
        put32(&mut f, 0);
        put32(&mut f, 0);
        put16(&mut f, optional_size as u32);
        put16(&mut f, 0x2022);
        let optional = f.len();
        f.resize(optional + optional_size, 0);
        let magic: u16 = if self.plus { 0x20B } else { 0x10B };
        f[optional..optional + 2].copy_from_slice(&magic.to_le_bytes());
        f[optional + dirs_at - 4..optional + dirs_at].copy_from_slice(&16u32.to_le_bytes());
        let cli_rva = 0x4100u32;
        let cli = optional + dirs_at + 14 * 8;
        f[cli..cli + 4].copy_from_slice(&cli_rva.to_le_bytes());
        f[cli + 4..cli + 8].copy_from_slice(&72u32.to_le_bytes());

        // Section table: .rdata (filler) and .text (CLI header and metadata).
        let text_len = 0x100 + 72 + md.len();
        let text_raw = (text_len + 0x1FF) & !0x1FF;
        for (name, va, vsize, raw_size, raw_ptr) in [
            (b".rdata\0\0", 0x1000u32, 0x200u32, 0x200u32, 0x400u32),
            (
                b".text\0\0\0",
                0x4000,
                text_len as u32,
                text_raw as u32,
                0x600,
            ),
        ] {
            f.extend_from_slice(name);
            put32(&mut f, vsize);
            put32(&mut f, va);
            put32(&mut f, raw_size);
            put32(&mut f, raw_ptr);
            f.extend_from_slice(&[0; 16]);
        }
        f.resize(0x400, 0);
        f.extend(std::iter::repeat_n(0xAA, 0x200));
        // .text
        f.extend(std::iter::repeat_n(0xBB, 0x100));
        let mut cli_header = Vec::new();
        put32(&mut cli_header, 72);
        put16(&mut cli_header, 2);
        put16(&mut cli_header, 5);
        put32(&mut cli_header, cli_rva + 72);
        put32(&mut cli_header, md.len() as u32);
        put32(&mut cli_header, 1);
        cli_header.resize(72, 0);
        f.extend_from_slice(&cli_header);
        let metadata_offset = f.len();
        f.extend_from_slice(&md);
        let metadata_end = f.len();
        f.resize(0x600 + text_raw, 0);
        Image {
            bytes: f,
            metadata_offset,
            metadata_end,
            tables_offset: metadata_offset + tables_rel,
        }
    }
}

fn generic_sig(kind: u8, token: u32) -> Vec<u8> {
    let mut sig = vec![0x15, kind];
    compress(&mut sig, token);
    sig.push(1);
    sig.push(0x08);
    sig
}
