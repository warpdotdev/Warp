use std::{
    collections::BTreeMap,
    env, fmt, fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde::Deserialize;
use url::Url;

const DEFAULT_PORT: u16 = 4444;
const DEFAULT_UPSTREAM_NAME: &str = "default";
const DEFAULT_UPSTREAM_TIMEOUT_SECS: u64 = 180;
const DEFAULT_MODEL_IDS: [&str; 4] = [
    "auto",
    "cli-agent-auto",
    "computer-use-agent-auto",
    "coding-auto",
];

#[derive(Debug, Parser)]
#[command(
    name = "warp-shim-server",
    about = "Local Warp control-plane shim scaffold for OSS Warp AI flows"
)]
struct CliArgs {
    /// Path to a warp-shim TOML config file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Host/IP address to bind. Defaults to 127.0.0.1.
    #[arg(long, value_name = "HOST")]
    host: Option<IpAddr>,

    /// Port to bind. Defaults to 4444.
    #[arg(long, value_name = "PORT")]
    port: Option<u16>,

    /// OpenAI-compatible upstream base URL, for example http://127.0.0.1:11434/v1.
    #[arg(long, value_name = "URL")]
    upstream_url: Option<String>,

    /// API key to send to the upstream. Optional for local upstreams such as Ollama.
    #[arg(long, value_name = "KEY")]
    api_key: Option<String>,

    /// Environment variable containing the upstream API key.
    #[arg(long, value_name = "ENV")]
    api_key_env: Option<String>,

    /// Model mapping in the form warp_model=upstream_model. May be repeated.
    #[arg(long = "model", value_name = "WARP=UPSTREAM")]
    model: Vec<ModelMappingArg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMappingArg {
    warp_model: String,
    upstream_model: String,
}

impl FromStr for ModelMappingArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let (warp_model, upstream_model) = value
            .split_once('=')
            .ok_or_else(|| "model mappings must use warp_model=upstream_model".to_string())?;
        let warp_model = warp_model.trim();
        let upstream_model = upstream_model.trim();

        if warp_model.is_empty() || upstream_model.is_empty() {
            return Err("model mapping names must not be empty".to_string());
        }

        Ok(Self {
            warp_model: warp_model.to_string(),
            upstream_model: upstream_model.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ShimConfig {
    pub config_path: Option<PathBuf>,
    pub server: ServerConfig,
    pub upstreams: BTreeMap<String, UpstreamConfig>,
    pub models: BTreeMap<String, ModelMapping>,
    pub features: FeatureConfig,
}

impl ShimConfig {
    pub fn from_sources() -> Result<Self> {
        let cli = CliArgs::parse();
        let (toml, config_path) = read_toml_config(cli.config.as_deref())?;

        let host = cli
            .host
            .or(parse_env("WARP_SHIM_HOST")?)
            .or(toml.server.host)
            .unwrap_or_else(default_host);
        let port = cli
            .port
            .or(parse_env("WARP_SHIM_PORT")?)
            .or(toml.server.port)
            .unwrap_or(DEFAULT_PORT);
        let public_base_url = toml
            .server
            .public_base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}", SocketAddr::new(host, port)));

        let upstreams = merge_upstreams(&cli, &toml)?;
        let models = merge_models(&cli, &toml, &upstreams)?;
        let features = merge_features(&toml.features);

        Ok(Self {
            config_path,
            server: ServerConfig {
                host,
                port,
                public_base_url,
            },
            upstreams,
            models,
            features,
        })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new(self.server.host, self.server.port)
    }

    pub fn default_upstream(&self) -> Result<&UpstreamConfig> {
        self.upstreams
            .get(DEFAULT_UPSTREAM_NAME)
            .ok_or_else(|| anyhow!("validated config is missing the default upstream"))
    }

    pub fn model_mappings_for_log(&self) -> String {
        self.models
            .iter()
            .map(|(warp_model, mapping)| {
                format!("{warp_model}={}:{}", mapping.upstream, mapping.model)
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub public_base_url: String,
}

#[derive(Clone, Debug)]
pub struct UpstreamConfig {
    pub base_url: Url,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub timeout_secs: u64,
    pub streaming: bool,
}

#[derive(Clone, Debug)]
pub struct ModelMapping {
    pub upstream: String,
    pub model: String,
}

#[derive(Clone, Debug)]
pub struct FeatureConfig {
    pub tools_enabled: bool,
    pub mcp_tools_enabled: bool,
    pub passive_suggestions_enabled: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            tools_enabled: true,
            mcp_tools_enabled: true,
            passive_suggestions_enabled: true,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct TomlConfig {
    server: TomlServer,
    upstreams: BTreeMap<String, TomlUpstream>,
    models: BTreeMap<String, TomlModelMapping>,
    features: TomlFeatures,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct TomlServer {
    host: Option<IpAddr>,
    port: Option<u16>,
    public_base_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct TomlUpstream {
    base_url: Option<String>,
    api_key: Option<String>,
    api_key_env: Option<String>,
    timeout_secs: Option<u64>,
    streaming: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct TomlModelMapping {
    #[serde(default)]
    upstream: Option<String>,
    model: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct TomlFeatures {
    tools_enabled: Option<bool>,
    mcp_tools_enabled: Option<bool>,
    passive_suggestions_enabled: Option<bool>,
}

fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn read_toml_config(cli_config_path: Option<&Path>) -> Result<(TomlConfig, Option<PathBuf>)> {
    if let Some(path) = cli_config_path {
        return parse_toml_file(path).map(|config| (config, Some(path.to_path_buf())));
    }

    if let Some(path) = non_empty_env("WARP_SHIM_CONFIG").map(PathBuf::from) {
        return parse_toml_file(&path).map(|config| (config, Some(path)));
    }

    for path in implicit_config_paths() {
        if path.exists() {
            return parse_toml_file(&path).map(|config| (config, Some(path)));
        }
    }

    Ok((TomlConfig::default(), None))
}

fn implicit_config_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("warp-shim.toml")];
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".warp-shim").join("config.toml"));
    }
    paths
}

fn parse_toml_file(path: &Path) -> Result<TomlConfig> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read shim config {}", path.display()))?;
    toml::from_str(&contents)
        .with_context(|| format!("failed to parse shim config {}", path.display()))
}

fn merge_upstreams(cli: &CliArgs, toml: &TomlConfig) -> Result<BTreeMap<String, UpstreamConfig>> {
    let mut upstreams = BTreeMap::new();

    for (name, upstream) in &toml.upstreams {
        if name == DEFAULT_UPSTREAM_NAME {
            continue;
        }

        let base_url = upstream
            .base_url
            .clone()
            .ok_or_else(|| anyhow!("[upstreams.{name}].base_url is required"))?;
        upstreams.insert(
            name.clone(),
            build_upstream_config(name, base_url, upstream, None, None)?,
        );
    }

    let default_toml = toml
        .upstreams
        .get(DEFAULT_UPSTREAM_NAME)
        .cloned()
        .unwrap_or_default();
    let default_base_url = cli
        .upstream_url
        .clone()
        .or_else(|| non_empty_env("WARP_SHIM_UPSTREAM_URL"))
        .or_else(|| default_toml.base_url.clone())
        .ok_or_else(|| {
            anyhow!(
                "--upstream-url is required unless WARP_SHIM_UPSTREAM_URL or \
                 [upstreams.default].base_url is configured"
            )
        })?;
    let default_api_key_env = cli
        .api_key_env
        .clone()
        .or_else(|| non_empty_env("WARP_SHIM_API_KEY_ENV"))
        .or_else(|| default_toml.api_key_env.clone());
    let default_api_key = cli
        .api_key
        .clone()
        .or_else(|| non_empty_env("WARP_SHIM_API_KEY"))
        .or_else(|| default_toml.api_key.clone())
        .or_else(|| default_api_key_env.as_deref().and_then(non_empty_env));

    upstreams.insert(
        DEFAULT_UPSTREAM_NAME.to_string(),
        build_upstream_config(
            DEFAULT_UPSTREAM_NAME,
            default_base_url,
            &default_toml,
            Some(default_api_key),
            default_api_key_env,
        )?,
    );

    Ok(upstreams)
}

fn build_upstream_config(
    name: &str,
    base_url: String,
    upstream: &TomlUpstream,
    api_key_override: Option<Option<String>>,
    api_key_env_override: Option<String>,
) -> Result<UpstreamConfig> {
    let base_url = Url::parse(base_url.trim())
        .with_context(|| format!("invalid base URL for upstream `{name}`"))?;
    match base_url.scheme() {
        "http" | "https" => {}
        scheme => bail!("upstream `{name}` must use http or https, not `{scheme}`"),
    }

    let api_key_env = api_key_env_override.or_else(|| upstream.api_key_env.clone());
    let api_key = api_key_override.unwrap_or_else(|| {
        upstream
            .api_key
            .clone()
            .or_else(|| api_key_env.as_deref().and_then(non_empty_env))
    });

    Ok(UpstreamConfig {
        base_url,
        api_key,
        api_key_env,
        timeout_secs: upstream
            .timeout_secs
            .unwrap_or(DEFAULT_UPSTREAM_TIMEOUT_SECS),
        streaming: upstream.streaming.unwrap_or(true),
    })
}

fn merge_models(
    cli: &CliArgs,
    toml: &TomlConfig,
    upstreams: &BTreeMap<String, UpstreamConfig>,
) -> Result<BTreeMap<String, ModelMapping>> {
    let models = if !cli.model.is_empty() {
        model_args_to_map(&cli.model)
    } else if let Some(env_models) = parse_env_model_map()? {
        model_args_to_map(&env_models)
    } else if !toml.models.is_empty() {
        toml.models
            .iter()
            .map(|(warp_model, mapping)| {
                (
                    warp_model.clone(),
                    ModelMapping {
                        upstream: mapping
                            .upstream
                            .clone()
                            .unwrap_or_else(|| DEFAULT_UPSTREAM_NAME.to_string()),
                        model: mapping.model.clone(),
                    },
                )
            })
            .collect()
    } else {
        DEFAULT_MODEL_IDS
            .into_iter()
            .map(|warp_model| {
                (
                    warp_model.to_string(),
                    ModelMapping {
                        upstream: DEFAULT_UPSTREAM_NAME.to_string(),
                        model: "auto".to_string(),
                    },
                )
            })
            .collect()
    };

    for (warp_model, mapping) in &models {
        if !upstreams.contains_key(&mapping.upstream) {
            bail!(
                "model mapping `{warp_model}` references unknown upstream `{}`",
                mapping.upstream
            );
        }
    }

    Ok(models)
}

fn model_args_to_map(args: &[ModelMappingArg]) -> BTreeMap<String, ModelMapping> {
    args.iter()
        .map(|arg| {
            (
                arg.warp_model.clone(),
                ModelMapping {
                    upstream: DEFAULT_UPSTREAM_NAME.to_string(),
                    model: arg.upstream_model.clone(),
                },
            )
        })
        .collect()
}

fn parse_env_model_map() -> Result<Option<Vec<ModelMappingArg>>> {
    let Some(value) = non_empty_env("WARP_SHIM_MODEL_MAP") else {
        return Ok(None);
    };

    let mut mappings = Vec::new();
    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let mapping = entry
            .parse::<ModelMappingArg>()
            .map_err(|err| anyhow!("invalid WARP_SHIM_MODEL_MAP entry `{entry}`: {err}"))?;
        mappings.push(mapping);
    }

    if mappings.is_empty() {
        bail!("WARP_SHIM_MODEL_MAP did not contain any model mappings");
    }

    Ok(Some(mappings))
}

fn merge_features(toml: &TomlFeatures) -> FeatureConfig {
    let defaults = FeatureConfig::default();
    FeatureConfig {
        tools_enabled: toml.tools_enabled.unwrap_or(defaults.tools_enabled),
        mcp_tools_enabled: toml.mcp_tools_enabled.unwrap_or(defaults.mcp_tools_enabled),
        passive_suggestions_enabled: toml
            .passive_suggestions_enabled
            .unwrap_or(defaults.passive_suggestions_enabled),
    }
}

fn parse_env<T>(name: &str) -> Result<Option<T>>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let Some(value) = non_empty_env(name) else {
        return Ok(None);
    };

    value
        .parse::<T>()
        .map(Some)
        .map_err(|err| anyhow!("invalid {name}: {err}"))
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
