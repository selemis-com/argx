//! Internal Rust type-contract resolution used by built-ins and generated derives.

use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    convert::Infallible,
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use crate::type_contract::{
    PrimitiveType, TYPE_CONTRACT_VERSION, TypeContract, TypeContractValue, TypeDefinition,
    TypeDefinitionKind,
};

/// Semantic identity used only while deduplicating named definitions in one discovery run.
///
/// Built-in Rust forms use structural variants so equivalent standard-library containers share
/// one canonical semantic identity. Derived declarations retain nominal [`TypeId`] identity, with
/// generic type and const arguments included so distinct monomorphizations cannot alias one
/// another accidentally. None of this identity is serialized into the public protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKey {
    /// Unit type.
    Unit,
    /// Exact Rust primitive.
    Primitive(PrimitiveType),
    /// UTF-8 string shape.
    String,
    /// Operating-system-native string shape.
    OsString,
    /// Filesystem path shape.
    Path,
    /// Standard-library uninhabited error type.
    Infallible,
    /// Optional value shape.
    Optional(Box<Self>),
    /// Variable-length sequence shape.
    Sequence(Box<Self>),
    /// Set shape.
    Set(Box<Self>),
    /// Map shape.
    Map(Box<Self>, Box<Self>),
    /// Fixed-length homogeneous array shape.
    Array(Box<Self>, usize),
    /// Fixed-length heterogeneous tuple shape.
    Tuple(Vec<Self>),
    /// One nominal derived Rust declaration.
    Named {
        /// Nominal declaration identity.
        declaration: TypeId,
        /// Canonical contract identities of generic type arguments.
        arguments: Vec<Self>,
        /// Values of const-generic arguments in declaration order.
        const_arguments: Vec<String>,
    },
}

impl TypeKey {
    /// Creates one nominal private type identity from a declaration marker and generic arguments.
    #[must_use]
    pub const fn named<M: 'static>(arguments: Vec<Self>, const_arguments: Vec<String>) -> Self {
        Self::Named { declaration: TypeId::of::<M>(), arguments, const_arguments }
    }
}

/// Converts one stable const-generic value to its private identity component.
///
/// Stable Rust const-generic parameter kinds implement [`fmt::Display`]. The result is internal to
/// one type key and is never serialized as part of the public contract.
#[must_use]
pub fn const_key<T>(value: &T) -> String
where
    T: fmt::Display + ?Sized,
{
    value.to_string()
}

/// Internal resolution contract implemented by supported types and generated derives.
pub trait TypeContractSource {
    /// Resolves this type into the current contract document.
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue;

    /// Returns this type's private structural identity for named-definition deduplication.
    fn type_key() -> TypeKey;
}

/// Mutable state for one type-contract discovery run.
#[derive(Debug, Default)]
pub struct TypeResolver {
    /// Definitions reserved in deterministic first-discovery order.
    definitions: Vec<Option<TypeDefinition>>,
    /// Previously reserved definition indices keyed by private Rust type identity.
    definitions_by_type: HashMap<TypeKey, usize>,
}

impl TypeResolver {
    /// Resolves or reserves one named Rust declaration.
    ///
    /// Reservation happens before `build` executes so self-recursive and mutually recursive types
    /// resolve to references instead of recursing indefinitely.
    pub fn named<F>(
        &mut self,
        key: TypeKey,
        name: &'static str,
        description: Option<&'static str>,
        build: F,
    ) -> TypeContractValue
    where
        F: FnOnce(&mut Self) -> TypeDefinitionKind,
    {
        if let Some(index) = self.definitions_by_type.get(&key).copied() {
            return TypeContractValue::Reference { definition: definition_id(index) };
        }

        let index = self.definitions.len();
        self.definitions_by_type.insert(key, index);
        self.definitions.push(None);

        let kind = build(self);
        self.definitions[index] = Some(TypeDefinition {
            id: definition_id(index),
            name: name.to_owned(),
            description: description.map(str::to_owned),
            kind,
        });

        TypeContractValue::Reference { definition: definition_id(index) }
    }

    /// Completes a discovery run after all reserved definitions have been populated.
    ///
    /// # Panics
    ///
    /// Panics if an internal reservation was not populated before discovery completed.
    pub(crate) fn finish(self) -> Vec<TypeDefinition> {
        self.definitions
            .into_iter()
            .map(|definition| {
                definition.expect("reserved type definitions are populated before discovery ends")
            })
            .collect()
    }
}

/// Builds one complete public type-contract document.
#[must_use]
pub fn discover_type_contract<T>() -> TypeContract
where
    T: TypeContractSource + ?Sized,
{
    let mut resolver = TypeResolver::default();
    let root = T::resolve_type(&mut resolver);
    TypeContract { version: TYPE_CONTRACT_VERSION, root, definitions: resolver.finish() }
}

/// Returns the stable document-local spelling for one definition index.
fn definition_id(index: usize) -> String {
    format!("type-{index}")
}

/// Implements an exact primitive contract with a canonical semantic key.
macro_rules! primitive_contract {
    ($ty:ty, $primitive:ident) => {
        impl TypeContractSource for $ty {
            fn resolve_type(_resolver: &mut TypeResolver) -> TypeContractValue {
                TypeContractValue::Primitive { primitive: PrimitiveType::$primitive }
            }

            fn type_key() -> TypeKey {
                TypeKey::Primitive(PrimitiveType::$primitive)
            }
        }
    };
}

primitive_contract!(bool, Bool);
primitive_contract!(char, Char);
primitive_contract!(i8, I8);
primitive_contract!(i16, I16);
primitive_contract!(i32, I32);
primitive_contract!(i64, I64);
primitive_contract!(i128, I128);
primitive_contract!(isize, Isize);
primitive_contract!(u8, U8);
primitive_contract!(u16, U16);
primitive_contract!(u32, U32);
primitive_contract!(u64, U64);
primitive_contract!(u128, U128);
primitive_contract!(usize, Usize);
primitive_contract!(f32, F32);
primitive_contract!(f64, F64);

impl TypeContractSource for () {
    fn resolve_type(_resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Unit
    }

    fn type_key() -> TypeKey {
        TypeKey::Unit
    }
}

impl TypeContractSource for Infallible {
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        resolver.named(Self::type_key(), "Infallible", None, |_resolver| TypeDefinitionKind::Enum {
            variants: Vec::new(),
        })
    }

    fn type_key() -> TypeKey {
        TypeKey::Infallible
    }
}

/// Implements one semantic leaf contract shared by owned and borrowed standard-library forms.
macro_rules! leaf_contract {
    ($ty:ty, $value:expr, $key:expr) => {
        impl TypeContractSource for $ty {
            fn resolve_type(_resolver: &mut TypeResolver) -> TypeContractValue {
                $value
            }

            fn type_key() -> TypeKey {
                $key
            }
        }
    };
}

leaf_contract!(String, TypeContractValue::String, TypeKey::String);
leaf_contract!(str, TypeContractValue::String, TypeKey::String);
leaf_contract!(OsString, TypeContractValue::OsString, TypeKey::OsString);
leaf_contract!(OsStr, TypeContractValue::OsString, TypeKey::OsString);
leaf_contract!(PathBuf, TypeContractValue::Path, TypeKey::Path);
leaf_contract!(Path, TypeContractValue::Path, TypeKey::Path);

impl<T> TypeContractSource for Option<T>
where
    T: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Optional { value: Box::new(T::resolve_type(resolver)) }
    }

    fn type_key() -> TypeKey {
        TypeKey::Optional(Box::new(T::type_key()))
    }
}

/// Implements one standard-library sequence container.
macro_rules! sequence_contract {
    ($container:ident) => {
        impl<T> TypeContractSource for $container<T>
        where
            T: TypeContractSource,
        {
            fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
                TypeContractValue::Sequence { element: Box::new(T::resolve_type(resolver)) }
            }

            fn type_key() -> TypeKey {
                TypeKey::Sequence(Box::new(T::type_key()))
            }
        }
    };
}

sequence_contract!(Vec);
sequence_contract!(VecDeque);
sequence_contract!(LinkedList);

impl<T> TypeContractSource for [T]
where
    T: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Sequence { element: Box::new(T::resolve_type(resolver)) }
    }

    fn type_key() -> TypeKey {
        TypeKey::Sequence(Box::new(T::type_key()))
    }
}

impl<T, const N: usize> TypeContractSource for [T; N]
where
    T: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Array { element: Box::new(T::resolve_type(resolver)), length: N }
    }

    fn type_key() -> TypeKey {
        TypeKey::Array(Box::new(T::type_key()), N)
    }
}

impl<T> TypeContractSource for BTreeSet<T>
where
    T: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Set { element: Box::new(T::resolve_type(resolver)) }
    }

    fn type_key() -> TypeKey {
        TypeKey::Set(Box::new(T::type_key()))
    }
}

impl<T, S> TypeContractSource for HashSet<T, S>
where
    T: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Set { element: Box::new(T::resolve_type(resolver)) }
    }

    fn type_key() -> TypeKey {
        TypeKey::Set(Box::new(T::type_key()))
    }
}

impl<K, V> TypeContractSource for BTreeMap<K, V>
where
    K: TypeContractSource,
    V: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Map {
            key: Box::new(K::resolve_type(resolver)),
            value: Box::new(V::resolve_type(resolver)),
        }
    }

    fn type_key() -> TypeKey {
        TypeKey::Map(Box::new(K::type_key()), Box::new(V::type_key()))
    }
}

impl<K, V, S> TypeContractSource for HashMap<K, V, S>
where
    K: TypeContractSource,
    V: TypeContractSource,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        TypeContractValue::Map {
            key: Box::new(K::resolve_type(resolver)),
            value: Box::new(V::resolve_type(resolver)),
        }
    }

    fn type_key() -> TypeKey {
        TypeKey::Map(Box::new(K::type_key()), Box::new(V::type_key()))
    }
}

impl<T> TypeContractSource for &T
where
    T: TypeContractSource + ?Sized,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        T::resolve_type(resolver)
    }

    fn type_key() -> TypeKey {
        T::type_key()
    }
}

impl<T> TypeContractSource for &mut T
where
    T: TypeContractSource + ?Sized,
{
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        T::resolve_type(resolver)
    }

    fn type_key() -> TypeKey {
        T::type_key()
    }
}

/// Implements an ownership-only wrapper as semantically transparent.
macro_rules! transparent_contract {
    ($wrapper:ident) => {
        impl<T> TypeContractSource for $wrapper<T>
        where
            T: TypeContractSource + ?Sized,
        {
            fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
                T::resolve_type(resolver)
            }

            fn type_key() -> TypeKey {
                T::type_key()
            }
        }
    };
}

transparent_contract!(Box);
transparent_contract!(Rc);
transparent_contract!(Arc);

/// Implements heterogeneous tuples while preserving element order.
macro_rules! tuple_contract {
    ($($type:ident),+ $(,)?) => {
        impl<$($type),+> TypeContractSource for ($($type,)+)
        where
            $($type: TypeContractSource,)+
        {
            fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
                TypeContractValue::Tuple {
                    elements: vec![$($type::resolve_type(resolver)),+],
                }
            }

            fn type_key() -> TypeKey {
                TypeKey::Tuple(vec![$($type::type_key()),+])
            }
        }
    };
}

tuple_contract!(A);
tuple_contract!(A, B);
tuple_contract!(A, B, C);
tuple_contract!(A, B, C, D);
tuple_contract!(A, B, C, D, E);
tuple_contract!(A, B, C, D, E, F);
tuple_contract!(A, B, C, D, E, F, G);
tuple_contract!(A, B, C, D, E, F, G, H);
tuple_contract!(A, B, C, D, E, F, G, H, I);
tuple_contract!(A, B, C, D, E, F, G, H, I, J);
tuple_contract!(A, B, C, D, E, F, G, H, I, J, K);
tuple_contract!(A, B, C, D, E, F, G, H, I, J, K, L);
