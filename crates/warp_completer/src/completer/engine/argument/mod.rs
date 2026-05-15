cfg_if::cfg_if! {
    if #[cfg(feature = "v2")] {
        mod v2;
        pub use v2::*;
    } else {
        mod legacy;
        pub use legacy::*;
    }
}

use crate::{
    meta::{Span, SpannedItem},
    parsers::{
        hir::{Expression, ShellCommand},
        ParsedExpression, ParsedToken,
    },
};

/// Creates a new empty positional arg in a shell_command. This is useful before evaluating args
/// so that we don't include the extra whitespace at the end of the command (e.g. "cd ") within the
/// top level command.
fn add_extra_positional(shell_command: &mut ShellCommand, cursor: &Span) {
    let mut positional =
        vec![ParsedExpression::new(Expression::Literal, ParsedToken::empty()).spanned(*cursor)];

    match shell_command.args.positionals.take() {
        Some(mut positionals) => {
            positionals.append(&mut positional);
            shell_command.args.positionals = Some(positionals);
        }
        None => shell_command.args.positionals = Some(positional),
    };
}

fn should_use_file_path_fallback(tokens_from_command: &[&str]) -> bool {
    !tokens_from_command
        .first()
        .is_some_and(|command| command.eq_ignore_ascii_case("pkill"))
}
