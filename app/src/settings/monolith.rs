use settings::{macros::define_settings_group, SupportedPlatforms, SyncToCloud};

define_settings_group!(MonolithSettings, settings: [
    api_url: MonolithApiUrl {
        type: String,
        default: "https://api.monolith.raava.ai".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "monolith.api.url",
        description: "Base URL for the Monolith Fleet API used by the cockpit and Monolith MCP server.",
    },
    default_tenant_id: MonolithDefaultTenantId {
        type: String,
        default: "".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "monolith.cockpit.default_tenant_id",
        description: "Optional default tenant selected by the Monolith cockpit.",
    },
    cockpit_environment: MonolithCockpitEnvironment {
        type: String,
        default: "staging".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "monolith.cockpit.environment",
        description: "Active Monolith cockpit environment profile: staging or prod.",
    },
    cockpit_profile_path: MonolithCockpitProfilePath {
        type: String,
        default: "".to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "monolith.cockpit.profile_path",
        description: "Optional local JSON profile for tenant, VM, and runtime inventory used by the Monolith cockpit MVP.",
    },
]);
