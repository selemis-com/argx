//! Static command-model and generated-key contract tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use argx::__private::CommandArgs as _;

    #[derive(argx::Parser)]
    #[argx(name = "example")]
    struct Cli {
        #[argx(short, long)]
        verbose: bool,
        #[argx(long = "output")]
        output: Option<String>,
        input: String,
        rest: Vec<String>,
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

    #[test]
    fn parser_derive_emits_static_command_tables() {
        let value = Cli { verbose: false, output: None, input: String::new(), rest: Vec::new() };
        assert!(!value.verbose);
        assert!(value.output.is_none());
        assert!(value.input.is_empty());
        assert!(value.rest.is_empty());

        let command = Cli::COMMAND;
        assert_eq!(command.name, "example");
        assert_eq!(command.flags.len(), 2);
        assert_eq!(command.args.len(), 2);
        assert!(command.subcommands.is_empty());

        assert_eq!(command.flags[0].name, "verbose");
        assert_eq!(command.flags[0].longs, ["verbose"]);
        assert_eq!(command.flags[0].shorts, [b'v']);
        assert!(!command.flags[0].takes_value);
        assert!(!command.flags[0].allow_hyphen_values);
        assert!(!command.flags[0].allow_negative_numbers);

        assert_eq!(command.flags[1].name, "output");
        assert_eq!(command.flags[1].longs, ["output"]);
        assert!(command.flags[1].shorts.is_empty());
        assert!(command.flags[1].takes_value);
        assert!(!command.flags[1].allow_hyphen_values);
        assert!(!command.flags[1].allow_negative_numbers);

        assert_eq!(command.args[0].name, "input");
        assert!(command.args[0].required);
        assert!(!command.args[0].variadic);
        assert!(!command.args[0].allow_negative_numbers);

        assert_eq!(command.args[1].name, "rest");
        assert!(!command.args[1].required);
        assert!(command.args[1].variadic);
        assert!(!command.args[1].allow_negative_numbers);
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
}
