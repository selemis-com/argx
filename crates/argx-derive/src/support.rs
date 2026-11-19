//! Small compile-time helpers shared across derive frontends and code generation.

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Expr, Lit};

/// Returns an identifier without Rust's raw-identifier prefix.
pub(crate) fn ident_name(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

/// Converts the default Rust spelling to Argx's kebab-case command-line spelling.
///
/// Underscores become dashes and every uppercase character begins a new segment.
pub(crate) fn to_kebab(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 4);
    for (index, character) in value.chars().enumerate() {
        if character == '_' {
            output.push('-');
        } else if character.is_uppercase() {
            if index > 0 {
                output.push('-');
            }
            output.extend(character.to_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

/// Derives a stable user-facing spelling for simple defaults without evaluating application code.
pub(crate) fn default_help(expression: &Expr, value_enum: bool) -> Option<String> {
    match expression {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Str(value) => Some(value.value()),
            Lit::Int(value) => Some(value.base10_digits().to_owned()),
            Lit::Float(value) => Some(value.base10_digits().to_owned()),
            _ => None,
        },
        Expr::Unary(expr) if matches!(expr.op, syn::UnOp::Neg(_)) => {
            default_help(&expr.expr, value_enum).map(|value| format!("-{value}"))
        }
        Expr::Path(expr) if value_enum => {
            expr.path.segments.last().map(|segment| to_kebab(&segment.ident.to_string()))
        }
        Expr::Call(expr) if expr.args.len() == 1 => {
            let Expr::Path(function) = &*expr.func else { return None };
            if function.path.segments.last()?.ident != "from" {
                return None;
            }
            let Expr::Lit(argument) = expr.args.first()? else { return None };
            let Lit::Str(value) = &argument.lit else { return None };
            Some(value.value())
        }
        _ => None,
    }
}

/// Returns the canonical absolute Argx path. Renamed dependencies provide a crate-root alias.
pub(crate) fn facade_path() -> TokenStream {
    quote!(::argx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn converts_default_names() {
        assert_eq!(super::to_kebab("HTTPServer"), "h-t-t-p-server");
        assert_eq!(super::to_kebab("output_file"), "output-file");
    }
}
