use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::FluentResource;
use std::sync::OnceLock;

use crate::locale::Locale;

static BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();

fn load_resource(locale: Locale) -> FluentResource {
    let ftl = match locale {
        Locale::EnUs => include_str!("../locales/en-US.ftl"),
        Locale::ZhCn => include_str!("../locales/zh-CN.ftl"),
    };
    FluentResource::try_new(ftl.to_owned()).expect("failed to parse FTL resource")
}

/// Initialise (or re-initialise) the global Fluent bundle for the given locale.
/// Must be called at least once before `bundle()` or `translate()`.
pub fn init_bundle(locale: Locale) {
    let mut bundle = FluentBundle::new_concurrent(vec![locale.fluent_tag().parse().unwrap()]);
    bundle
        .add_resource(load_resource(locale))
        .expect("failed to add Fluent resource");
    let _ = BUNDLE.set(bundle);
}

/// Borrow the global bundle (panics if not yet initialised).
pub fn bundle() -> &'static FluentBundle<FluentResource> {
    BUNDLE
        .get()
        .expect("warp_i18n bundle not initialised — call init_bundle() first")
}
