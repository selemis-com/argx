//! Stable machine-readable contracts for Rust value types.

use serde::Serialize;

/// Current serialized Argx Rust type-contract protocol version.
pub const TYPE_CONTRACT_VERSION: u32 = 1;

/// One versioned machine-readable contract for a Rust value type.
///
/// Definition identifiers are local to this document. Consumers must not persist or compare them
/// across independently generated contracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeContract {
    /// Serialized type-contract protocol version.
    pub version: u32,
    /// Semantic shape of the requested root type.
    pub root: TypeContractValue,
    /// Named Rust declarations referenced by the root, in deterministic discovery order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub definitions: Vec<TypeDefinition>,
}

impl TypeContract {
    /// Serializes this contract as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this contract as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Semantic shape of one Rust value within a type contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeContractValue {
    /// The unit type, `()`.
    Unit,
    /// One scalar Rust primitive.
    Primitive {
        /// Exact Rust primitive represented by this value.
        primitive: PrimitiveType,
    },
    /// UTF-8 text represented by `String` or `str`.
    String,
    /// Operating-system-native text represented by `OsString` or `OsStr`.
    OsString,
    /// A filesystem path represented by `PathBuf` or `Path`.
    Path,
    /// A value that may be absent.
    Optional {
        /// Semantic type present in the `Some` case.
        value: Box<Self>,
    },
    /// An ordered variable-length sequence.
    Sequence {
        /// Element type.
        value: Box<Self>,
    },
    /// An unordered or key-ordered collection of unique values.
    Set {
        /// Element type.
        value: Box<Self>,
    },
    /// A key-value collection.
    Map {
        /// Map key type.
        key: Box<Self>,
        /// Map value type.
        value: Box<Self>,
    },
    /// A fixed-length homogeneous array.
    Array {
        /// Element type.
        value: Box<Self>,
        /// Number of elements.
        length: usize,
    },
    /// A fixed-length heterogeneous tuple.
    Tuple {
        /// Tuple elements in declaration order.
        values: Vec<Self>,
    },
    /// A reference to one named definition in this contract document.
    Reference {
        /// Document-local definition identifier.
        definition: String,
    },
}

/// Exact built-in Rust primitive represented by a type contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PrimitiveType {
    /// `bool`.
    #[serde(rename = "bool")]
    Bool,
    /// `char`.
    #[serde(rename = "char")]
    Char,
    /// `i8`.
    #[serde(rename = "i8")]
    I8,
    /// `i16`.
    #[serde(rename = "i16")]
    I16,
    /// `i32`.
    #[serde(rename = "i32")]
    I32,
    /// `i64`.
    #[serde(rename = "i64")]
    I64,
    /// `i128`.
    #[serde(rename = "i128")]
    I128,
    /// `isize`.
    #[serde(rename = "isize")]
    Isize,
    /// `u8`.
    #[serde(rename = "u8")]
    U8,
    /// `u16`.
    #[serde(rename = "u16")]
    U16,
    /// `u32`.
    #[serde(rename = "u32")]
    U32,
    /// `u64`.
    #[serde(rename = "u64")]
    U64,
    /// `u128`.
    #[serde(rename = "u128")]
    U128,
    /// `usize`.
    #[serde(rename = "usize")]
    Usize,
    /// `f32`.
    #[serde(rename = "f32")]
    F32,
    /// `f64`.
    #[serde(rename = "f64")]
    F64,
}

/// One named Rust declaration referenced by a type contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinition {
    /// Identifier used by [`TypeContractValue::Reference`] within this document.
    pub id: String,
    /// Rust declaration name without module or generic-argument qualification.
    pub name: String,
    /// First paragraph of Rust documentation attached to this declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Structural kind and contents of this declaration.
    #[serde(flatten)]
    pub kind: TypeDefinitionKind,
}

/// Structural kind of one named Rust declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeDefinitionKind {
    /// A struct with named fields.
    Struct {
        /// Fields in declaration order.
        fields: Vec<TypeFieldContract>,
    },
    /// A tuple struct.
    TupleStruct {
        /// Fields in declaration order.
        fields: Vec<TypeFieldContract>,
    },
    /// A unit struct.
    UnitStruct,
    /// An enum.
    Enum {
        /// Variants in declaration order.
        variants: Vec<TypeVariantContract>,
    },
}

/// One field in a struct, tuple struct, or enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeFieldContract {
    /// Rust field name for named fields; omitted for tuple fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// First paragraph of Rust documentation attached to this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic field type.
    pub value: TypeContractValue,
}

/// One enum variant in a named type definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeVariantContract {
    /// Rust variant name.
    pub name: String,
    /// First paragraph of Rust documentation attached to this variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Variant payload shape.
    #[serde(flatten)]
    pub kind: TypeVariantKind,
}

/// Payload shape of one enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeVariantKind {
    /// A unit variant.
    Unit,
    /// A tuple variant.
    Tuple {
        /// Fields in declaration order.
        fields: Vec<TypeFieldContract>,
    },
    /// A struct variant with named fields.
    Struct {
        /// Fields in declaration order.
        fields: Vec<TypeFieldContract>,
    },
}

/// Marks a Rust type that can produce an Argx machine-readable type contract.
///
/// Implementations are provided for supported standard-library types and by
/// `#[derive(argx::Contract)]`. Manual implementations are not part of Argx's stable extension
/// surface.
pub trait ContractType: crate::__private::TypeContractSource {
    /// Discovers the complete semantic contract for this Rust type.
    #[must_use]
    fn type_contract() -> TypeContract {
        crate::__private::discover_type_contract::<Self>()
    }
}

impl<T> ContractType for T where T: crate::__private::TypeContractSource + ?Sized {}
