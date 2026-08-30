//! Canonical semantic model built from derive input before code generation.
//!
//! CLI meaning is normalized here exactly once. Code generation then projects that meaning into
//! shared static command tables while Rust-specific construction, value conversion, and semantic
//! type resolution remain separate. Future projections should extend this model rather than
//! reinterpret attributes or generated command tables.
//!
//! Validation is split between declaration-local checks that the proc macro can resolve directly
//! and composition checks emitted into generated constants. The latter are necessary for flattened
//! `Args` declarations because one macro expansion cannot introspect another expansion's fields.

use proc_macro2::Span;
use syn::Type;

mod command;
mod shape;
mod subcommand;

pub(crate) use shape::Shape;

/// One normalized command declaration.
pub(crate) struct Command {
    /// Rust-facing information required to implement the derived type.
    pub binding: CommandBinding,
    /// Command-line semantics shared by every generated projection.
    pub semantics: CommandSemantics,
    /// Fields in declaration order.
    pub fields: Vec<Field>,
}

/// Rust-facing information for a `Parser` or `Args` declaration.
pub(crate) struct CommandBinding {
    /// Rust type name receiving the generated implementations.
    pub ident: syn::Ident,
    /// Visibility of the derived declaration.
    pub visibility: syn::Visibility,
    /// Generic parameters copied to generated implementations.
    pub generics: syn::Generics,
    /// Whole declaration token stream used to seed stable semantic identities.
    pub fingerprint: String,
    /// Whether the public `Parser` trait is implemented in addition to `CommandArgs`.
    pub root: bool,
    /// Whether the declaration is a unit struct.
    pub unit: bool,
}

/// CLI semantics common to root commands and selectable subcommands.
pub(crate) struct CommandSemantics {
    /// Command-line name represented by this declaration.
    pub name: String,
    /// One-line help summary for this command.
    pub about: Option<String>,
    /// Full prose rendered before generated command help sections.
    pub description: Option<String>,
    /// User-authored help sections from Rust doc comments.
    pub help_sections: Vec<HelpSection>,
    /// Short version text expression.
    pub version: Option<syn::Expr>,
    /// Long version text expression.
    pub long_version: Option<syn::Expr>,
    /// Hidden command spellings accepted in addition to the canonical name.
    pub aliases: Vec<String>,
    /// Whether this command declaration participates in machine-readable schema discovery.
    pub schema: bool,
}

/// One user-authored command help section.
pub(crate) struct HelpSection {
    /// Section heading.
    pub heading: String,
    /// Section body.
    pub body: String,
}

/// One enum deriving `Subcommand`.
pub(crate) struct Subcommand {
    /// Rust-facing information required to implement the enum.
    pub binding: SubcommandBinding,
    /// Variants in declaration order.
    pub variants: Vec<Variant>,
}

/// Rust-facing information for a `Subcommand` declaration.
pub(crate) struct SubcommandBinding {
    /// Rust enum receiving the generated implementation.
    pub ident: syn::Ident,
    /// Generic parameters copied to generated implementations.
    pub generics: syn::Generics,
    /// Whole declaration token stream used to seed stable variant identities.
    pub fingerprint: String,
    /// Whether this structural command set participates in schema discovery.
    pub schema: bool,
}

/// One selectable subcommand variant.
pub(crate) struct Variant {
    /// Rust-facing variant construction information.
    pub binding: VariantBinding,
    /// Command-line semantics of this selectable command.
    pub semantics: CommandSemantics,
}

/// Rust-facing information for one subcommand variant.
pub(crate) struct VariantBinding {
    /// Rust enum variant name.
    pub ident: syn::Ident,
    /// Optional reusable `Args` payload.
    pub payload: Option<Type>,
}

/// One named Rust field after CLI semantics have been normalized.
pub(crate) struct Field {
    /// Rust-facing information used to construct the destination value.
    pub binding: FieldBinding,
    /// CLI meaning of this field.
    pub semantics: FieldSemantics,
    /// Help-section heading applied when this field flattens another `Args` declaration.
    pub help_heading: Option<String>,
}

/// Rust-facing information for one destination field.
pub(crate) struct FieldBinding {
    /// Rust field identifier used when generated code builds the destination value.
    pub ident: syn::Ident,
    /// Declared Rust field type.
    pub ty: Type,
    /// Source span used for declaration-level diagnostics.
    pub span: Span,
    /// Field name without Rust raw-identifier syntax, used by binding diagnostics.
    pub name: String,
    /// Normalized typed-value conversion, when this field binds CLI values directly.
    pub value: Option<ValueBinding>,
    /// Typed Rust expression used when this argument is absent.
    pub default: Option<syn::Expr>,
}

/// Rust conversion information for a value-bearing field.
pub(crate) struct ValueBinding {
    /// Rust type receiving one parsed value.
    pub ty: Type,
    /// Conversion strategy selected from the destination type.
    pub conversion: ValueConversion,
    /// Optional ecosystem type recognized for invocation schema projection.
    pub schema: ValueSchema,
    /// Whether a repeated field preserves absence with an outer `Option`.
    pub optional_collection: bool,
}

/// Schema-relevant semantic type recognized from one destination value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueSchema {
    /// Ordinary lexical string value.
    Lexical,
    /// `chrono::NaiveDate`.
    Date,
    /// `chrono::DateTime<_>`.
    DateTime,
    /// `uuid::Uuid`.
    Uuid,
    /// `url::Url`.
    Url,
}

/// Conversion strategy for one raw CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueConversion {
    /// Preserve UTF-8 text directly as `String`.
    Text,
    /// Reconstruct an operating-system string before converting the destination.
    Os,
    /// Parse UTF-8 text through `FromStr`.
    FromStr,
}

/// CLI role represented by one Rust field.
#[expect(
    clippy::large_enum_variant,
    reason = "derive-time model; boxing would add indirection without reducing meaningful runtime cost"
)]
pub(crate) enum FieldSemantics {
    /// One named or positional CLI argument.
    Argument(Argument),
    /// Another independently derived `Args` declaration composed inline.
    Flatten,
    /// A required nested command selected from a derived subcommand enum.
    Subcommand,
}

/// Normalized semantics for one named or positional argument.
pub(crate) struct Argument {
    /// One-line help summary for this argument.
    pub help: Option<String>,
    /// Full help prose for this argument.
    pub long_help: Option<String>,
    /// Whether the argument is named or positional on the command line.
    pub kind: ArgumentKind,
    /// Canonical user-facing label used by diagnostics.
    pub diagnostic: String,
    /// Whether a named argument remains in scope for descendant commands.
    pub global: bool,
    /// Syntactic value shape relevant to CLI cardinality.
    pub shape: Shape,
    /// Whether absence is satisfied by a typed Rust default.
    pub has_default: bool,
    /// Static user-facing spelling of the declared default, when derivable from syntax.
    pub default_value: Option<String>,
    /// Field names that must be satisfied when this argument is supplied.
    pub requires: Vec<String>,
    /// Field names that cannot be supplied together with this argument.
    pub conflicts: Vec<String>,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
    /// Whether this argument is restricted to a finite `ValueEnum` vocabulary.
    pub value_enum: bool,
    /// Whether this value-less flag binds its occurrence count.
    pub count: bool,
    /// Whether collection values are split on commas before conversion.
    pub delimited: bool,
}

/// Command-line category of one argument.
pub(crate) enum ArgumentKind {
    /// A named flag with one or more spellings.
    Flag {
        /// Canonical long spellings without `--`.
        longs: Vec<String>,
        /// Hidden long aliases without `--`.
        aliases: Vec<String>,
        /// Short spellings as ASCII bytes.
        shorts: Vec<u8>,
    },
    /// A positional argument.
    Positional,
}

/// One generic parameter name relevant while inspecting a composed field type.
#[derive(Debug)]
enum GenericName {
    /// Type or const parameter identifier.
    Ident(syn::Ident),
    /// Lifetime parameter identifier without the apostrophe.
    Lifetime(syn::Ident),
}

/// Visitor that detects use of one containing generic parameter inside a flattened type.
#[derive(Debug)]
struct GenericUse<'a> {
    /// Generic names declared by the containing struct.
    params: &'a [GenericName],
    /// Whether a matching parameter was encountered.
    found: bool,
}

impl<'ast> syn::visit::Visit<'ast> for GenericUse<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if let Some(first) = path.path.segments.first()
            && self
                .params
                .iter()
                .any(|param| matches!(param, GenericName::Ident(name) if name == &first.ident))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_type_path(self, path);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(first) = path.path.segments.first()
            && self
                .params
                .iter()
                .any(|param| matches!(param, GenericName::Ident(name) if name == &first.ident))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if self
            .params
            .iter()
            .any(|param| matches!(param, GenericName::Lifetime(name) if name == &lifetime.ident))
        {
            self.found = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::{DeriveInput, parse_quote};

    use super::*;

    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers construct owned syntax trees solely for one validation"
    )]
    fn command_error(input: DeriveInput, root: bool) -> String {
        Command::from_input(&input, root)
            .err()
            .expect("command model should be rejected")
            .to_string()
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers construct owned syntax trees solely for one validation"
    )]
    fn subcommand_error(input: DeriveInput) -> String {
        Subcommand::from_input(&input)
            .err()
            .expect("subcommand model should be rejected")
            .to_string()
    }

    #[test]
    fn command_model_separates_cli_semantics_from_rust_binding() {
        let input: DeriveInput = parse_quote! {
            /// Example command.
            #[argx(name = "example")]
            struct Cli {
                /// Enable verbose output.
                #[argx(short, long, global)]
                verbose: bool,
                #[argx(long, default = std::path::PathBuf::from("out"))]
                output: Option<std::path::PathBuf>,
                #[argx(flatten)]
                shared: Shared,
                #[argx(subcommand)]
                command: Commands,
            }
        };

        let command = Command::from_input(&input, true).expect("command model should be valid");
        assert_eq!(command.semantics.name, "example");
        assert_eq!(command.semantics.about.as_deref(), Some("Example command."));
        assert!(command.binding.root);

        let verbose = &command.fields[0];
        let Some(argument) = verbose.argument() else {
            panic!("verbose should be an argument");
        };
        assert!(matches!(&argument.kind, ArgumentKind::Flag { .. }));
        assert_eq!(argument.diagnostic, "--verbose");
        assert!(argument.global);
        assert_eq!(argument.shape, Shape::Bool);
        assert!(verbose.binding.value.is_none());

        let output = &command.fields[1];
        let Some(argument) = output.argument() else {
            panic!("output should be an argument");
        };
        assert!(matches!(&argument.kind, ArgumentKind::Flag { .. }));
        assert_eq!(argument.diagnostic, "--output");
        assert_eq!(argument.shape, Shape::Optional);
        assert!(argument.has_default);
        assert_eq!(output.value_binding().conversion, ValueConversion::Os);
        assert!(output.binding.default.is_some());

        assert!(matches!(&command.fields[2].semantics, FieldSemantics::Flatten));
        assert!(command.fields[2].binding.value.is_none());
        assert!(matches!(&command.fields[3].semantics, FieldSemantics::Subcommand));
        assert!(command.fields[3].binding.value.is_none());
    }

    #[test]
    fn command_model_recognizes_schema_value_types() {
        let input: DeriveInput = parse_quote! {
            struct Cli {
                at: chrono::DateTime<chrono::Utc>,
                date: chrono::NaiveDate,
                time: chrono::NaiveTime,
                local: chrono::NaiveDateTime,
                id: Uuid,
                #[argx(long)]
                url: Option<url::Url>,
                value: String,
            }
        };

        let command = Command::from_input(&input, true).expect("command model should be valid");

        assert_eq!(command.fields[0].value_binding().schema, ValueSchema::DateTime);
        assert_eq!(command.fields[1].value_binding().schema, ValueSchema::Date);
        assert_eq!(command.fields[2].value_binding().schema, ValueSchema::Lexical);
        assert_eq!(command.fields[3].value_binding().schema, ValueSchema::Lexical);
        assert_eq!(command.fields[4].value_binding().schema, ValueSchema::Uuid);
        assert_eq!(command.fields[5].value_binding().schema, ValueSchema::Url);
        assert_eq!(command.fields[6].value_binding().schema, ValueSchema::Lexical);
    }

    #[test]
    fn composed_fields_reject_lifetime_and_const_dependencies() {
        let lifetime_input: DeriveInput = parse_quote! {
            struct Cli<'a> {
                #[argx(flatten)]
                shared: Shared<'a>,
            }
        };
        let error = Command::from_input(&lifetime_input, true)
            .err()
            .expect("flattened type must not depend on the containing lifetime");
        assert!(error.to_string().contains("`flatten` cannot depend"));

        let const_input: DeriveInput = parse_quote! {
            struct Cli<const N: usize> {
                #[argx(flatten)]
                shared: Shared<N>,
            }
        };
        let error = Command::from_input(&const_input, true)
            .err()
            .expect("flattened type must not depend on the containing const parameter");
        assert!(error.to_string().contains("`flatten` cannot depend"));

        let const_expression_input: DeriveInput = parse_quote! {
            struct Cli<const N: usize> {
                #[argx(flatten)]
                shared: Shared<{ N }>,
            }
        };
        let error = Command::from_input(&const_expression_input, true)
            .err()
            .expect("flattened const expressions must detect containing const parameters");
        assert!(error.to_string().contains("`flatten` cannot depend"));

        let subcommand_lifetime_input: DeriveInput = parse_quote! {
            enum Commands<'a> {
                Run(Shared<'a>),
            }
        };
        let error = Subcommand::from_input(&subcommand_lifetime_input)
            .err()
            .expect("payload must not depend on the containing lifetime");
        assert!(error.to_string().contains("subcommand payload cannot depend"));

        let subcommand_const_input: DeriveInput = parse_quote! {
            enum Commands<const N: usize> {
                Run(Shared<N>),
            }
        };
        let error = Subcommand::from_input(&subcommand_const_input)
            .err()
            .expect("payload must not depend on the containing const parameter");
        assert!(error.to_string().contains("subcommand payload cannot depend"));
    }

    #[test]
    fn command_and_subcommand_names_reject_ambiguous_token_spellings() {
        let empty_command: DeriveInput = parse_quote! {
            #[argx(name = "")]
            struct Cli;
        };
        assert_eq!(
            Command::from_input(&empty_command, true)
                .err()
                .expect("empty command name must fail")
                .to_string(),
            "command name cannot be empty",
        );

        let whitespace_subcommand: DeriveInput = parse_quote! {
            enum Commands {
                #[argx(name = "bad name")]
                Run,
            }
        };
        let error = Subcommand::from_input(&whitespace_subcommand)
            .err()
            .expect("whitespace in a subcommand name must fail");
        assert!(error.to_string().contains("subcommand name must be non-empty"));

        let equals_alias: DeriveInput = parse_quote! {
            enum Commands {
                #[argx(alias = "run=now")]
                Run,
            }
        };
        let error = Subcommand::from_input(&equals_alias)
            .err()
            .expect("equals signs in subcommand aliases must fail");
        assert!(error.to_string().contains("subcommand name must be non-empty"));
    }

    #[test]
    fn command_declarations_reject_unsupported_shapes_and_metadata() {
        let error = command_error(
            parse_quote!(
                enum Cli {
                    Run,
                }
            ),
            true,
        );
        assert_eq!(error, "Parser can only be derived for structs");

        let error = command_error(
            parse_quote!(
                struct Cli(String);
            ),
            true,
        );
        assert_eq!(error, "Parser and Args do not support tuple structs; use named fields");

        let error = command_error(
            parse_quote! {
                #[argx(alias = "tool")]
                struct Cli;
            },
            true,
        );
        assert_eq!(error, "command aliases are only valid on Subcommand variants");

        let error = command_error(
            parse_quote! {
                #[argx(version = "1.0")]
                struct Shared;
            },
            false,
        );
        assert_eq!(
            error,
            "version metadata is only valid on Parser declarations and Subcommand variants",
        );
    }

    #[test]
    fn subcommand_declarations_reject_invalid_variants_and_payloads() {
        let error = subcommand_error(parse_quote!(
            struct Commands;
        ));
        assert_eq!(error, "Subcommand can only be derived for enums");

        let error = subcommand_error(parse_quote!(
            enum Commands {}
        ));
        assert_eq!(error, "Subcommand requires at least one variant");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                #[argx(name = "same")]
                First,
                #[argx(name = "same")]
                Second,
            }
        });
        assert_eq!(error, "duplicate subcommand `same`");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                #[argx(alias = "run")]
                Run,
            }
        });
        assert_eq!(error, "duplicate subcommand spelling `run`");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run(Option<Shared>),
            }
        });
        assert_eq!(error, "subcommand payload must be one direct Args type");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run(Shared, Other),
            }
        });
        assert_eq!(error, "subcommand tuple variants must contain exactly one Args payload");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run { shared: Shared },
            }
        });
        assert_eq!(
            error,
            "subcommand variants support only unit variants or one unnamed Args payload",
        );

        let error = subcommand_error(parse_quote! {
            #[argx(schema)]
            enum Commands {
                Status,
            }
        });
        assert_eq!(
            error,
            "`#[argx(schema)]` requires executable subcommands to use a concrete Args payload; use an empty Args struct instead of a unit variant",
        );

        let error = command_error(
            parse_quote! {
                #[argx(schema)]
                struct Leaf {
                    value: String,
                }
            },
            false,
        );
        assert_eq!(
            error,
            "`#[argx(schema)]` on Args requires a `#[argx(subcommand)]` field; executable leaves use `#[argx(handler = ...)]`",
        );
    }

    #[test]
    fn composed_fields_reject_incompatible_roles_attributes_and_wrappers() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, subcommand)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` and `subcommand` cannot be combined");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, requires = "value")]
                    shared: Shared,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` and `conflicts` are only valid on argument fields");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand, long)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`subcommand` cannot be combined with flag, value, or help attributes");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    command: Option<Commands>,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "`subcommand` does not support `Option<T>`; hold the Subcommand enum directly",
        );

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, delimited)]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`delimited` requires a collection field");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    command: Vec<Commands>,
                }
            },
            true,
        );
        assert_eq!(error, "`subcommand` does not support collection wrappers");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, long)]
                    shared: Shared,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` cannot be combined with flag, value, or help attributes");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten)]
                    shared: Option<Shared>,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` does not support `Option<T>`; hold the Args struct directly");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten)]
                    shared: Vec<Shared>,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "`flatten` does not support collection wrappers; hold one Args struct directly",
        );
    }

    #[test]
    fn argument_fields_reject_incompatible_flag_and_value_policies() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(alias = "other")]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`alias` and `aliases` are only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(allow_hyphen_values)]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`allow_hyphen_values` is only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(global)]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`global` is only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, allow_negative_numbers)]
                    verbose: bool,
                }
            },
            true,
        );
        assert_eq!(error, "value policies are not valid on bool fields");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, default = true)]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`default` is only supported on scalar value-taking flags");
    }

    #[test]
    fn command_wide_validation_rejects_reserved_duplicate_and_ambiguous_layouts() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "help")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`--help` is reserved by Argx");

        let error = command_error(
            parse_quote! {
                #[argx(schema)]
                struct Cli {
                    #[argx(long = "schema")]
                    value: Option<String>,
                }
            },
            true,
        );
        assert_eq!(error, "`--schema` is reserved when schema discovery is enabled");

        let error = command_error(
            parse_quote! {
                #[argx(schema)]
                struct Cli {
                    #[argx(short = 'S')]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`-S` is reserved when schema discovery is enabled");

        assert!(
            Command::from_input(
                &parse_quote! {
                    struct Cli {
                        #[argx(long = "schema", short = 'S')]
                        value: Option<String>,
                    }
                },
                true,
            )
            .is_ok()
        );

        let error = command_error(
            parse_quote! {
                #[argx(version = "1.0")]
                struct Cli {
                    #[argx(short = 'V')]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`-V` is reserved when command version metadata is present");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "same")]
                    first: bool,
                    #[argx(long = "same")]
                    second: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate long flag `--same`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(short = 'x')]
                    first: bool,
                    #[argx(short = 'x')]
                    second: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate short flag `-x`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    optional: Option<String>,
                    required: String,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "required positional arguments cannot follow optional positional arguments"
        );

        let error = command_error(
            parse_quote! {
                struct Cli {
                    values: Vec<String>,
                    later: Option<String>,
                }
            },
            true,
        );
        assert_eq!(error, "variadic positional argument must be the last positional argument");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    first: Commands,
                    #[argx(subcommand)]
                    second: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "a command can contain only one `subcommand` field");
    }

    #[test]
    fn constraint_validation_rejects_invalid_local_relationships() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` must name a Rust argument field");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "value")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` cannot reference its own field `value`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "token", requires = "token")]
                    value: bool,
                    #[argx(long)]
                    token: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate `requires` reference `token`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "token", conflicts = "token")]
                    value: bool,
                    #[argx(long)]
                    token: bool,
                }
            },
            true,
        );
        assert_eq!(error, "argument `value` cannot both require and conflict with `token`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "command")]
                    value: bool,
                    #[argx(subcommand)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` target `command` is not an argument field");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, conflicts = "missing")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`conflicts` names no argument field `missing` in this command");
    }

    #[test]
    fn spelling_validation_rejects_invalid_values() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(short = '=')]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "short flag must be one visible ASCII character other than `-` or `=`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "bad name")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls",
        );
    }
}
