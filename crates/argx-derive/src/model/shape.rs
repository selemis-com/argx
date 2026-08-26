//! Rust value-shape recognition for generated binding.
//!
//! Procedural macros cannot resolve aliases or trait semantics, so Argx recognizes only direct
//! standard-path spellings of `bool`, `Option`, and `Vec`. This module keeps that syntactic rule in
//! one place; conversion strategy is selected later after the outer cardinality wrappers are peeled.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{GenericArgument, PathArguments, Type};

/// What a field's Rust type says about how many values it can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// Bare `bool`, which is a switch when used as a flag.
    Bool,
    /// `Option<T>`: zero or one value.
    Optional,
    /// A bare value type: exactly one value.
    Required,
    /// `Vec<T>` or `Option<Vec<T>>`: zero or more values.
    Many,
}

impl Shape {
    /// Infers the value shape from the outer standard collection wrappers.
    pub(super) fn from_type(ty: &Type) -> Self {
        if matches!(
            rendered_path(ty).as_str(),
            "bool"
                | "std::primitive::bool"
                | "::std::primitive::bool"
                | "core::primitive::bool"
                | "::core::primitive::bool"
        ) {
            return Self::Bool;
        }
        if peel_option(ty).and_then(peel_vec).is_some() {
            return Self::Many;
        }
        if peel_option(ty).is_some() {
            return Self::Optional;
        }
        if peel_vec(ty).is_some() {
            return Self::Many;
        }
        Self::Required
    }
}

/// Rejects collection nestings that the typed binding model does not define.
pub(super) fn validate_value_shape(ty: &Type, shape: Shape, span: Span) -> syn::Result<()> {
    let value = match shape {
        Shape::Bool | Shape::Required => return Ok(()),
        Shape::Optional => peel_option(ty).expect("optional shape must contain Option"),
        Shape::Many => {
            let collection = peel_option(ty).unwrap_or(ty);
            peel_vec(collection).expect("many shape must contain Vec")
        }
    };

    if peel_option(value).is_some() || peel_vec(value).is_some() {
        return Err(syn::Error::new(
            span,
            "nested Option and Vec value wrappers are not supported",
        ));
    }
    Ok(())
}

/// Peels one standard `Option` wrapper.
pub(super) fn peel_option(ty: &Type) -> Option<&Type> {
    peel_standard(
        ty,
        &[
            "Option",
            "std::option::Option",
            "::std::option::Option",
            "core::option::Option",
            "::core::option::Option",
        ],
    )
}

/// Peels one standard `Vec` wrapper.
pub(super) fn peel_vec(ty: &Type) -> Option<&Type> {
    peel_standard(
        ty,
        &["Vec", "std::vec::Vec", "::std::vec::Vec", "alloc::vec::Vec", "::alloc::vec::Vec"],
    )
}

/// Returns a type path as written with token-stream spacing removed.
pub(super) fn rendered_path(ty: &Type) -> String {
    ty.to_token_stream().to_string().replace(' ', "")
}

/// Peels one recognized standard generic wrapper and returns its sole type argument.
fn peel_standard<'a>(ty: &'a Type, accepted: &[&str]) -> Option<&'a Type> {
    if !accepted.contains(&rendered_path(ty).split('<').next()?) {
        return None;
    }

    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut arguments = arguments.args.iter();
    let GenericArgument::Type(inner) = arguments.next()? else {
        return None;
    };
    if arguments.next().is_some() {
        return None;
    }
    Some(inner)
}
