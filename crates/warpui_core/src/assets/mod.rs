use anyhow::{anyhow, Result};
use std::borrow::Cow;
pub mod asset_cache;

impl AssetProvider for () {
    fn get(&self, path: &str) -> Result<Cow<'_, [u8]>> {
        Err(anyhow!(
            "get called on empty asset provider with \"{}\"",
            path
        ))
    }
}

pub trait AssetProvider: 'static {
    fn get(&self, path: &str) -> Result<Cow<'_, [u8]>>;
}
