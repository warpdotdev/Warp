//! Data types and pure logic for user-configured custom LSP servers
//! (`[[editor.language_servers]]` entries in settings.toml).

use std::collections::BTreeMap;

use serde_json::Value;

pub mod matcher;
pub mod parse;
pub mod placeholder;
pub mod validate;

/// One user-defined custom LSP server entry, parsed and validated from a
/// `[[editor.language_servers]]` table in settings.toml.
#[derive(Debug, Clone)]
pub struct LspServerDescriptor {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub filetypes: Vec<LspFiletypePattern>,
    pub root_markers: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Verbatim payload for the LSP `initialize` request's
    /// `initializationOptions` field. Strings inside undergo placeholder
    /// substitution at launch time; other values pass through unchanged.
    pub initialization_options: Option<Value>,
    /// Verbatim payload delivered via `workspace/didChangeConfiguration` after
    /// `initialized`, and returned for any `workspace/configuration` request
    /// the server makes. Subject to the same placeholder substitution as
    /// `initialization_options`.
    pub workspace_config: Option<Value>,
}

/// One element of `LspServerDescriptor::filetypes`. Parsed from either a bare
/// string pattern (TOML: `"*.rb"`) or an inline table (TOML: `{ pattern =
/// "*.rb", language_id = "ruby" }`); both forms produce the same struct.
///
/// Patterns containing glob metacharacters (`*`, `?`, `[`) match
/// case-insensitively; literal basenames match case-sensitively.
#[derive(Debug, Clone)]
pub struct LspFiletypePattern {
    /// The pattern string as written by the user (e.g. `"*.rb"`, `"Gemfile"`).
    pub pattern: String,
    /// When `Some`, this is the LSP `languageId` sent for files matched by
    /// this pattern. When `None`, the matcher derives a default from the
    /// matched file's extension (lowercased) or basename.
    pub language_id: Option<String>,
    /// Compiled matcher. Private so the choice of glob library is an
    /// implementation detail; callers test matches via [`is_match`].
    matcher: globset::GlobMatcher,
}

impl LspFiletypePattern {
    /// Constructs a pattern from its already-compiled parts.
    pub(crate) fn from_parts(
        pattern: String,
        language_id: Option<String>,
        matcher: globset::GlobMatcher,
    ) -> Self {
        Self {
            pattern,
            language_id,
            matcher,
        }
    }

    /// Returns `true` if this pattern matches `basename`. Glob patterns
    /// match case-insensitively, literal basenames case-sensitively; that
    /// distinction is baked into the compiled matcher at parse time.
    pub fn is_match(&self, basename: &str) -> bool {
        self.matcher.is_match(basename)
    }
}

