//! Derive-generated static-model projection tests.
//!
//! These tests inspect the hidden metadata ABI shared by `argx-derive` and the runtime. They protect
//! composition order, semantic identities, normalized constraints, help metadata, and subcommand
//! tables before parsing begins. Assertions about key layout are implementation invariants of this
//! ABI rather than user-facing CLI semantics; end-to-end binding behavior is covered separately in
//! `parser.rs`.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #![expect(dead_code, reason = "fixtures are exercised through generated metadata")]

    use argx::__private::{Arg, Command, CommandArgs as _, Event, Flag};

    /// Documentation that is overridden by explicit command help.
    #[derive(argx::Parser)]
    #[argx(name = "example", about = "Example command")]
    struct Cli {
        /// Enable verbose output.
        #[argx(short, long, global)]
        verbose: bool,
        /// Documentation overridden by explicit field help.
        #[argx(long = "output", help = "Optional output path")]
        output: Option<String>,
        /// Input value to process.
        input: String,
        rest: Vec<String>,
    }

    #[derive(argx::Args)]
    struct Shared {
        #[argx(long)]
        shared: bool,
        middle: String,
    }

    #[derive(argx::Args)]
    struct Nested {
        #[argx(long)]
        nested: bool,
        #[argx(flatten)]
        shared: Shared,
    }

    #[derive(argx::Parser)]
    struct Flattened {
        #[argx(long)]
        before_flag: bool,
        before: String,
        #[argx(flatten)]
        nested: Nested,
        #[argx(long)]
        after_flag: bool,
        after: String,
    }

    #[derive(argx::Args)]
    struct AddArgs {
        #[argx(long)]
        force: bool,
        value: String,
    }

    #[derive(argx::Args)]
    struct NestedArgs {
        #[argx(subcommand)]
        command: NestedCommand,
    }

    #[derive(argx::Subcommand)]
    enum NestedCommand {
        Leaf,
    }

    #[derive(argx::Subcommand)]
    enum Commands {
        /// Add one value.
        Add(AddArgs),
        /// Documentation overridden by explicit command help.
        #[argx(about = "Remove one value")]
        Remove(AddArgs),
        Nested(NestedArgs),
        Status,
    }

    #[derive(argx::Parser)]
    struct CommandCli {
        #[argx(long)]
        verbose: bool,
        #[argx(subcommand)]
        command: Commands,
    }

    const VERSION: &str = "1.2.3";
    const LONG_VERSION: &str = "1.2.3 (build abc123)";

    #[derive(argx::Subcommand)]
    enum MetadataCommands {
        #[argx(version = VERSION, long_version = LONG_VERSION)]
        Versioned,
        Plain,
    }

    #[derive(argx::Parser)]
    #[argx(version = VERSION, long_version = LONG_VERSION)]
    struct MetadataCli {
        #[argx(subcommand)]
        command: MetadataCommands,
    }

    #[derive(argx::Args)]
    struct ConstraintShared {
        #[argx(long = "auth-token", alias = "token")]
        token: Option<String>,
    }

    #[derive(argx::Parser)]
    struct ConstraintCli {
        #[argx(long, requires = "token", conflicts = "stdout")]
        endpoint: Option<String>,
        #[argx(flatten)]
        shared: ConstraintShared,
        #[argx(long)]
        stdout: bool,
    }

    #[derive(argx::Args)]
    struct GroupedModelArgs {
        #[argx(long)]
        json: bool,
        #[argx(long)]
        field: Vec<String>,
    }

    #[derive(argx::Parser)]
    struct GroupedModel {
        /// Output
        #[argx(flatten)]
        output: GroupedModelArgs,
    }

    /// Documented command summary.
    ///
    /// Additional command context remains available to full help.
    ///
    /// # Examples
    ///
    /// example run
    ///
    /// # Machine-readable usage
    ///
    /// Use `example schema`.
    #[derive(argx::Parser)]
    struct DocumentedModel;

    mod add {
        #[derive(argx::Args)]
        pub(super) struct Options {
            #[argx(long)]
            pub(super) force: bool,
        }
    }

    mod remove {
        #[derive(argx::Args)]
        pub(super) struct Options {
            #[argx(long)]
            pub(super) force: bool,
        }
    }

    mod command_a {
        #[derive(argx::Subcommand)]
        pub(super) enum Action {
            Run,
        }
    }

    mod command_b {
        #[derive(argx::Subcommand)]
        pub(super) enum Action {
            Run,
        }
    }

    #[test]
    fn parser_derive_emits_static_command_tables() {
        let command = Cli::COMMAND;
        assert_eq!(command.name, "example");
        assert_eq!(command.about, Some("Example command"));
        assert!(command.subcommands.is_empty());

        let &[verbose, output] = command.flags else {
            panic!("expected verbose and output flags");
        };
        assert_eq!(
            *verbose,
            Flag {
                key: verbose.key,
                name: "verbose",
                diagnostic: "--verbose",
                help: Some("Enable verbose output."),
                longs: &["verbose"],
                shorts: b"v",
                global: true,
                ..Flag::BOOL
            },
        );
        assert_eq!(
            *output,
            Flag {
                key: output.key,
                name: "output",
                diagnostic: "--output",
                help: Some("Optional output path"),
                longs: &["output"],
                ..Flag::VALUE
            },
        );

        let &[input, rest] = command.args else {
            panic!("expected input and rest positionals");
        };
        assert_eq!(
            *input,
            Arg {
                key: input.key,
                name: "input",
                help: Some("Input value to process."),
                ..Arg::REQUIRED
            },
        );
        assert_eq!(
            *rest,
            Arg { key: rest.key, name: "rest", required: false, variadic: true, ..Arg::REQUIRED },
        );
    }

    #[test]
    fn flatten_field_docs_are_projected_as_help_groups() {
        let command = GroupedModel::COMMAND;
        let &[group] = command.help_groups else {
            panic!("expected one documented help group");
        };
        let &[json, field, ..] = command.flags else {
            panic!("expected json and field flags");
        };

        assert_eq!(group.heading, "Output");
        assert!(group.args.is_empty());
        let &[group_json, group_field] = group.flags else {
            panic!("expected json and field flags in the help group");
        };
        assert!(std::ptr::eq(group_json, json));
        assert!(std::ptr::eq(group_field, field));
        assert!(!json.repeatable);
        assert!(field.repeatable);
    }

    #[test]
    fn command_docs_are_projected_as_structured_help() {
        let command = DocumentedModel::COMMAND;
        assert_eq!(command.about, Some("Documented command summary."));
        assert_eq!(
            command.description,
            Some(
                "Documented command summary.\n\nAdditional command context remains available to full help."
            )
        );
        let [examples, machine_usage] = command.help_sections else {
            panic!("expected examples and machine-readable usage sections");
        };
        assert_eq!(examples.heading, "Examples");
        assert_eq!(examples.body, "example run");
        assert_eq!(machine_usage.heading, "Machine-readable usage");
        assert_eq!(machine_usage.body, "Use `example schema`.");
    }

    #[test]
    fn flattened_tables_are_composed_in_declaration_order_without_rekeying_children() {
        let command = Flattened::COMMAND;
        let nested = <Nested as argx::__private::CommandArgs>::COMMAND;
        let shared = <Shared as argx::__private::CommandArgs>::COMMAND;

        let &[before_flag, nested_flag, shared_flag, after_flag] = command.flags else {
            panic!("expected four composed flags");
        };
        assert_eq!(
            [before_flag.name, nested_flag.name, shared_flag.name, after_flag.name],
            ["before_flag", "nested", "shared", "after_flag"],
        );
        let &[nested_own_flag, ..] = nested.flags else {
            panic!("expected nested flag");
        };
        let &[shared_own_flag, ..] = shared.flags else {
            panic!("expected shared flag");
        };
        assert_eq!(nested_flag.key, nested_own_flag.key);
        assert_eq!(shared_flag.key, shared_own_flag.key);

        let &[before, middle, after] = command.args else {
            panic!("expected three composed positionals");
        };
        assert_eq!([before.name, middle.name, after.name], ["before", "middle", "after"]);
        let &[shared_middle, ..] = shared.args else {
            panic!("expected shared middle positional");
        };
        assert_eq!(middle.key, shared_middle.key);

        assert!(argx::__private::command_keys_unique(command.flags, command.args));
        assert!(argx::__private::flag_spellings_unique(command.flags));
        assert!(argx::__private::positional_layout_valid(command.args));
    }

    #[test]
    fn key_low_half_encodes_kind_and_local_index() {
        let command = Cli::COMMAND;
        let high = command.key & 0xffff_ffff_0000_0000;

        let &[verbose, output, ..] = command.flags else {
            panic!("expected verbose and output flags");
        };
        let &[input, rest, ..] = command.args else {
            panic!("expected input and rest positionals");
        };

        assert_eq!(command.key, high | 0x8000_0000);
        assert_eq!(verbose.key, high);
        assert_eq!(output.key, high | 1);
        assert_eq!(input.key, high | 0x4000_0000);
        assert_eq!(rest.key, high | 0x4000_0001);
    }

    #[test]
    fn identical_declarations_in_different_modules_have_different_keys() {
        let add = <add::Options as argx::__private::CommandArgs>::COMMAND;
        let remove = <remove::Options as argx::__private::CommandArgs>::COMMAND;
        let &[add_force, ..] = add.flags else {
            panic!("expected add force flag");
        };
        let &[remove_force, ..] = remove.flags else {
            panic!("expected remove force flag");
        };

        assert_ne!(add.key, remove.key);
        assert_ne!(add_force.key, remove_force.key);
        assert_eq!(add_force.key & 0xffff_ffff, remove_force.key & 0xffff_ffff);
    }

    #[test]
    fn subcommand_derive_exposes_static_nested_command_tables() {
        use argx::__private::Subcommands as _;

        let command = CommandCli::COMMAND;
        let &[add, remove, nested, status] = command.subcommands else {
            panic!("expected add, remove, nested, and status subcommands");
        };
        assert_eq!(
            [add.name, remove.name, nested.name, status.name],
            ["add", "remove", "nested", "status"],
        );
        assert_eq!(add.about, Some("Add one value."));
        assert_eq!(remove.about, Some("Remove one value"));

        let add_args = <AddArgs as argx::__private::CommandArgs>::COMMAND;
        assert_eq!(add.flags, add_args.flags);
        assert_eq!(add.args, add_args.args);
        let &[force, ..] = add.flags else {
            panic!("expected add force flag");
        };
        let &[value, ..] = add.args else {
            panic!("expected add value positional");
        };
        assert_eq!(force.name, "force");
        assert_eq!(value.name, "value");
        assert_eq!(remove.flags, add_args.flags);
        assert_eq!(remove.args, add_args.args);

        let &[leaf] = nested.subcommands else {
            panic!("expected nested leaf command");
        };
        assert_eq!(leaf.name, "leaf");
        assert_eq!(Commands::COMMANDS, command.subcommands);

        let keys = [add.key, remove.key, nested.key, status.key];
        assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
        for (index, key) in keys.into_iter().enumerate() {
            assert_eq!(key & 0xffff_ffff, 0x8000_0000 | index as u64);
        }
    }

    #[test]
    fn version_metadata_is_projected_into_private_command_actions() {
        use argx::__private::{ActionKind, Subcommands as _};

        let root = MetadataCli::COMMAND;
        let &[help, version] = root.actions else {
            panic!("expected help and version actions");
        };
        assert_eq!(help.name, "help");
        assert_eq!(version.name, "version");
        assert_eq!(version.kind, ActionKind::Version { short: VERSION, long: LONG_VERSION });

        let &[versioned, plain] = MetadataCommands::COMMANDS else {
            panic!("expected versioned and plain commands");
        };
        assert_eq!(versioned.actions.len(), 2);
        assert_eq!(plain.actions.len(), 1);
    }

    #[test]
    fn constraints_are_normalized_to_semantic_argument_keys() {
        use argx::__private::ConstraintKind;

        let command = ConstraintCli::COMMAND;
        let &[endpoint, token, stdout, ..] = command.flags else {
            panic!("expected endpoint, token, and stdout flags");
        };
        let &[requires, conflicts] = command.constraints else {
            panic!("expected requires and conflicts constraints");
        };

        assert_eq!(requires.kind, ConstraintKind::Requires);
        assert_eq!((requires.source, requires.target), (endpoint.key, token.key));
        assert_eq!(conflicts.kind, ConstraintKind::Conflicts);
        assert_eq!((conflicts.source, conflicts.target), (endpoint.key, stdout.key));
        assert_eq!(token.name, "token");
        assert_eq!(token.diagnostic, "--auth-token");
        assert_eq!(token.aliases, ["token"]);
    }

    #[test]
    fn identical_subcommand_declarations_in_different_modules_have_different_keys() {
        use argx::__private::Subcommands as _;

        let &[first, ..] = command_a::Action::COMMANDS else {
            panic!("expected first run command");
        };
        let &[second, ..] = command_b::Action::COMMANDS else {
            panic!("expected second run command");
        };
        assert_ne!(first.key, second.key);
        assert_eq!(first.key & 0xffff_ffff, second.key & 0xffff_ffff);
    }

    #[test]
    fn binding_uses_key_dispatch_with_exact_metadata_identity() {
        let command = Cli::COMMAND;
        let &[real_flag, ..] = command.flags else {
            panic!("expected command flag metadata");
        };
        let fake_flag = Flag { key: real_flag.key, ..Flag::BOOL };
        let mut command_partial = <Cli as argx::__private::CommandArgs>::start();
        assert!(!<Cli as argx::__private::CommandArgs>::apply(
            &mut command_partial,
            &Event::Flag { flag: &fake_flag, value: None },
        ));

        let &[real_arg, ..] = command.args else {
            panic!("expected command positional metadata");
        };
        let fake_arg = Arg { key: real_arg.key, ..Arg::REQUIRED };
        assert!(!<Cli as argx::__private::CommandArgs>::apply(
            &mut command_partial,
            &Event::Arg { arg: &fake_arg, value: b"value" },
        ));

        let &[real_command, ..] = <Commands as argx::__private::Subcommands>::COMMANDS else {
            panic!("expected subcommand metadata");
        };
        let fake_command = Command { key: real_command.key, ..Command::EMPTY };
        let mut subcommand_partial = <Commands as argx::__private::Subcommands>::start();
        assert!(!<Commands as argx::__private::Subcommands>::apply(
            &mut subcommand_partial,
            &Event::Command { command: &fake_command },
        ));
    }
}
