use anyhow::Result;
use warp_cli::{vault::VaultCommand, GlobalOptions};
use warp_vault::{
    config::{ProviderType, VaultConfig},
    fetch_secrets,
    provider::aws::AwsProvider,
};
use warpui::AppContext;

pub fn run(_ctx: &mut AppContext, _global_options: GlobalOptions, command: VaultCommand) -> Result<()> {
    match command {
        VaultCommand::Inject(args) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let config = VaultConfig::load()?;

                let mappings = if let (Some(path), Some(env_var)) = (args.path, args.env_var) {
                    vec![warp_vault::config::SecretMapping { path, env_var }]
                } else {
                    config.mappings()
                };

                if mappings.is_empty() {
                    anyhow::bail!("vault: no mappings found — add entries to ~/.warp/vault.toml or pass both a path and --as flag");
                }

                let provider = match config.provider.provider_type {
                    ProviderType::Aws => AwsProvider::new(config.provider.region).await?,
                };

                let secrets = fetch_secrets(&provider, &mappings).await?;

                for secret in &secrets {
                    let escaped = secret.value().replace('\'', "'\\''");
                    println!("export {}='{}'", secret.env_var, escaped);
                }

                Ok(())
            })
        }
    }
}
