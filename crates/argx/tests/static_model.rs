//! Static command-model and generated-key contract tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use argx::__private::{Arg, Command, CommandArgs as _, Event, Flag};

    /// Documentation that is overridden by explicit command help.
    #[derive(argx::Parser)]
    #[argx(name = "example", about = "Example command")]
    struct Cli {
        /// Enable verbose output.
        #[argx(short, long, global)]
        verbose: bool,
        /// Documentation overridden by explicit field help.
        #[argx(long = "output", env = "ARGX_OUTPUT", help = "Optional output path")]
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
        let value = Cli { verbose: false, output: None, input: String::new(), rest: Vec::new() };
        assert!(!value.verbose);
        assert!(value.output.is_none());
        assert!(value.input.is_empty());
        assert!(value.rest.is_empty());

        let command = Cli::COMMAND;
        assert_eq!(command.name, "example");
        assert_eq!(command.about, Some("Example command"));
        assert_eq!(command.flags.len(), 2);
        assert_eq!(command.args.len(), 2);
        assert!(command.subcommands.is_empty());

        assert_eq!(command.flags[0].name, "verbose");
        assert_eq!(command.flags[0].longs, ["verbose"]);
        assert_eq!(command.flags[0].shorts, [b'v']);
        assert_eq!(command.flags[0].help, Some("Enable verbose output."));
        assert!(command.flags[0].global);
        assert!(!command.flags[0].takes_value);
        assert!(!command.flags[0].required);
        assert!(!command.flags[0].allow_hyphen_values);
        assert!(!command.flags[0].allow_negative_numbers);

        assert_eq!(command.flags[1].name, "output");
        assert_eq!(command.flags[1].env, Some("ARGX_OUTPUT"));
        assert_eq!(command.flags[1].longs, ["output"]);
        assert!(command.flags[1].shorts.is_empty());
        assert_eq!(command.flags[1].help, Some("Optional output path"));
        assert!(!command.flags[1].global);
        assert!(command.flags[1].takes_value);
        assert!(!command.flags[1].required);
        assert!(!command.flags[1].allow_hyphen_values);
        assert!(!command.flags[1].allow_negative_numbers);

        assert_eq!(command.args[0].name, "input");
        assert_eq!(command.args[0].help, Some("Input value to process."));
        assert!(command.args[0].required);
        assert!(!command.args[0].variadic);
        assert!(!command.args[0].allow_negative_numbers);

        assert_eq!(command.args[1].name, "rest");
        assert!(!command.args[1].required);
        assert!(command.args[1].variadic);
        assert!(!command.args[1].allow_negative_numbers);
    }

    #[test]
    fn flattened_tables_are_composed_in_declaration_order_without_rekeying_children() {
        let value = Flattened {
            before_flag: false,
            before: String::new(),
            nested: Nested {
                nested: false,
                shared: Shared { shared: false, middle: String::new() },
            },
            after_flag: false,
            after: String::new(),
        };
        assert!(!value.before_flag);
        assert!(value.before.is_empty());
        assert!(!value.nested.nested);
        assert!(!value.nested.shared.shared);
        assert!(value.nested.shared.middle.is_empty());
        assert!(!value.after_flag);
        assert!(value.after.is_empty());

        let command = Flattened::COMMAND;
        let nested = <Nested as argx::__private::CommandArgs>::COMMAND;
        let shared = <Shared as argx::__private::CommandArgs>::COMMAND;

        assert_eq!(command.flags.len(), 4);
        assert_eq!(command.flags[0].name, "before_flag");
        assert_eq!(command.flags[1].name, "nested");
        assert_eq!(command.flags[2].name, "shared");
        assert_eq!(command.flags[3].name, "after_flag");
        assert_eq!(command.flags[1].key, nested.flags[0].key);
        assert_eq!(command.flags[2].key, shared.flags[0].key);

        assert_eq!(command.args.len(), 3);
        assert_eq!(command.args[0].name, "before");
        assert_eq!(command.args[1].name, "middle");
        assert_eq!(command.args[2].name, "after");
        assert_eq!(command.args[1].key, shared.args[0].key);

        assert!(argx::__private::command_keys_unique(command.flags, command.args));
        assert!(argx::__private::flag_spellings_unique(command.flags));
        assert!(argx::__private::positional_layout_valid(command.args));
    }

    #[test]
    fn key_low_half_encodes_kind_and_local_index() {
        let command = Cli::COMMAND;
        let high = command.key & 0xffff_ffff_0000_0000;

        assert_eq!(command.key, high | 0x8000_0000);
        assert_eq!(command.flags[0].key, high);
        assert_eq!(command.flags[1].key, high | 1);
        assert_eq!(command.args[0].key, high | 0x4000_0000);
        assert_eq!(command.args[1].key, high | 0x4000_0001);
    }

    #[test]
    fn identical_declarations_in_different_modules_have_different_keys() {
        let add_value = add::Options { force: false };
        let remove_value = remove::Options { force: false };
        assert!(!add_value.force);
        assert!(!remove_value.force);

        let add = <add::Options as argx::__private::CommandArgs>::COMMAND;
        let remove = <remove::Options as argx::__private::CommandArgs>::COMMAND;

        assert_ne!(add.key, remove.key);
        assert_ne!(add.flags[0].key, remove.flags[0].key);
        assert_eq!(add.flags[0].key & 0xffff_ffff, remove.flags[0].key & 0xffff_ffff);
    }

    #[test]
    fn subcommand_derive_exposes_static_nested_command_tables() {
        use argx::__private::Subcommands as _;

        let command = CommandCli::COMMAND;
        assert_eq!(command.subcommands.len(), 4);
        assert_eq!(command.subcommands[0].name, "add");
        assert_eq!(command.subcommands[0].about, Some("Add one value."));
        assert_eq!(command.subcommands[1].name, "remove");
        assert_eq!(command.subcommands[1].about, Some("Remove one value"));
        assert_eq!(command.subcommands[2].name, "nested");
        assert_eq!(command.subcommands[3].name, "status");

        let add = command.subcommands[0];
        let add_args = <AddArgs as argx::__private::CommandArgs>::COMMAND;
        assert_eq!(add.flags, add_args.flags);
        assert_eq!(add.args, add_args.args);
        assert_eq!(add.flags[0].name, "force");
        assert_eq!(add.args[0].name, "value");

        let remove = command.subcommands[1];
        assert_eq!(remove.flags, add_args.flags);
        assert_eq!(remove.args, add_args.args);

        let nested = command.subcommands[2];
        assert_eq!(nested.subcommands.len(), 1);
        assert_eq!(nested.subcommands[0].name, "leaf");
        assert_eq!(Commands::COMMANDS, command.subcommands);
        assert_ne!(command.subcommands[0].key, command.subcommands[1].key);
        assert_ne!(command.subcommands[1].key, command.subcommands[2].key);
        assert_ne!(command.subcommands[2].key, command.subcommands[3].key);
        for (index, command) in command.subcommands.iter().enumerate() {
            assert_eq!(command.key & 0xffff_ffff, 0x8000_0000 | index as u64);
        }

        let add_value = Commands::Add(AddArgs { force: false, value: String::new() });
        let Commands::Add(add_value) = add_value else {
            unreachable!("constructed Add variant changed")
        };
        assert!(!add_value.force);
        assert!(add_value.value.is_empty());

        let remove_value = Commands::Remove(AddArgs { force: false, value: String::new() });
        let Commands::Remove(remove_value) = remove_value else {
            unreachable!("constructed Remove variant changed")
        };
        assert!(!remove_value.force);
        assert!(remove_value.value.is_empty());

        let nested_value = Commands::Nested(NestedArgs { command: NestedCommand::Leaf });
        let Commands::Nested(nested_value) = nested_value else {
            unreachable!("constructed Nested variant changed")
        };
        assert!(matches!(nested_value.command, NestedCommand::Leaf));

        let value = CommandCli { verbose: false, command: Commands::Status };
        assert!(!value.verbose);
        assert!(matches!(value.command, Commands::Status));
    }

    #[test]
    fn version_metadata_is_projected_into_private_command_actions() {
        let value = MetadataCli { command: MetadataCommands::Plain };
        assert!(matches!(value.command, MetadataCommands::Plain));

        use argx::__private::{ActionKind, Subcommands as _};

        let root = MetadataCli::COMMAND;
        assert_eq!(root.actions.len(), 2);
        assert_eq!(root.actions[0].name, "help");
        assert_eq!(root.actions[1].name, "version");
        assert_eq!(
            root.actions[1].kind,
            ActionKind::Version { short: VERSION, long: LONG_VERSION }
        );

        let commands = MetadataCommands::COMMANDS;
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].actions.len(), 2);
        assert_eq!(commands[1].actions.len(), 1);
    }

    #[test]
    fn identical_subcommand_declarations_in_different_modules_have_different_keys() {
        use argx::__private::Subcommands as _;

        let first_value = command_a::Action::Run;
        let second_value = command_b::Action::Run;
        assert!(matches!(first_value, command_a::Action::Run));
        assert!(matches!(second_value, command_b::Action::Run));

        let first = command_a::Action::COMMANDS[0];
        let second = command_b::Action::COMMANDS[0];
        assert_ne!(first.key, second.key);
        assert_eq!(first.key & 0xffff_ffff, second.key & 0xffff_ffff);
    }

    #[test]
    fn binding_uses_exact_metadata_identity_instead_of_semantic_keys() {
        let command = Cli::COMMAND;
        let real_flag = command.flags[0];
        let fake_flag = Flag { key: real_flag.key, ..Flag::BOOL };
        let mut command_partial = <Cli as argx::__private::CommandArgs>::start();
        assert!(!<Cli as argx::__private::CommandArgs>::apply(
            &mut command_partial,
            &Event::Flag { flag: &fake_flag, value: None },
        ));

        let real_arg = command.args[0];
        let fake_arg = Arg { key: real_arg.key, ..Arg::REQUIRED };
        assert!(!<Cli as argx::__private::CommandArgs>::apply(
            &mut command_partial,
            &Event::Arg { arg: &fake_arg, value: b"value" },
        ));

        let real_command = <Commands as argx::__private::Subcommands>::COMMANDS[0];
        let fake_command = Command { key: real_command.key, ..Command::EMPTY };
        let mut subcommand_partial = <Commands as argx::__private::Subcommands>::start();
        assert!(!<Commands as argx::__private::Subcommands>::apply(
            &mut subcommand_partial,
            &Event::Command { command: &fake_command },
        ));
    }
}
