use rust_embed::RustEmbed;

#[derive(Clone, Copy, RustEmbed)]
#[folder = "js/build"]
pub struct CommandSignaturesJs;

pub static COMMAND_SIGNATURES_JS: CommandSignaturesJs = CommandSignaturesJs;
