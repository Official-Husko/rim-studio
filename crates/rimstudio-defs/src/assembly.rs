//! The def type table built at run time from the user's own .NET assemblies (spike S-07).
//!
//! The game finds its def classes by reflection over the loaded assemblies. The repository ships no
//! table of them, so this module reads the same facts from the compiled files: a pure byte parser
//! of the PE image and its ECMA-335 metadata, with no IO, no XML and no `unsafe`. Only the tables
//! that matter are decoded (`Module`, `TypeRef`, `TypeDef`, `TypeSpec`, `Assembly`, `AssemblyRef`
//! and `NestedClass`); every other table is only measured so that row sizes and offsets are exact.
//!
//! Three steps, each usable alone:
//!
//! 1. [`read_assembly`]: one image to [`AssemblyTypes`] (assembly name, references, and for every
//!    type definition its namespace, name, flags and [`BaseType`]);
//! 2. [`build_type_table`]: the assemblies in load order to a [`TypeTable`] holding the types that
//!    derive from the root def type, following base types across assemblies by full name;
//! 3. [`managed_dirs`] and [`sort_game_assemblies`]: where the game assemblies live and the order
//!    in which they are loaded, as pure path computations for the caller that does the reading.
//!
//! # Names
//!
//! Full names are `Namespace.Name`, as reflection prints them. A nested type is
//! `Namespace.Outer+Inner` (the namespace is the one of the outermost type), and a generic type
//! keeps its metadata arity suffix (`Namespace.Name`1`).
//!
//! # Hostile input
//!
//! Every read is bounds checked and every offset computation is checked. A table that claims more
//! rows than the stream can hold is an error before anything is allocated, so memory use is
//! bounded by the input size. Names longer than [`MAX_NAME_LEN`] bytes, nesting deeper than
//! [`MAX_NESTING`] and base chains that loop are rejected per type (the type is skipped and
//! counted in [`AssemblyTypes::skipped`]); structural damage is an [`AssemblyError`].

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::DefsError;
use crate::type_table::{IGNORED_NAMESPACES, TypeInfo, TypeTable};

/// The longest name (in bytes) accepted from a string heap.
pub const MAX_NAME_LEN: usize = 4096;

/// The deepest nesting of types (or of type references) that is followed.
pub const MAX_NESTING: usize = 32;

/// The longest full type name (in bytes) that is kept.
const MAX_FULL_NAME_LEN: usize = 8192;
/// The most sections a PE image may have (the PE format limit).
const MAX_SECTIONS: usize = 96;
/// The most metadata streams accepted.
const MAX_STREAMS: usize = 32;
/// The longest metadata version string accepted.
const MAX_VERSION_LEN: usize = 1024;

/// The `TypeAttributes` bit of an interface.
const FLAG_INTERFACE: u32 = 0x20;
/// The `TypeAttributes` bit of an abstract class.
const FLAG_ABSTRACT: u32 = 0x80;
/// The `TypeAttributes` bit of a sealed class.
const FLAG_SEALED: u32 = 0x100;

/// The file name stem of the main game assembly.
pub const GAME_ASSEMBLY: &str = "Assembly-CSharp";

/// A failed read of an assembly or of a type table.
///
/// Every variant has a stable [`AssemblyError::code`] string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AssemblyError {
    /// The bytes are not a PE image.
    #[error("not a portable executable image")]
    NotPortableExecutable,
    /// The image is a PE file without a CLI header (not a managed assembly).
    #[error("the image has no CLI metadata (not a managed assembly)")]
    NoCliMetadata,
    /// The image ends before a structure it announces.
    #[error("the image is truncated: {what}")]
    Truncated {
        /// The structure that could not be read.
        what: &'static str,
    },
    /// A structure is inconsistent.
    #[error("malformed assembly metadata: {what}")]
    Malformed {
        /// What is wrong.
        what: &'static str,
    },
    /// A valid feature this reader does not handle.
    #[error("unsupported assembly feature: {what}")]
    Unsupported {
        /// The feature.
        what: &'static str,
    },
    /// The type table could not be assembled from the types.
    #[error(transparent)]
    TypeTable(#[from] DefsError),
}

impl AssemblyError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotPortableExecutable => "defs.assembly-not-pe",
            Self::NoCliMetadata => "defs.assembly-no-metadata",
            Self::Truncated { .. } => "defs.assembly-truncated",
            Self::Malformed { .. } => "defs.assembly-malformed",
            Self::Unsupported { .. } => "defs.assembly-unsupported",
            Self::TypeTable(_) => "defs.assembly-type-table",
        }
    }
}

/// A type named by a `TypeRef` (a type defined in another assembly or module).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalType {
    /// The namespace (of the outermost type for a nested reference).
    pub namespace: String,
    /// The name (`Outer+Inner` for a nested reference).
    pub name: String,
    /// The name of the referenced assembly when the scope is an `AssemblyRef`.
    pub assembly: Option<String>,
}

impl ExternalType {
    /// The full name, `Namespace.Name` or just the name in the global namespace.
    #[must_use]
    pub fn full_name(&self) -> String {
        join_name(&self.namespace, &self.name)
    }
}

/// The base type of a type definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseType {
    /// No base type (interfaces, the module type, `System.Object`).
    None,
    /// A type defined in the same assembly, by full name. A generic instantiation of such a type
    /// resolves to the generic type definition.
    Local(String),
    /// A type of another assembly. A generic instantiation resolves to the generic type.
    External(ExternalType),
    /// A base that could not be resolved (a damaged token, a value type, an array).
    Unresolved,
}

impl BaseType {
    /// The full name of the base, `None` for [`BaseType::None`] and [`BaseType::Unresolved`].
    #[must_use]
    pub fn full_name(&self) -> Option<String> {
        match self {
            Self::Local(name) => Some(name.clone()),
            Self::External(ext) => Some(ext.full_name()),
            Self::None | Self::Unresolved => None,
        }
    }
}

/// One type definition of an assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDefInfo {
    /// The namespace (of the outermost type for a nested type).
    pub namespace: String,
    /// The name (`Outer+Inner` for a nested type).
    pub name: String,
    /// The raw `TypeAttributes` flags.
    pub flags: u32,
    /// The base type.
    pub base: BaseType,
}

impl TypeDefInfo {
    /// The full name, `Namespace.Name` or just the name in the global namespace.
    #[must_use]
    pub fn full_name(&self) -> String {
        join_name(&self.namespace, &self.name)
    }

    /// True for an abstract type (an interface is abstract too).
    #[must_use]
    pub fn is_abstract(&self) -> bool {
        self.flags & FLAG_ABSTRACT != 0
    }

    /// True for an interface.
    #[must_use]
    pub fn is_interface(&self) -> bool {
        self.flags & FLAG_INTERFACE != 0
    }

    /// True for a sealed type.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.flags & FLAG_SEALED != 0
    }

    /// True for a type declared inside another type.
    #[must_use]
    pub fn is_nested(&self) -> bool {
        self.name.contains('+')
    }

    /// The name without the enclosing types.
    #[must_use]
    pub fn short_name(&self) -> &str {
        short_of(&self.name)
    }
}

/// What [`read_assembly`] extracts from one image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssemblyTypes {
    /// The assembly name (from the `Assembly` table, else the module name without extension).
    pub name: String,
    /// The names of the referenced assemblies, in metadata order.
    pub references: Vec<String>,
    /// The type definitions in metadata order.
    pub types: Vec<TypeDefInfo>,
    /// Type definitions left out because their names were unreadable, too long, or nested in a
    /// loop.
    pub skipped: usize,
}

fn join_name(ns: &str, name: &str) -> String {
    if ns.is_empty() {
        name.to_owned()
    } else {
        format!("{ns}.{name}")
    }
}

fn short_of(name: &str) -> &str {
    name.rsplit('+').next().unwrap_or(name)
}

// ---------------------------------------------------------------------------------------------
// Byte access
// ---------------------------------------------------------------------------------------------

fn bytes_at(data: &[u8], off: usize, len: usize) -> Option<&[u8]> {
    data.get(off..off.checked_add(len)?)
}

fn u8_at(data: &[u8], off: usize) -> Option<u8> {
    data.get(off).copied()
}

fn u16_at(data: &[u8], off: usize) -> Option<u16> {
    bytes_at(data, off, 2)
        .and_then(|b| <[u8; 2]>::try_from(b).ok())
        .map(u16::from_le_bytes)
}

fn u32_at(data: &[u8], off: usize) -> Option<u32> {
    bytes_at(data, off, 4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes)
}

fn u64_at(data: &[u8], off: usize) -> Option<u64> {
    bytes_at(data, off, 8)
        .and_then(|b| <[u8; 8]>::try_from(b).ok())
        .map(u64::from_le_bytes)
}

fn need<T>(value: Option<T>, what: &'static str) -> Result<T, AssemblyError> {
    value.ok_or(AssemblyError::Truncated { what })
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Decodes an ECMA-335 compressed unsigned integer, returning the value and the bytes used.
fn compressed_u32(data: &[u8]) -> Option<(u32, usize)> {
    let b0 = u32::from(*data.first()?);
    if b0 & 0x80 == 0 {
        Some((b0, 1))
    } else if b0 & 0xC0 == 0x80 {
        let b1 = u32::from(*data.get(1)?);
        Some((((b0 & 0x3F) << 8) | b1, 2))
    } else if b0 & 0xE0 == 0xC0 {
        let b1 = u32::from(*data.get(1)?);
        let b2 = u32::from(*data.get(2)?);
        let b3 = u32::from(*data.get(3)?);
        Some((((b0 & 0x1F) << 24) | (b1 << 16) | (b2 << 8) | b3, 4))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// PE image
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Section {
    virtual_address: u32,
    raw_size: u32,
    raw_ptr: u32,
}

struct PeImage<'a> {
    data: &'a [u8],
    sections: Vec<Section>,
    cli_rva: u32,
}

impl<'a> PeImage<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, AssemblyError> {
        if bytes_at(data, 0, 2) != Some(b"MZ".as_slice()) {
            return Err(AssemblyError::NotPortableExecutable);
        }
        let lfanew = to_usize(need(u32_at(data, 0x3C), "DOS header")?);
        if bytes_at(data, lfanew, 4) != Some(b"PE\0\0".as_slice()) {
            return Err(AssemblyError::NotPortableExecutable);
        }
        let coff = lfanew.checked_add(4).ok_or(AssemblyError::Truncated {
            what: "COFF header",
        })?;
        let section_count = usize::from(need(u16_at(data, coff + 2), "COFF header")?);
        let optional_size = usize::from(need(u16_at(data, coff + 16), "COFF header")?);
        let optional = coff + 20;
        let magic = need(u16_at(data, optional), "optional header")?;
        let dirs_at = match magic {
            0x10B => 96,
            0x20B => 112,
            _ => {
                return Err(AssemblyError::Unsupported {
                    what: "optional header magic",
                });
            }
        };
        let dir_count = need(u32_at(data, optional + dirs_at - 4), "optional header")?;
        let cli_dir_end = dirs_at + 15 * 8;
        if dir_count < 15 || optional_size < cli_dir_end {
            return Err(AssemblyError::NoCliMetadata);
        }
        let cli_rva = need(
            u32_at(data, optional + dirs_at + 14 * 8),
            "CLI data directory",
        )?;
        let cli_size = need(
            u32_at(data, optional + dirs_at + 14 * 8 + 4),
            "CLI data directory",
        )?;
        if cli_rva == 0 || cli_size == 0 {
            return Err(AssemblyError::NoCliMetadata);
        }
        if section_count == 0 || section_count > MAX_SECTIONS {
            return Err(AssemblyError::Malformed {
                what: "section count",
            });
        }
        let table = optional
            .checked_add(optional_size)
            .ok_or(AssemblyError::Truncated {
                what: "section table",
            })?;
        let mut sections = Vec::with_capacity(section_count);
        for i in 0..section_count {
            let at = table + i * 40;
            sections.push(Section {
                virtual_address: need(u32_at(data, at + 12), "section table")?,
                raw_size: need(u32_at(data, at + 16), "section table")?,
                raw_ptr: need(u32_at(data, at + 20), "section table")?,
            });
        }
        Ok(Self {
            data,
            sections,
            cli_rva,
        })
    }

    /// The file offset of an RVA, `None` when no section holds it.
    fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        self.sections.iter().find_map(|s| {
            let delta = rva.checked_sub(s.virtual_address)?;
            (delta < s.raw_size).then(|| to_usize(s.raw_ptr).checked_add(to_usize(delta)))?
        })
    }

    /// The bytes of the metadata root.
    fn metadata(&self) -> Result<&'a [u8], AssemblyError> {
        let cli = self
            .rva_to_offset(self.cli_rva)
            .ok_or(AssemblyError::Malformed {
                what: "CLI header address",
            })?;
        let md_rva = need(u32_at(self.data, cli + 8), "CLI header")?;
        let md_size = to_usize(need(u32_at(self.data, cli + 12), "CLI header")?);
        let md = self.rva_to_offset(md_rva).ok_or(AssemblyError::Malformed {
            what: "metadata address",
        })?;
        need(bytes_at(self.data, md, md_size), "metadata")
    }
}

// ---------------------------------------------------------------------------------------------
// Metadata streams
// ---------------------------------------------------------------------------------------------

struct Streams<'a> {
    tables: &'a [u8],
    strings: &'a [u8],
    blob: Option<&'a [u8]>,
}

fn parse_streams(md: &[u8]) -> Result<Streams<'_>, AssemblyError> {
    if u32_at(md, 0) != Some(0x424A_5342) {
        return Err(AssemblyError::Malformed {
            what: "metadata signature",
        });
    }
    let version_len = to_usize(need(u32_at(md, 12), "metadata root")?);
    if version_len > MAX_VERSION_LEN {
        return Err(AssemblyError::Malformed {
            what: "metadata version length",
        });
    }
    let mut pos = 16 + version_len;
    let count = usize::from(need(u16_at(md, pos + 2), "metadata root")?);
    if count > MAX_STREAMS {
        return Err(AssemblyError::Malformed {
            what: "stream count",
        });
    }
    pos += 4;
    let mut tables = None;
    let mut unoptimized = None;
    let mut strings = None;
    let mut blob = None;
    for _ in 0..count {
        let offset = to_usize(need(u32_at(md, pos), "stream header")?);
        let size = to_usize(need(u32_at(md, pos + 4), "stream header")?);
        let name_start = pos + 8;
        let mut name_len = 0;
        loop {
            match u8_at(md, name_start + name_len) {
                Some(0) => break,
                Some(_) if name_len < 32 => name_len += 1,
                Some(_) => {
                    return Err(AssemblyError::Malformed {
                        what: "stream name",
                    });
                }
                None => {
                    return Err(AssemblyError::Truncated {
                        what: "stream header",
                    });
                }
            }
        }
        let name = bytes_at(md, name_start, name_len).unwrap_or_default();
        pos = name_start + ((name_len + 1 + 3) & !3);
        let data = need(bytes_at(md, offset, size), "metadata stream")?;
        match name {
            b"#~" => tables = tables.or(Some(data)),
            b"#-" => unoptimized = unoptimized.or(Some(data)),
            b"#Strings" => strings = strings.or(Some(data)),
            b"#Blob" => blob = blob.or(Some(data)),
            _ => {}
        }
    }
    Ok(Streams {
        tables: tables.or(unoptimized).ok_or(AssemblyError::Malformed {
            what: "no tables stream",
        })?,
        strings: strings.ok_or(AssemblyError::Malformed {
            what: "no #Strings stream",
        })?,
        blob,
    })
}

fn heap_string(heap: &[u8], index: u32) -> Option<String> {
    let rest = heap.get(to_usize(index)..)?;
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    if end > MAX_NAME_LEN {
        return None;
    }
    Some(String::from_utf8_lossy(rest.get(..end)?).into_owned())
}

fn heap_blob(heap: &[u8], index: u32) -> Option<&[u8]> {
    let rest = heap.get(to_usize(index)..)?;
    let (len, used) = compressed_u32(rest)?;
    bytes_at(rest, used, to_usize(len))
}

// ---------------------------------------------------------------------------------------------
// Table schema
// ---------------------------------------------------------------------------------------------

const TABLE_MODULE: u8 = 0x00;
const TABLE_TYPE_REF: u8 = 0x01;
const TABLE_TYPE_DEF: u8 = 0x02;
const TABLE_TYPE_SPEC: u8 = 0x1B;
const TABLE_ASSEMBLY: u8 = 0x20;
const TABLE_ASSEMBLY_REF: u8 = 0x23;
const TABLE_NESTED_CLASS: u8 = 0x29;
/// The highest table id of ECMA-335.
const LAST_TABLE: u8 = 0x2C;
/// Marks an unused tag of a coded index.
const NONE: u8 = 0xFF;

#[derive(Debug, Clone, Copy)]
enum Coded {
    TypeDefOrRef,
    HasConstant,
    HasCustomAttribute,
    HasFieldMarshal,
    HasDeclSecurity,
    MemberRefParent,
    HasSemantics,
    MethodDefOrRef,
    MemberForwarded,
    Implementation,
    CustomAttributeType,
    ResolutionScope,
    TypeOrMethodDef,
}

impl Coded {
    /// The tag bits and the table of each tag value.
    fn layout(self) -> (u32, &'static [u8]) {
        match self {
            Self::TypeDefOrRef => (2, &[0x02, 0x01, 0x1B]),
            Self::HasConstant => (2, &[0x04, 0x08, 0x17]),
            Self::HasCustomAttribute => (
                5,
                &[
                    0x06, 0x04, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x00, 0x0E, 0x17, 0x14, 0x11, 0x1A,
                    0x1B, 0x20, 0x23, 0x26, 0x27, 0x28, 0x2A, 0x2C, 0x2B,
                ],
            ),
            Self::HasFieldMarshal => (1, &[0x04, 0x08]),
            Self::HasDeclSecurity => (2, &[0x02, 0x06, 0x20]),
            Self::MemberRefParent => (3, &[0x02, 0x01, 0x1A, 0x06, 0x1B]),
            Self::HasSemantics => (1, &[0x14, 0x17]),
            Self::MethodDefOrRef => (1, &[0x06, 0x0A]),
            Self::MemberForwarded => (1, &[0x04, 0x06]),
            Self::Implementation => (2, &[0x26, 0x23, 0x27]),
            Self::CustomAttributeType => (3, &[NONE, NONE, 0x06, 0x0A, NONE]),
            Self::ResolutionScope => (2, &[0x00, 0x1A, 0x23, 0x01]),
            Self::TypeOrMethodDef => (1, &[0x02, 0x06]),
        }
    }

    /// The table and 1 based row a coded value points at.
    fn decode(self, value: u32) -> Option<(u8, u32)> {
        let (bits, tables) = self.layout();
        let tag = usize::try_from(value & ((1 << bits) - 1)).ok()?;
        let table = *tables.get(tag)?;
        (table != NONE).then_some((table, value >> bits))
    }
}

#[derive(Debug, Clone, Copy)]
enum Col {
    U8x2,
    U16,
    U32,
    Str,
    Guid,
    Blob,
    Idx(u8),
    Coded(Coded),
}

use Col::{Blob, Guid, Idx, Str, U8x2, U16, U32};

const MODULE: &[Col] = &[U16, Str, Guid, Guid, Guid];
const TYPE_REF: &[Col] = &[Col::Coded(Coded::ResolutionScope), Str, Str];
const TYPE_DEF: &[Col] = &[
    U32,
    Str,
    Str,
    Col::Coded(Coded::TypeDefOrRef),
    Idx(0x04),
    Idx(0x06),
];
const FIELD_PTR: &[Col] = &[Idx(0x04)];
const FIELD: &[Col] = &[U16, Str, Blob];
const METHOD_PTR: &[Col] = &[Idx(0x06)];
const METHOD_DEF: &[Col] = &[U32, U16, U16, Str, Blob, Idx(0x08)];
const PARAM_PTR: &[Col] = &[Idx(0x08)];
const PARAM: &[Col] = &[U16, U16, Str];
const INTERFACE_IMPL: &[Col] = &[Idx(0x02), Col::Coded(Coded::TypeDefOrRef)];
const MEMBER_REF: &[Col] = &[Col::Coded(Coded::MemberRefParent), Str, Blob];
const CONSTANT: &[Col] = &[U8x2, Col::Coded(Coded::HasConstant), Blob];
const CUSTOM_ATTRIBUTE: &[Col] = &[
    Col::Coded(Coded::HasCustomAttribute),
    Col::Coded(Coded::CustomAttributeType),
    Blob,
];
const FIELD_MARSHAL: &[Col] = &[Col::Coded(Coded::HasFieldMarshal), Blob];
const DECL_SECURITY: &[Col] = &[U16, Col::Coded(Coded::HasDeclSecurity), Blob];
const CLASS_LAYOUT: &[Col] = &[U16, U32, Idx(0x02)];
const FIELD_LAYOUT: &[Col] = &[U32, Idx(0x04)];
const STAND_ALONE_SIG: &[Col] = &[Blob];
const EVENT_MAP: &[Col] = &[Idx(0x02), Idx(0x14)];
const EVENT_PTR: &[Col] = &[Idx(0x14)];
const EVENT: &[Col] = &[U16, Str, Col::Coded(Coded::TypeDefOrRef)];
const PROPERTY_MAP: &[Col] = &[Idx(0x02), Idx(0x17)];
const PROPERTY_PTR: &[Col] = &[Idx(0x17)];
const PROPERTY: &[Col] = &[U16, Str, Blob];
const METHOD_SEMANTICS: &[Col] = &[U16, Idx(0x06), Col::Coded(Coded::HasSemantics)];
const METHOD_IMPL: &[Col] = &[
    Idx(0x02),
    Col::Coded(Coded::MethodDefOrRef),
    Col::Coded(Coded::MethodDefOrRef),
];
const MODULE_REF: &[Col] = &[Str];
const TYPE_SPEC: &[Col] = &[Blob];
const IMPL_MAP: &[Col] = &[U16, Col::Coded(Coded::MemberForwarded), Str, Idx(0x1A)];
const FIELD_RVA: &[Col] = &[U32, Idx(0x04)];
const ENC_LOG: &[Col] = &[U32, U32];
const ENC_MAP: &[Col] = &[U32];
const ASSEMBLY: &[Col] = &[U32, U16, U16, U16, U16, U32, Blob, Str, Str];
const ASSEMBLY_PROCESSOR: &[Col] = &[U32];
const ASSEMBLY_OS: &[Col] = &[U32, U32, U32];
const ASSEMBLY_REF: &[Col] = &[U16, U16, U16, U16, U32, Blob, Str, Str, Blob];
const ASSEMBLY_REF_PROCESSOR: &[Col] = &[U32, Idx(0x23)];
const ASSEMBLY_REF_OS: &[Col] = &[U32, U32, U32, Idx(0x23)];
const FILE: &[Col] = &[U32, Str, Blob];
const EXPORTED_TYPE: &[Col] = &[U32, U32, Str, Str, Col::Coded(Coded::Implementation)];
const MANIFEST_RESOURCE: &[Col] = &[U32, U32, Str, Col::Coded(Coded::Implementation)];
const NESTED_CLASS: &[Col] = &[Idx(0x02), Idx(0x02)];
const GENERIC_PARAM: &[Col] = &[U16, U16, Col::Coded(Coded::TypeOrMethodDef), Str];
const METHOD_SPEC: &[Col] = &[Col::Coded(Coded::MethodDefOrRef), Blob];
const GENERIC_PARAM_CONSTRAINT: &[Col] = &[Idx(0x2A), Col::Coded(Coded::TypeDefOrRef)];

fn schema(table: u8) -> &'static [Col] {
    match table {
        0x00 => MODULE,
        0x01 => TYPE_REF,
        0x02 => TYPE_DEF,
        0x03 => FIELD_PTR,
        0x04 => FIELD,
        0x05 => METHOD_PTR,
        0x06 => METHOD_DEF,
        0x07 => PARAM_PTR,
        0x08 => PARAM,
        0x09 => INTERFACE_IMPL,
        0x0A => MEMBER_REF,
        0x0B => CONSTANT,
        0x0C => CUSTOM_ATTRIBUTE,
        0x0D => FIELD_MARSHAL,
        0x0E => DECL_SECURITY,
        0x0F => CLASS_LAYOUT,
        0x10 => FIELD_LAYOUT,
        0x11 => STAND_ALONE_SIG,
        0x12 => EVENT_MAP,
        0x13 => EVENT_PTR,
        0x14 => EVENT,
        0x15 => PROPERTY_MAP,
        0x16 => PROPERTY_PTR,
        0x17 => PROPERTY,
        0x18 => METHOD_SEMANTICS,
        0x19 => METHOD_IMPL,
        0x1A => MODULE_REF,
        0x1B => TYPE_SPEC,
        0x1C => IMPL_MAP,
        0x1D => FIELD_RVA,
        0x1E => ENC_LOG,
        0x1F => ENC_MAP,
        0x20 => ASSEMBLY,
        0x21 => ASSEMBLY_PROCESSOR,
        0x22 => ASSEMBLY_OS,
        0x23 => ASSEMBLY_REF,
        0x24 => ASSEMBLY_REF_PROCESSOR,
        0x25 => ASSEMBLY_REF_OS,
        0x26 => FILE,
        0x27 => EXPORTED_TYPE,
        0x28 => MANIFEST_RESOURCE,
        0x29 => NESTED_CLASS,
        0x2A => GENERIC_PARAM,
        0x2B => METHOD_SPEC,
        0x2C => GENERIC_PARAM_CONSTRAINT,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------------------------

const TABLE_SLOTS: usize = LAST_TABLE as usize + 1;

/// The `#~` stream: row counts, row sizes and where every table starts.
struct Tables<'a> {
    data: &'a [u8],
    rows: [u32; TABLE_SLOTS],
    wide_strings: bool,
    wide_guids: bool,
    wide_blobs: bool,
    start: [usize; TABLE_SLOTS],
    row_size: [usize; TABLE_SLOTS],
}

impl<'a> Tables<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, AssemblyError> {
        let heap_sizes = need(u8_at(data, 6), "tables header")?;
        let valid = need(u64_at(data, 8), "tables header")?;
        if valid >> (u32::from(LAST_TABLE) + 1) != 0 {
            return Err(AssemblyError::Unsupported {
                what: "metadata table id",
            });
        }
        let mut rows = [0u32; TABLE_SLOTS];
        let mut pos = 24usize;
        for (table, slot) in rows.iter_mut().enumerate() {
            if valid >> table & 1 == 1 {
                *slot = need(u32_at(data, pos), "table row counts")?;
                pos += 4;
            }
        }
        if heap_sizes & 0x40 != 0 {
            pos += 4;
        }
        let mut tables = Self {
            data,
            rows,
            wide_strings: heap_sizes & 0x01 != 0,
            wide_guids: heap_sizes & 0x02 != 0,
            wide_blobs: heap_sizes & 0x04 != 0,
            start: [0; TABLE_SLOTS],
            row_size: [0; TABLE_SLOTS],
        };
        let mut cursor = pos;
        for table in 0..=LAST_TABLE {
            let slot = usize::from(table);
            let size = schema(table)
                .iter()
                .map(|&c| tables.col_size(c))
                .sum::<usize>();
            let bytes = to_usize(tables.rows_of(table))
                .checked_mul(size)
                .ok_or(AssemblyError::Truncated { what: "table rows" })?;
            if let (Some(start), Some(row_size)) =
                (tables.start.get_mut(slot), tables.row_size.get_mut(slot))
            {
                *start = cursor;
                *row_size = size;
            }
            cursor = cursor
                .checked_add(bytes)
                .filter(|&end| end <= data.len())
                .ok_or(AssemblyError::Truncated { what: "table rows" })?;
        }
        Ok(tables)
    }

    fn rows_of(&self, table: u8) -> u32 {
        self.rows.get(usize::from(table)).copied().unwrap_or(0)
    }

    fn col_size(&self, col: Col) -> usize {
        let wide = |wide: bool| if wide { 4 } else { 2 };
        match col {
            Col::U8x2 | Col::U16 => 2,
            Col::U32 => 4,
            Col::Str => wide(self.wide_strings),
            Col::Guid => wide(self.wide_guids),
            Col::Blob => wide(self.wide_blobs),
            Col::Idx(table) => wide(self.rows_of(table) > 0xFFFF),
            Col::Coded(kind) => {
                let (bits, tables) = kind.layout();
                let max = tables
                    .iter()
                    .filter(|&&t| t != NONE)
                    .map(|&t| self.rows_of(t))
                    .max()
                    .unwrap_or(0);
                wide(u64::from(max) >= 1u64 << (16 - bits))
            }
        }
    }

    /// One cell of a table: `row` is 1 based, `col` the column number.
    fn cell(&self, table: u8, row: u32, col: usize) -> Option<u32> {
        let slot = usize::from(table);
        if row == 0 || row > self.rows_of(table) {
            return None;
        }
        let cols = schema(table);
        let before: usize = cols.iter().take(col).map(|&c| self.col_size(c)).sum();
        let size = self.col_size(*cols.get(col)?);
        let at = self
            .start
            .get(slot)?
            .checked_add(to_usize(row - 1).checked_mul(*self.row_size.get(slot)?)?)?
            .checked_add(before)?;
        match size {
            2 => u16_at(self.data, at).map(u32::from),
            4 => u32_at(self.data, at),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Resolution of names and bases
// ---------------------------------------------------------------------------------------------

struct Reader<'a> {
    tables: Tables<'a>,
    strings: &'a [u8],
    blob: Option<&'a [u8]>,
    references: Vec<String>,
    /// The final `(namespace, name)` of every type definition, `None` when it is unusable.
    names: Vec<Option<(String, String)>>,
}

impl Reader<'_> {
    fn string(&self, table: u8, row: u32, col: usize) -> Option<String> {
        heap_string(self.strings, self.tables.cell(table, row, col)?)
    }

    /// The type reference at a row, following nested scopes.
    fn external(&self, row: u32) -> Option<ExternalType> {
        let mut names: Vec<String> = Vec::new();
        let mut current = row;
        for _ in 0..=MAX_NESTING {
            let scope = self.tables.cell(TABLE_TYPE_REF, current, 0)?;
            names.push(self.string(TABLE_TYPE_REF, current, 1)?);
            let namespace = self.string(TABLE_TYPE_REF, current, 2)?;
            match Coded::ResolutionScope.decode(scope) {
                Some((TABLE_TYPE_REF, parent)) => current = parent,
                Some((TABLE_ASSEMBLY_REF, r)) => {
                    let assembly = r
                        .checked_sub(1)
                        .and_then(|i| self.references.get(to_usize(i)))
                        .cloned();
                    return Some(finish_external(names, namespace, assembly));
                }
                _ => return Some(finish_external(names, namespace, None)),
            }
        }
        None
    }

    fn base_of(&self, coded: u32, allow_spec: bool) -> BaseType {
        match Coded::TypeDefOrRef.decode(coded) {
            Some((_, 0)) => BaseType::None,
            Some((TABLE_TYPE_DEF, row)) => self
                .names
                .get(to_usize(row).saturating_sub(1))
                .and_then(Option::as_ref)
                .map_or(BaseType::Unresolved, |(ns, name)| {
                    BaseType::Local(join_name(ns, name))
                }),
            Some((TABLE_TYPE_REF, row)) => self
                .external(row)
                .map_or(BaseType::Unresolved, BaseType::External),
            Some((TABLE_TYPE_SPEC, row)) if allow_spec => self.type_spec(row),
            _ => BaseType::Unresolved,
        }
    }

    /// A generic instantiation of a class resolves to the generic class.
    fn type_spec(&self, row: u32) -> BaseType {
        let Some(blob) = self.blob else {
            return BaseType::Unresolved;
        };
        let Some(index) = self.tables.cell(TABLE_TYPE_SPEC, row, 0) else {
            return BaseType::Unresolved;
        };
        let Some(sig) = heap_blob(blob, index) else {
            return BaseType::Unresolved;
        };
        // GENERICINST, then CLASS, then the generic type as a compressed TypeDefOrRef token.
        if sig.first() != Some(&0x15) || sig.get(1) != Some(&0x12) {
            return BaseType::Unresolved;
        }
        match sig.get(2..).and_then(compressed_u32) {
            Some((token, _)) => self.base_of(token, false),
            None => BaseType::Unresolved,
        }
    }
}

fn finish_external(
    mut names: Vec<String>,
    namespace: String,
    assembly: Option<String>,
) -> ExternalType {
    names.reverse();
    ExternalType {
        namespace,
        name: names.join("+"),
        assembly,
    }
}

/// The final name of every type definition: nested types become `Outer+Inner` in the namespace of
/// the outermost type.
fn final_names(
    raw: &[Option<(String, String)>],
    enclosing: &[u32],
) -> Vec<Option<(String, String)>> {
    (0..raw.len())
        .map(|i| {
            let (first_ns, name) = raw.get(i)?.as_ref()?;
            let mut namespace: &String = first_ns;
            let mut path: Vec<&str> = vec![name.as_str()];
            let mut length = name.len();
            let mut current = i;
            let mut depth = 0;
            while let Some(&parent) = enclosing.get(current) {
                if parent == 0 {
                    break;
                }
                depth += 1;
                if depth > MAX_NESTING {
                    return None;
                }
                current = to_usize(parent).checked_sub(1)?;
                let (ns, n) = raw.get(current)?.as_ref()?;
                namespace = ns;
                length += n.len() + 1;
                if length > MAX_FULL_NAME_LEN {
                    return None;
                }
                path.push(n);
            }
            path.reverse();
            Some((namespace.clone(), path.join("+")))
        })
        .collect()
}

/// Reads the types of one managed assembly image.
///
/// Parses the PE image (PE32 and PE32+), the CLI header and the metadata root, the `#~` (or `#-`)
/// tables stream with its heap size flags, and the `#Strings` and `#Blob` heaps. The result lists
/// every type definition in metadata order, with its base type resolved as far as the image allows
/// (see [`BaseType`]).
///
/// # Errors
/// [`AssemblyError`] when the bytes are not a managed PE image, are truncated, or their metadata
/// is inconsistent. Damage confined to single types only skips them
/// ([`AssemblyTypes::skipped`]).
pub fn read_assembly(bytes: &[u8]) -> Result<AssemblyTypes, AssemblyError> {
    let pe = PeImage::parse(bytes)?;
    let metadata = pe.metadata()?;
    let streams = parse_streams(metadata)?;
    let tables = Tables::parse(streams.tables)?;
    let mut reader = Reader {
        tables,
        strings: streams.strings,
        blob: streams.blob,
        references: Vec::new(),
        names: Vec::new(),
    };

    let ref_count = reader.tables.rows_of(TABLE_ASSEMBLY_REF);
    reader.references = (1..=ref_count)
        .map(|row| {
            reader
                .string(TABLE_ASSEMBLY_REF, row, 6)
                .unwrap_or_default()
        })
        .collect();

    let name = match reader.string(TABLE_ASSEMBLY, 1, 7) {
        Some(name) if !name.is_empty() => name,
        _ => {
            let module = reader.string(TABLE_MODULE, 1, 1).unwrap_or_default();
            let lower = module.to_ascii_lowercase();
            if lower.ends_with(".dll") || lower.ends_with(".exe") {
                module
                    .get(..module.len().saturating_sub(4))
                    .unwrap_or("")
                    .to_owned()
            } else {
                module
            }
        }
    };

    let type_count = reader.tables.rows_of(TABLE_TYPE_DEF);
    let raw: Vec<Option<(String, String)>> = (1..=type_count)
        .map(|row| {
            let name = reader.string(TABLE_TYPE_DEF, row, 1)?;
            let namespace = reader.string(TABLE_TYPE_DEF, row, 2)?;
            (!name.is_empty()).then_some((namespace, name))
        })
        .collect();
    let mut enclosing = vec![0u32; raw.len()];
    for row in 1..=reader.tables.rows_of(TABLE_NESTED_CLASS) {
        let nested = reader.tables.cell(TABLE_NESTED_CLASS, row, 0);
        let outer = reader.tables.cell(TABLE_NESTED_CLASS, row, 1);
        if let (Some(nested), Some(outer)) = (nested, outer) {
            let valid = |v: u32| v >= 1 && v <= type_count;
            let slot = enclosing.get_mut(to_usize(nested).saturating_sub(1));
            if let (true, Some(slot)) = (valid(nested) && valid(outer), slot) {
                *slot = outer;
            }
        }
    }
    reader.names = final_names(&raw, &enclosing);

    let mut types = Vec::with_capacity(raw.len());
    let mut skipped = 0;
    for (index, entry) in reader.names.iter().enumerate() {
        let (Some((namespace, tname)), Ok(row)) = (entry, u32::try_from(index + 1)) else {
            skipped += 1;
            continue;
        };
        let flags = reader.tables.cell(TABLE_TYPE_DEF, row, 0).unwrap_or(0);
        let base = reader
            .tables
            .cell(TABLE_TYPE_DEF, row, 3)
            .map_or(BaseType::Unresolved, |coded| reader.base_of(coded, true));
        types.push(TypeDefInfo {
            namespace: namespace.clone(),
            name: tname.clone(),
            flags,
            base,
        });
    }
    Ok(AssemblyTypes {
        name,
        references: reader.references,
        types,
        skipped,
    })
}

// ---------------------------------------------------------------------------------------------
// The type table
// ---------------------------------------------------------------------------------------------

/// Builds the def type table from assemblies in load order.
///
/// A type is kept when it is `root` or derives from it, following base types by full name across
/// all the given assemblies (a base in an assembly that was not given ends the chain). Each kept
/// type carries its base full name (also when the base is not itself in the table, like the
/// parent of the root), its abstract flag and the name of its assembly.
///
/// The order is the assembly load order, then metadata order. A full name defined by two
/// assemblies is listed once, at the position of its first definition, with the data of the last
/// (the later assembly wins, as in the game).
///
/// `short_name_collisions` lists, for every short name found in the global namespace or in one of
/// [`IGNORED_NAMESPACES`] under two or more full names where at least one is a def type, all the
/// full names in load order (non def types included, as in the research prototype). The table
/// then resolves the short name to the last of them.
///
/// # Errors
/// [`AssemblyError::TypeTable`] when the table cannot be assembled (an empty type name).
pub fn build_type_table(
    assemblies: &[AssemblyTypes],
    root: &str,
) -> Result<TypeTable, AssemblyError> {
    // Full name -> the last definition; `order` keeps the first position of each name.
    let mut defs: FxHashMap<String, Entry<'_>> = FxHashMap::default();
    let mut order: Vec<String> = Vec::new();
    for asm in assemblies {
        for ty in &asm.types {
            let full = ty.full_name();
            let entry = Entry {
                assembly: asm.name.as_str(),
                ty,
                base: ty.base.full_name(),
            };
            if defs.insert(full.clone(), entry).is_none() {
                order.push(full);
            }
        }
    }

    let mut derives: FxHashMap<&str, bool> = FxHashMap::default();
    let mut types: Vec<(String, TypeInfo)> = Vec::new();
    for full in &order {
        if !derives_from(full, root, &defs, &mut derives) {
            continue;
        }
        let Some(entry) = defs.get(full) else {
            continue;
        };
        types.push((
            full.clone(),
            TypeInfo {
                base: entry.base.clone(),
                is_abstract: entry.ty.is_abstract(),
                assembly: Some(entry.assembly.to_owned()),
            },
        ));
    }

    let mut by_short: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for full in &order {
        let Some(&Entry { ty, .. }) = defs.get(full) else {
            continue;
        };
        if ty.namespace.is_empty() || IGNORED_NAMESPACES.contains(&ty.namespace.as_str()) {
            by_short
                .entry(ty.short_name())
                .or_default()
                .push(full.as_str());
        }
    }
    let kept: FxHashSet<&str> = types.iter().map(|(n, _)| n.as_str()).collect();
    let collisions: BTreeMap<String, Vec<String>> = by_short
        .into_iter()
        .filter(|(_, names)| names.len() > 1 && names.iter().any(|n| kept.contains(n)))
        .map(|(short, names)| {
            (
                short.to_owned(),
                names.into_iter().map(str::to_owned).collect(),
            )
        })
        .collect();

    let names = assemblies.iter().map(|a| a.name.clone()).collect();
    Ok(TypeTable::from_parts(
        root.to_owned(),
        names,
        types,
        collisions,
    )?)
}

/// One definition seen while building the table.
#[derive(Clone)]
struct Entry<'a> {
    assembly: &'a str,
    ty: &'a TypeDefInfo,
    base: Option<String>,
}

/// True when `name` is the root or has the root among its bases (a cycle or a missing link
/// answers false). Results are memoised.
fn derives_from<'a>(
    name: &'a str,
    root: &str,
    defs: &'a FxHashMap<String, Entry<'a>>,
    memo: &mut FxHashMap<&'a str, bool>,
) -> bool {
    let mut chain: Vec<&'a str> = Vec::new();
    let mut seen: FxHashSet<&'a str> = FxHashSet::default();
    let mut current: Option<&'a str> = Some(name);
    let mut answer = false;
    while let Some(n) = current {
        if let Some(&known) = memo.get(n) {
            answer = known;
            break;
        }
        if !seen.insert(n) {
            break;
        }
        chain.push(n);
        if n == root {
            answer = true;
            break;
        }
        current = defs.get(n).and_then(|e| e.base.as_deref());
    }
    for n in chain {
        memo.insert(n, answer);
    }
    answer
}

// ---------------------------------------------------------------------------------------------
// Where the game assemblies are
// ---------------------------------------------------------------------------------------------

/// The folders that can hold the game assemblies of an install, one per operating system layout
/// (Linux, Windows, macOS application bundle, and the generic Unity name), in that order.
///
/// Pure: the caller checks which of them exists.
#[must_use]
pub fn managed_dirs(install: &Utf8Path) -> Vec<Utf8PathBuf> {
    [
        "RimWorldLinux_Data/Managed",
        "RimWorldWin64_Data/Managed",
        "RimWorldMac.app/Contents/Resources/Data/Managed",
        "RimWorld_Data/Managed",
    ]
    .iter()
    .map(|rel| install.join(rel))
    .collect()
}

/// True for the runtime libraries of a game folder (the framework, Unity, Mono and the bundled
/// third party libraries), which hold no def classes and need not be read.
#[must_use]
pub fn is_runtime_library(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".dll").unwrap_or(&lower);
    stem == "mscorlib"
        || stem == "netstandard"
        || stem.starts_with("system.")
        || stem == "system"
        || stem.starts_with("unityengine")
        || stem.starts_with("unity.")
        || stem.starts_with("mono.")
        || stem.starts_with("com.rlabrecque.")
        || stem == "isharpziplib"
}

/// Sorts game assembly paths into the load order of the type table: `Assembly-CSharp` first, then
/// the others by file name (case insensitive, then exact name, then path).
///
/// Later assemblies win a short name that two of them define, so the main game assembly goes
/// first and everything else is ordered by name to keep the result deterministic.
pub fn sort_game_assemblies(paths: &mut [Utf8PathBuf]) {
    paths.sort_by_cached_key(|p| {
        let stem = p.file_stem().unwrap_or_default();
        (
            stem != GAME_ASSEMBLY,
            stem.to_ascii_lowercase(),
            stem.to_owned(),
            p.clone(),
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_integers_decode_all_widths() {
        assert_eq!(compressed_u32(&[0x03]), Some((3, 1)));
        assert_eq!(compressed_u32(&[0x7F]), Some((0x7F, 1)));
        assert_eq!(compressed_u32(&[0x80, 0x80]), Some((0x80, 2)));
        assert_eq!(compressed_u32(&[0xBF, 0xFF]), Some((0x3FFF, 2)));
        assert_eq!(compressed_u32(&[0xC0, 0x00, 0x40, 0x00]), Some((0x4000, 4)));
        assert_eq!(compressed_u32(&[0xFF]), None);
        assert_eq!(compressed_u32(&[0x80]), None);
        assert_eq!(compressed_u32(&[]), None);
    }

    #[test]
    fn coded_indexes_decode_tag_and_row() {
        assert_eq!(Coded::TypeDefOrRef.decode(0b100), Some((0x02, 1)));
        assert_eq!(Coded::TypeDefOrRef.decode(0b101), Some((0x01, 1)));
        assert_eq!(Coded::TypeDefOrRef.decode(0b110), Some((0x1B, 1)));
        assert_eq!(Coded::TypeDefOrRef.decode(0b111), None);
        assert_eq!(Coded::ResolutionScope.decode((5 << 2) | 2), Some((0x23, 5)));
        assert_eq!(Coded::CustomAttributeType.decode(0), None);
    }

    #[test]
    fn every_table_has_a_schema() {
        for table in 0..=LAST_TABLE {
            assert!(!schema(table).is_empty(), "table {table:#x}");
        }
        assert!(schema(LAST_TABLE + 1).is_empty());
    }

    #[test]
    fn nested_names_join_with_plus_and_loops_are_dropped() {
        let raw = vec![
            Some(("RS_Ns".to_owned(), "RS_Outer".to_owned())),
            Some((String::new(), "RS_Inner".to_owned())),
            Some((String::new(), "RS_Loop".to_owned())),
            Some((String::new(), "RS_Loop2".to_owned())),
        ];
        let names = final_names(&raw, &[0, 1, 4, 3]);
        assert_eq!(
            names.first().cloned().flatten(),
            Some(("RS_Ns".into(), "RS_Outer".into()))
        );
        assert_eq!(
            names.get(1).cloned().flatten(),
            Some(("RS_Ns".into(), "RS_Outer+RS_Inner".into()))
        );
        assert_eq!(names.get(2).cloned().flatten(), None);
        assert_eq!(names.get(3).cloned().flatten(), None);
    }

    #[test]
    fn game_assemblies_sort_with_the_main_one_first() {
        let mut paths: Vec<Utf8PathBuf> = [
            "m/zeta.dll",
            "m/Assembly-CSharp-firstpass.dll",
            "m/Assembly-CSharp.dll",
            "m/Alpha.dll",
        ]
        .iter()
        .map(Utf8PathBuf::from)
        .collect();
        sort_game_assemblies(&mut paths);
        let names: Vec<&str> = paths.iter().filter_map(|p| p.file_name()).collect();
        assert_eq!(
            names,
            vec![
                "Assembly-CSharp.dll",
                "Alpha.dll",
                "Assembly-CSharp-firstpass.dll",
                "zeta.dll"
            ]
        );
    }

    #[test]
    fn runtime_libraries_are_recognised() {
        for name in [
            "mscorlib.dll",
            "System.Core.dll",
            "UnityEngine.CoreModule.dll",
            "Mono.Security.dll",
            "netstandard.dll",
        ] {
            assert!(is_runtime_library(name), "{name}");
        }
        for name in ["Assembly-CSharp.dll", "CombatExtended.dll", "0Harmony.dll"] {
            assert!(!is_runtime_library(name), "{name}");
        }
    }

    #[test]
    fn managed_dirs_cover_every_operating_system_layout() {
        let dirs = managed_dirs(Utf8Path::new("/games/RimWorld"));
        assert_eq!(dirs.len(), 4);
        assert!(dirs.iter().all(|d| d.starts_with("/games/RimWorld")));
        assert!(
            dirs.first()
                .is_some_and(|d| d.ends_with("RimWorldLinux_Data/Managed"))
        );
        assert!(
            dirs.iter()
                .any(|d| d.ends_with("RimWorldMac.app/Contents/Resources/Data/Managed"))
        );
    }
}
