mod bundle;
mod locale;

pub use bundle::init_bundle;
pub use locale::Locale;

use fluent_bundle::{FluentArgs, FluentValue};
use std::borrow::Cow;

/// Look up a message by its FTL key.
///
/// Returns the translated string, or the key itself as a fallback (so missing
/// keys are visible during development rather than silently failing).
pub fn translate(msg_id: &str) -> Cow<'static, str> {
    let bundle = bundle::bundle();
    match bundle.get_message(msg_id) {
        Some(msg) => match msg.value() {
            Some(pattern) => {
                let mut errors = vec![];
                bundle.format_pattern(pattern, None, &mut errors).into()
            }
            None => Cow::Owned(msg_id.to_owned()),
        },
        None => Cow::Owned(msg_id.to_owned()),
    }
}

/// Look up a message with named arguments (e.g. `{ $name }` in the FTL value).
pub fn translate_with_args(msg_id: &str, args: &[(&str, FluentValue<'_>)]) -> Cow<'static, str> {
    let bundle = bundle::bundle();
    match bundle.get_message(msg_id) {
        Some(msg) => match msg.value() {
            Some(pattern) => {
                let mut errors = vec![];
                let mut fluent_args = FluentArgs::new();
                for &(k, ref v) in args {
                    fluent_args.set(k, v.clone());
                }
                bundle
                    .format_pattern(pattern, Some(&fluent_args), &mut errors)
                    .into()
            }
            None => Cow::Owned(msg_id.to_owned()),
        },
        None => Cow::Owned(msg_id.to_owned()),
    }
}

/// Translate a static message key.
#[macro_export]
macro_rules! tr {
    ($key:literal) => {
        warp_i18n::translate($key)
    };
}

/// Translate a message key with arguments.
///
/// # Example
///
/// ```ignore
/// tr_f!("greeting", ("name", "World".into()))
/// ```
#[macro_export]
macro_rules! tr_f {
    ($key:literal, $(($k:expr, $v:expr)),+ $(,)?) => {
        warp_i18n::translate_with_args($key, &[$(( $k, $v )),+])
    };
}
