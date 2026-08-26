//! Naming helpers used when command-line spellings are inferred from Rust identifiers.
//!
//! Inference is intentionally mechanical rather than acronym-aware: every uppercase character
//! starts a segment, so `HTTPServer` becomes `h-t-t-p-server`. Explicit `name` or `long` metadata
//! is the escape hatch when a product-specific spelling should differ from this stable rule.

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

#[cfg(test)]
mod tests {
    #[test]
    fn converts_default_names() {
        assert_eq!(super::to_kebab("HTTPServer"), "h-t-t-p-server");
        assert_eq!(super::to_kebab("output_file"), "output-file");
    }
}
