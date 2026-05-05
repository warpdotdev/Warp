use anyhow::Result;
use warp_cli::{vault::VaultCommand, GlobalOptions};
use warp_vault::{
    config::{ProviderConfig, ProviderType, SecretMapping, VaultConfig},
    fetch_secrets,
    provider::aws::AwsProvider,
};
use warpui::AppContext;

pub fn run(_ctx: &mut AppContext, _global_options: GlobalOptions, command: VaultCommand) -> Result<()> {
    match command {
        VaultCommand::Inject(args) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (mappings, provider_config) =
                    if let (Some(path), Some(env_var)) = (args.path, args.env_var) {
                        let provider_config = ProviderConfig {
                            provider_type: ProviderType::Aws,
                            region: None,
                        };
                        (vec![SecretMapping::new(path, env_var)?], provider_config)
                    } else {
                        let config = VaultConfig::load()?;
                        let provider_config = config.provider;
                        (config.mappings()?, provider_config)
                    };

                if mappings.is_empty() {
                    anyhow::bail!("vault: no mappings found — add entries to ~/.warp/vault.toml or pass both a path and --as flag");
                }

                let provider = match provider_config.provider_type {
                    ProviderType::Aws => AwsProvider::new(provider_config.region).await?,
                };

                let secrets = fetch_secrets(&provider, &mappings).await?;

                for secret in &secrets {
                    let escaped = secret.value().replace('\'', "'\\''");
                    eprintln!("✓ {} ready", secret.env_var());
                    println!("export {}='{}'", secret.env_var(), escaped);
                }

                Ok(())
            })
        }
    }
}
