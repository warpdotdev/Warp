use anyhow::{anyhow, Result};

pub fn run() -> Result<()> {
    Err(anyhow!("Plugin host unsupported on WASM"))
}
