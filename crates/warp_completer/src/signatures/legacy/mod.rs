use std::sync::{Arc, OnceLock};

use warp_command_signatures::{
    Argument, ArgumentType, CommandBuilder, CommandSignatureGenerators, Generator, GeneratorName,
    GeneratorResults, IsArgumentOptional, Signature, Suggestion as MetadataSuggestion,
};
use warp_core::channel::Channel;

pub mod registry;

pub use registry::CommandRegistry;

static GLOBAL_REGISTRY: OnceLock<Arc<CommandRegistry>> = OnceLock::new();

impl CommandRegistry {
    /// Returns a reference to a single global instance of the command registry.
    ///
    /// The registry is a read-only store of information used to provide smart
    /// suggestions and completions, and as such, only one instance is required
    /// across the application.  The registry itself can be quite large, so use
    /// of a single global instance avoids unnecessary memory allocations and
    /// usage.
    pub fn global_instance() -> Arc<Self> {
        GLOBAL_REGISTRY
            .get_or_init(|| {
                // TODO(wasm): Determine how to asynchronously load command signatures on wasm.
                Arc::new(CommandRegistry::new_with_embedded_signatures())
            })
            .clone()
    }

    /// Returns a new [`CommandRegistry`] that looks up commands in the embedded
    /// set of command signatures.
    fn new_with_embedded_signatures() -> Self {
        let mut dynamic_completion_data = warp_command_signatures::dynamic_command_signature_data();
        let (pkill_command_name, pkill_completion_data): (
            String,
            warp_command_signatures::DynamicCompletionData,
        ) = pkill_dynamic_completion_data().into();
        dynamic_completion_data.insert(pkill_command_name, pkill_completion_data);

        let registry = CommandRegistry::new(
            |command| {
                let start = instant::Instant::now();
                let signature = warp_command_signatures::signature_by_name(command);
                log::debug!(
                    "Lazily loaded command signature for {command} in {}s",
                    start.elapsed().as_secs_f32()
                );
                signature
            },
            dynamic_completion_data,
        );

        Self::register_warp_signatures(&registry);
        registry.register_signature(pkill_signature());

        registry
    }

    /// Register signatures for Warp CLI commands.
    ///
    /// Ideally this would be done outside of the `warp_completer` crate, but it's not currently
    /// possible to configure the shared [`Self::global_instance`].
    fn register_warp_signatures(registry: &Self) {
        // We use the current instance's signature for each channel. This is not entirely accurate - for example:
        // * The user might be SSHed into a host with a different version of the CLI
        // * The user might be using Preview, which will have different features than Stable.
        // However, it'll be close enough, and this approach ensures that we keep the CLI completions up to date.
        let channels = [Channel::Stable, Channel::Preview, Channel::Dev];

        for channel in channels {
            let bin_name = channel.cli_command_name();
            let mut clap_cmd = warp_cli::Args::clap_command();
            let signature =
                crate::signatures::clap::signature_from_clap_command(&mut clap_cmd, bin_name);
            registry.register_signature(signature);
        }
    }

    /// Returns an empty [`CommandRegistry`] that contains no signatures nor
    /// generators.
    pub fn empty() -> Self {
        CommandRegistry::new(|_| None, std::collections::HashMap::new())
    }

    /// Returns a [`CommandRegistry`] that uses the provided set of signatures
    /// and generators.  This does not utilize any data from the
    /// warp-command-signatures crate.
    #[cfg(feature = "test-util")]
    pub fn new_for_test(
        signatures: impl IntoIterator<Item = Signature>,
        generators: std::collections::HashMap<
            String,
            warp_command_signatures::DynamicCompletionData,
        >,
    ) -> Self {
        let registry = CommandRegistry::new(|_| None, generators);
        signatures
            .into_iter()
            .for_each(|signature| registry.register_signature(signature));
        registry
    }
}

fn pkill_signature() -> Signature {
    Signature {
        name: "pkill".to_string(),
        alias_generator: None,
        description: Some("signal processes by name".to_string()),
        arguments: Some(vec![Argument {
            display_name: Some("process_name".to_string()),
            description: Some("Process name or pattern".to_string()),
            is_variadic: false,
            is_command: false,
            argument_types: vec![ArgumentType::Generator(GeneratorName(
                "process_name".to_string(),
            ))],
            optional: IsArgumentOptional::Required,
            skip_generator_validation: false,
        }]),
        subcommands: None,
        options: None,
        priority: Default::default(),
        parser_directives: Default::default(),
    }
}

fn pkill_dynamic_completion_data() -> CommandSignatureGenerators {
    CommandSignatureGenerators::new("pkill").add_generator(
        "process_name",
        Generator::script(
            CommandBuilder::pipe(
                CommandBuilder::single_command("ps -A -o comm"),
                CommandBuilder::single_command("sort -u"),
            ),
            |output| GeneratorResults {
                suggestions: output
                    .trim()
                    .lines()
                    .filter_map(pkill_process_suggestion)
                    .collect(),
                is_ordered: false,
            },
        ),
    )
}

fn pkill_process_suggestion(path: &str) -> Option<MetadataSuggestion> {
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);

    if name.is_empty() || name == "COMM" {
        return None;
    }

    Some(MetadataSuggestion::with_description(name, path))
}

#[cfg(test)]
mod tests {
    use super::pkill_process_suggestion;

    #[test]
    fn pkill_process_suggestion_uses_basename() {
        let suggestion = pkill_process_suggestion("/usr/bin/login").unwrap();

        assert_eq!(suggestion.exact_string, "login");
        assert_eq!(suggestion.description.as_deref(), Some("/usr/bin/login"));
    }

    #[test]
    fn pkill_process_suggestion_allows_bare_process_names() {
        let suggestion = pkill_process_suggestion("zsh").unwrap();

        assert_eq!(suggestion.exact_string, "zsh");
        assert_eq!(suggestion.description.as_deref(), Some("zsh"));
    }

    #[test]
    fn pkill_process_suggestion_skips_ps_header() {
        assert!(pkill_process_suggestion("COMM").is_none());
    }
}

// We only implement Default for this in tests, as in production, we should
// always use the shared instance, but in tests, we might want to configure
// instances differently.
#[cfg(feature = "test-util")]
impl Default for CommandRegistry {
    fn default() -> Self {
        CommandRegistry::new_with_embedded_signatures()
    }
}
