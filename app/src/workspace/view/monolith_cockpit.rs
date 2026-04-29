use crate::{
    appearance::Appearance, root_view::SubshellCommandArg, settings::MonolithSettings,
    terminal::shell::ShellType, workspace::WorkspaceAction,
};
use serde::Deserialize;
use settings::Setting as _;
use std::{collections::HashSet, path::Path};
use warp_core::ui::Icon;
use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Element, Fill, Flex, Hoverable, MainAxisAlignment, MainAxisSize,
    MouseStateHandle, Padding, ParentElement, Radius, ScrollbarWidth, Shrinkable, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext};

const OPEN_SUBSHELL_ACTION: &str =
    "root_view:open_new_tab_insert_subshell_command_and_bootstrap_if_supported";
const STAGING_API_URL: &str = "https://raava-fleet-api-staging-lmbn6fkciq-ue.a.run.app";
const PROD_API_URL: &str = "https://api.fleetos.raavasolutions.com";
const GCP_PROJECT: &str = "raava-481318";
const STAGING_PROFILE_PATH: &str =
    "/Users/master/projects/warp-monolith/examples/monolith-cockpit-profile.live.json";
const PROD_PROFILE_PATH: &str =
    "/Users/master/projects/warp-monolith/examples/monolith-cockpit-profile.prod.json";

#[derive(Clone, Debug, Deserialize)]
struct RuntimeProfile {
    name: String,
    status: String,
    workdir: String,
    git_ref: String,
    #[serde(default)]
    service_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HostProfile {
    name: String,
    zone: String,
    status: String,
    #[serde(default)]
    project: Option<String>,
    runtimes: Vec<RuntimeProfile>,
}

#[derive(Clone, Debug, Deserialize)]
struct TenantProfile {
    name: String,
    environment: String,
    hosts: Vec<HostProfile>,
}

#[derive(Clone, Debug, Deserialize)]
struct CockpitProfile {
    tenants: Vec<TenantProfile>,
}

#[derive(Clone, Debug)]
pub enum MonolithCockpitAction {
    OpenCommand { command: String },
    StartTenantChat { prompt: String },
    ShowTenantFilter { filter: TenantFilter },
    ExpandAllTenants { tenant_names: Vec<String> },
    CollapseAllTenants,
    ToggleTenant { tenant_name: String },
    SwitchEnvironment { environment: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TenantFilter {
    All,
    Active,
    Offboarded,
    WithVms,
}

pub struct MonolithCockpitView {
    button_mouse_states: Vec<MouseStateHandle>,
    tenant_mouse_states: Vec<MouseStateHandle>,
    environment_mouse_states: Vec<MouseStateHandle>,
    cloud_mouse_states: Vec<MouseStateHandle>,
    scroll_state: ClippedScrollStateHandle,
    expanded_tenants: HashSet<String>,
    tenant_filter: TenantFilter,
}

impl MonolithCockpitView {
    pub fn new(_: &mut ViewContext<Self>) -> Self {
        Self {
            button_mouse_states: (0..512).map(|_| MouseStateHandle::default()).collect(),
            tenant_mouse_states: (0..128).map(|_| MouseStateHandle::default()).collect(),
            environment_mouse_states: (0..2).map(|_| MouseStateHandle::default()).collect(),
            cloud_mouse_states: (0..8).map(|_| MouseStateHandle::default()).collect(),
            scroll_state: ClippedScrollStateHandle::default(),
            expanded_tenants: HashSet::new(),
            tenant_filter: TenantFilter::All,
        }
    }

    fn default_profile() -> CockpitProfile {
        CockpitProfile {
            tenants: Vec::new(),
        }
    }

    fn load_profile(app: &AppContext) -> (CockpitProfile, Option<String>) {
        let profile_path = MonolithSettings::as_ref(app).cockpit_profile_path.value();
        if profile_path.trim().is_empty() {
            return (Self::default_profile(), None);
        }

        match std::fs::read_to_string(Path::new(profile_path)) {
            Ok(contents) => match serde_json::from_str::<CockpitProfile>(&contents) {
                Ok(profile) => (profile, Some(format!("profile: {profile_path}"))),
                Err(error) => (
                    Self::default_profile(),
                    Some(format!("profile parse failed: {error}")),
                ),
            },
            Err(error) => (
                Self::default_profile(),
                Some(format!("profile read failed: {error}")),
            ),
        }
    }

    fn shell_escape(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    fn gcloud_ssh_prefix(host: &HostProfile) -> String {
        let mut command = format!("gcloud compute ssh {}", Self::shell_escape(&host.name));
        if !host.zone.trim().is_empty() {
            command.push_str(&format!(" --zone {}", Self::shell_escape(&host.zone)));
        }
        if let Some(project) = host
            .project
            .as_ref()
            .filter(|project| !project.trim().is_empty())
        {
            command.push_str(&format!(" --project {}", Self::shell_escape(project)));
        }
        command
    }

    fn remote_command(host: &HostProfile, command: &str) -> String {
        format!(
            "{} --command {}",
            Self::gcloud_ssh_prefix(host),
            Self::shell_escape(command)
        )
    }

    fn tenant_status_label(tenant: &TenantProfile) -> &'static str {
        if tenant.environment.contains("offboarded") {
            "offboarded"
        } else if tenant.environment.contains("active") {
            "active"
        } else {
            "unknown"
        }
    }

    fn tenant_environment_label(tenant: &TenantProfile) -> &'static str {
        if tenant.environment.contains("prod") {
            "prod"
        } else if tenant.environment.contains("staging") {
            "staging"
        } else {
            "env"
        }
    }

    fn runtime_count(tenant: &TenantProfile) -> usize {
        tenant
            .hosts
            .iter()
            .map(|host| host.runtimes.len())
            .sum::<usize>()
    }

    fn running_runtime_count(tenant: &TenantProfile) -> usize {
        tenant
            .hosts
            .iter()
            .flat_map(|host| &host.runtimes)
            .filter(|runtime| runtime.status.contains("running"))
            .count()
    }

    fn tenant_matches_filter(tenant: &TenantProfile, filter: TenantFilter) -> bool {
        match filter {
            TenantFilter::All => true,
            TenantFilter::Active => Self::tenant_status_label(tenant) == "active",
            TenantFilter::Offboarded => Self::tenant_status_label(tenant) == "offboarded",
            TenantFilter::WithVms => !tenant.hosts.is_empty(),
        }
    }

    fn tenant_filter_label(filter: TenantFilter) -> &'static str {
        match filter {
            TenantFilter::All => "all",
            TenantFilter::Active => "active",
            TenantFilter::Offboarded => "offboarded",
            TenantFilter::WithVms => "with vms",
        }
    }

    fn cockpit_summary(profile: &CockpitProfile) -> (usize, usize, usize, usize, usize) {
        let tenants = profile.tenants.len();
        let active = profile
            .tenants
            .iter()
            .filter(|tenant| tenant.environment.contains("active"))
            .count();
        let offboarded = profile
            .tenants
            .iter()
            .filter(|tenant| tenant.environment.contains("offboarded"))
            .count();
        let vms = profile
            .tenants
            .iter()
            .map(|tenant| tenant.hosts.len())
            .sum::<usize>();
        let runtimes = profile
            .tenants
            .iter()
            .map(Self::runtime_count)
            .sum::<usize>();

        (tenants, active, offboarded, vms, runtimes)
    }

    fn tenant_chat_prompt(
        tenant: &TenantProfile,
        active_environment: &str,
        api_url: &str,
    ) -> String {
        let host_lines = if tenant.hosts.is_empty() {
            "- no VMs listed in the current cockpit profile".to_string()
        } else {
            tenant
                .hosts
                .iter()
                .map(|host| {
                    let runtime_names = if host.runtimes.is_empty() {
                        "no runtimes".to_string()
                    } else {
                        host.runtimes
                            .iter()
                            .map(|runtime| {
                                format!("{}:{}:{}", runtime.name, runtime.status, runtime.workdir)
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    format!(
                        "- {} zone={} status={} runtimes=[{}]",
                        host.name, host.zone, host.status, runtime_names
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "/agent You are managing one Monolith tenant from the Warp cockpit.\n\
Tenant: {}\n\
Tenant environment/status: {}\n\
Active cockpit environment: {}\n\
Fleet API: {}\n\
GCP project: {}\n\
VMs and runtimes:\n{}\n\n\
Operate only within this tenant by default. Start read-only: summarize health, risk, and the safest next actions. \
Before any write, show the exact command, target tenant, target VM/runtime, environment, and ask for explicit confirmation. \
Production writes require explicit elevated workflow confirmation.",
            tenant.name, tenant.environment, active_environment, api_url, GCP_PROJECT, host_lines
        )
    }

    fn runtime_service_name(runtime: &RuntimeProfile) -> String {
        runtime
            .service_name
            .clone()
            .unwrap_or_else(|| format!("monolith-agent-{}", runtime.name))
    }

    fn section_label(label: &str, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        Text::new(label.to_string(), appearance.ui_font_family(), 11.)
            .with_color(theme.disabled_ui_text_color().into_solid())
            .with_style(Properties::default().weight(Weight::Semibold))
            .finish()
    }

    fn value_line(label: &str, value: &str, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(6.)
            .with_child(
                Text::new(label.to_string(), appearance.ui_font_family(), 12.)
                    .with_color(theme.disabled_ui_text_color().into_solid())
                    .finish(),
            )
            .with_child(
                Text::new(value.to_string(), appearance.ui_font_family(), 12.)
                    .with_color(theme.main_text_color(theme.background()).into_solid())
                    .finish(),
            )
            .finish()
    }

    fn next_mouse_state(
        mouse_states: &[MouseStateHandle],
        button_index: &mut usize,
    ) -> MouseStateHandle {
        let index = *button_index;
        *button_index += 1;
        mouse_states.get(index).cloned().unwrap_or_default()
    }

    fn action_button(
        label: &str,
        command: String,
        mouse_state: MouseStateHandle,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let button = Container::new(
            Text::new(label.to_string(), appearance.ui_font_family(), 11.)
                .with_color(theme.active_ui_text_color().into_solid())
                .finish(),
        )
        .with_padding(Padding::uniform(4.).with_left(8.).with_right(8.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish();

        Hoverable::new(mouse_state, |_| button)
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(MonolithCockpitAction::OpenCommand {
                    command: command.clone(),
                });
            })
            .finish()
    }

    fn typed_button(
        label: &str,
        action: MonolithCockpitAction,
        mouse_state: MouseStateHandle,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let button = Container::new(
            Text::new(label.to_string(), appearance.ui_font_family(), 11.)
                .with_color(theme.active_ui_text_color().into_solid())
                .finish(),
        )
        .with_padding(Padding::uniform(4.).with_left(8.).with_right(8.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish();

        Hoverable::new(mouse_state, |_| button)
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
            })
            .finish()
    }

    fn tenant_filter_button(
        label: &str,
        filter: TenantFilter,
        is_active: bool,
        mouse_state: MouseStateHandle,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut button = Container::new(
            Text::new(label.to_string(), appearance.ui_font_family(), 11.)
                .with_color(theme.active_ui_text_color().into_solid())
                .finish(),
        )
        .with_padding(Padding::uniform(4.).with_left(8.).with_right(8.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));

        if is_active {
            button = button.with_background(theme.surface_3());
        }

        Hoverable::new(mouse_state, |_| button.finish())
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(MonolithCockpitAction::ShowTenantFilter { filter });
            })
            .finish()
    }

    fn status_chip(label: &str, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        Container::new(
            Text::new(label.to_string(), appearance.ui_font_family(), 10.)
                .with_color(theme.nonactive_ui_text_color().into_solid())
                .finish(),
        )
        .with_padding(Padding::uniform(3.).with_left(6.).with_right(6.))
        .with_background(theme.surface_2())
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
    }

    fn environment_button(
        label: &str,
        environment: &str,
        is_active: bool,
        mouse_state: MouseStateHandle,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let environment = environment.to_string();

        let mut button = Container::new(
            Text::new(label.to_string(), appearance.ui_font_family(), 11.)
                .with_color(theme.active_ui_text_color().into_solid())
                .with_style(Properties::default().weight(Weight::Semibold))
                .finish(),
        )
        .with_padding(Padding::uniform(5.).with_left(9.).with_right(9.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));

        if is_active {
            button = button.with_background(theme.surface_3());
        }

        Hoverable::new(mouse_state, |_| button.finish())
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(MonolithCockpitAction::SwitchEnvironment {
                    environment: environment.clone(),
                });
            })
            .finish()
    }

    fn render_environment_switcher(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let settings = MonolithSettings::as_ref(app);
        let active_environment = settings.cockpit_environment.value();
        let api_url = settings.api_url.value();
        let is_prod = active_environment == "prod";

        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(7.)
            .with_child(
                Flex::row()
                    .with_spacing(6.)
                    .with_child(Self::environment_button(
                        "staging",
                        "staging",
                        !is_prod,
                        self.environment_mouse_states
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                        app,
                    ))
                    .with_child(Self::environment_button(
                        "prod",
                        "prod",
                        is_prod,
                        self.environment_mouse_states
                            .get(1)
                            .cloned()
                            .unwrap_or_default(),
                        app,
                    ))
                    .finish(),
            )
            .with_child(
                Text::new(api_url.clone(), appearance.ui_font_family(), 10.)
                    .with_color(
                        Appearance::as_ref(app)
                            .theme()
                            .disabled_ui_text_color()
                            .into_solid(),
                    )
                    .finish(),
            )
            .finish()
    }

    fn render_cloud_toolbar(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let setup_command = format!(
            "gcloud auth login && printf '\\nMonolith cockpit passes --project {} explicitly; it does not mutate global gcloud project or ADC credentials.\\n'",
            Self::shell_escape(GCP_PROJECT),
        );
        let status_command = format!(
            "printf 'account: '; gcloud auth list --filter=status:ACTIVE --format='value(account)'; printf 'project: '; gcloud config get-value project; gcloud compute instances list --project {} --filter={} --format='table(name,zone.basename(),status,labels.raava-tenant,labels.raava-agent)'",
            Self::shell_escape(GCP_PROJECT),
            Self::shell_escape("labels.raava-managed=true"),
        );
        let project_command = format!(
            "printf 'cockpit project: {}\\n'; printf 'global gcloud project: '; gcloud config get-value project",
            Self::shell_escape(GCP_PROJECT),
        );

        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(6.)
            .with_child(
                Text::new(
                    format!("cloud: gcloud / {}", GCP_PROJECT),
                    appearance.ui_font_family(),
                    10.,
                )
                .with_color(theme.disabled_ui_text_color().into_solid())
                .finish(),
            )
            .with_child(
                Flex::row()
                    .with_spacing(6.)
                    .with_child(Self::action_button(
                        "auth",
                        setup_command,
                        self.cloud_mouse_states.first().cloned().unwrap_or_default(),
                        app,
                    ))
                    .with_child(Self::action_button(
                        "status",
                        status_command,
                        self.cloud_mouse_states.get(1).cloned().unwrap_or_default(),
                        app,
                    ))
                    .with_child(Self::action_button(
                        "project",
                        project_command,
                        self.cloud_mouse_states.get(2).cloned().unwrap_or_default(),
                        app,
                    ))
                    .finish(),
            )
            .finish()
    }

    fn render_cockpit_summary(
        &self,
        profile: &CockpitProfile,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let settings = MonolithSettings::as_ref(app);
        let active_environment = settings.cockpit_environment.value();
        let is_prod = active_environment == "prod";
        let write_mode = if is_prod {
            "prod locked"
        } else {
            "staging guarded"
        };
        let (tenants, active, offboarded, vms, runtimes) = Self::cockpit_summary(profile);

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(8.)
                .with_child(Self::section_label("OPERATOR STATUS", app))
                .with_child(
                    Flex::row()
                        .with_spacing(6.)
                        .with_child(Self::status_chip(&format!("tenants {tenants}"), app))
                        .with_child(Self::status_chip(&format!("active {active}"), app))
                        .with_child(Self::status_chip(&format!("offboarded {offboarded}"), app))
                        .with_child(Self::status_chip(&format!("vms {vms}"), app))
                        .with_child(Self::status_chip(&format!("runtimes {runtimes}"), app))
                        .finish(),
                )
                .with_child(
                    Text::new(
                        format!("write mode: {write_mode}"),
                        appearance.ui_font_family(),
                        11.,
                    )
                    .with_color(theme.disabled_ui_text_color().into_solid())
                    .finish(),
                )
                .finish(),
        )
        .with_padding(Padding::uniform(10.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
        .finish()
    }

    fn runtime_card(
        tenant: &TenantProfile,
        host: &HostProfile,
        runtime: &RuntimeProfile,
        mouse_states: &[MouseStateHandle],
        button_index: &mut usize,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let service_name = Self::runtime_service_name(runtime);
        let prod_locked = tenant.environment.contains("prod");
        let inactive_target = tenant.environment.contains("offboarded")
            || host.status.contains("terminated")
            || host.status.contains("unknown");
        let mutation_guard = |action: &str, reason: &str| {
            let message = format!(
                "Monolith cockpit blocked {action} for {}/{}/{}: {reason}",
                tenant.name, host.name, runtime.name
            );
            format!("printf '%s\\n' {}", Self::shell_escape(&message))
        };

        let runtime_shell = Self::remote_command(
            host,
            &format!(
                "cd {} && exec ${{SHELL:-bash}} -l",
                Self::shell_escape(&runtime.workdir)
            ),
        );
        let git_status = Self::remote_command(
            host,
            &format!(
                "cd {} && git status --short --branch",
                Self::shell_escape(&runtime.workdir)
            ),
        );
        let logs = Self::remote_command(
            host,
            &format!(
                "cd {} && (test -d logs && tail -n 200 -f logs/*.log || journalctl --user -u {} -f)",
                Self::shell_escape(&runtime.workdir),
                Self::shell_escape(&service_name),
            ),
        );
        let deploy = if prod_locked {
            mutation_guard("deploy", "prod writes require explicit elevated workflow")
        } else if inactive_target {
            mutation_guard("deploy", "target is offboarded, terminated, or unknown")
        } else {
            Self::remote_command(
                host,
                &format!(
                    "cd {} && ./deploy.sh --tenant {} --runtime {}",
                    Self::shell_escape(&runtime.workdir),
                    Self::shell_escape(&tenant.name),
                    Self::shell_escape(&runtime.name),
                ),
            )
        };
        let start = if prod_locked {
            mutation_guard("start", "prod writes require explicit elevated workflow")
        } else if inactive_target {
            mutation_guard("start", "target is offboarded, terminated, or unknown")
        } else {
            Self::remote_command(
                host,
                &format!(
                    "systemctl --user start {}",
                    Self::shell_escape(&service_name)
                ),
            )
        };
        let pause = if prod_locked {
            mutation_guard("pause", "prod writes require explicit elevated workflow")
        } else if inactive_target {
            mutation_guard("pause", "target is offboarded, terminated, or unknown")
        } else {
            Self::remote_command(
                host,
                &format!(
                    "systemctl --user stop {}",
                    Self::shell_escape(&service_name)
                ),
            )
        };
        let restart = if prod_locked {
            mutation_guard("restart", "prod writes require explicit elevated workflow")
        } else if inactive_target {
            mutation_guard("restart", "target is offboarded, terminated, or unknown")
        } else {
            Self::remote_command(
                host,
                &format!(
                    "systemctl --user restart {}",
                    Self::shell_escape(&service_name)
                ),
            )
        };

        let header = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Flex::row()
                    .with_spacing(8.)
                    .with_child(
                        ConstrainedBox::new(
                            Icon::Dataflow
                                .to_warpui_icon(theme.sub_text_color(theme.background()))
                                .finish(),
                        )
                        .with_width(14.)
                        .with_height(14.)
                        .finish(),
                    )
                    .with_child(
                        Text::new(runtime.name.clone(), appearance.ui_font_family(), 13.)
                            .with_color(theme.active_ui_text_color().into_solid())
                            .with_style(Properties::default().weight(Weight::Semibold))
                            .finish(),
                    )
                    .finish(),
            )
            .with_child(
                Text::new(runtime.status.clone(), appearance.ui_font_family(), 11.)
                    .with_color(theme.disabled_ui_text_color().into_solid())
                    .finish(),
            )
            .finish();

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(8.)
                .with_child(header)
                .with_child(Self::value_line("workdir", &runtime.workdir, app))
                .with_child(Self::value_line("git", &runtime.git_ref, app))
                .with_child(Self::value_line("service", &service_name, app))
                .with_child(
                    Flex::row()
                        .with_spacing(6.)
                        .with_child(Self::action_button(
                            "shell",
                            runtime_shell,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .with_child(Self::action_button(
                            "git",
                            git_status,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .with_child(Self::action_button(
                            "logs",
                            logs,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_spacing(6.)
                        .with_child(Self::action_button(
                            "deploy",
                            deploy,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .with_child(Self::action_button(
                            "start",
                            start,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .with_child(Self::action_button(
                            "pause",
                            pause,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .with_child(Self::action_button(
                            "restart",
                            restart,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .finish(),
                )
                .finish(),
        )
        .with_padding(Padding::uniform(10.))
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
    }

    fn host_card(
        host: &HostProfile,
        tenant: &TenantProfile,
        mouse_states: &[MouseStateHandle],
        button_index: &mut usize,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let ssh_command = Self::gcloud_ssh_prefix(host);

        let mut runtimes = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(8.);
        for runtime in &host.runtimes {
            runtimes.add_child(Self::runtime_card(
                tenant,
                host,
                runtime,
                mouse_states,
                button_index,
                app,
            ));
        }

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(8.)
                .with_child(
                    Flex::row()
                        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(
                            Text::new(host.name.clone(), appearance.ui_font_family(), 13.)
                                .with_color(theme.active_ui_text_color().into_solid())
                                .with_style(Properties::default().weight(Weight::Semibold))
                                .finish(),
                        )
                        .with_child(Self::action_button(
                            "ssh",
                            ssh_command,
                            Self::next_mouse_state(mouse_states, button_index),
                            app,
                        ))
                        .finish(),
                )
                .with_child(Self::value_line("zone", &host.zone, app))
                .with_child(Self::value_line("status", &host.status, app))
                .with_child(Self::value_line(
                    "runtimes",
                    &host.runtimes.len().to_string(),
                    app,
                ))
                .with_child(runtimes.finish())
                .finish(),
        )
        .with_padding(Padding::uniform(12.))
        .with_background(theme.surface_2())
        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
    }

    fn tenant_card(
        tenant: &TenantProfile,
        is_expanded: bool,
        tenant_mouse_state: MouseStateHandle,
        mouse_states: &[MouseStateHandle],
        button_index: &mut usize,
        active_environment: &str,
        api_url: &str,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut hosts = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(10.);
        for host in &tenant.hosts {
            hosts.add_child(Self::host_card(
                host,
                tenant,
                mouse_states,
                button_index,
                app,
            ));
        }

        let chevron_icon = if is_expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };

        let tenant_name = tenant.name.clone();
        let prompt = Self::tenant_chat_prompt(tenant, active_environment, api_url);
        let chat_button = Self::typed_button(
            "chat",
            MonolithCockpitAction::StartTenantChat { prompt },
            Self::next_mouse_state(mouse_states, button_index),
            app,
        );

        let header = Hoverable::new(tenant_mouse_state, |_| {
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        Flex::row()
                            .with_spacing(6.)
                            .with_child(
                                Text::new(tenant.name.clone(), appearance.ui_font_family(), 14.)
                                    .with_color(theme.active_ui_text_color().into_solid())
                                    .with_style(Properties::default().weight(Weight::Bold))
                                    .finish(),
                            )
                            .with_child(Self::status_chip(
                                Self::tenant_environment_label(tenant),
                                app,
                            ))
                            .with_child(Self::status_chip(Self::tenant_status_label(tenant), app))
                            .finish(),
                    )
                    .with_child(
                        ConstrainedBox::new(
                            chevron_icon
                                .to_warpui_icon(theme.nonactive_ui_text_color())
                                .finish(),
                        )
                        .with_width(14.)
                        .with_height(14.)
                        .finish(),
                    )
                    .finish(),
            )
            .with_padding(Padding::uniform(2.))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(MonolithCockpitAction::ToggleTenant {
                tenant_name: tenant_name.clone(),
            });
        })
        .finish();

        let mut content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(10.)
            .with_child(header)
            .with_child(Self::value_line("environment", &tenant.environment, app))
            .with_child(Self::value_line(
                "vms",
                &tenant.hosts.len().to_string(),
                app,
            ))
            .with_child(Self::value_line(
                "runtimes",
                &format!(
                    "{} / {} running",
                    Self::running_runtime_count(tenant),
                    Self::runtime_count(tenant)
                ),
                app,
            ))
            .with_child(chat_button);

        if is_expanded {
            content.add_child(hosts.finish());
        }

        Container::new(content.finish())
            .with_padding(Padding::uniform(12.))
            .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .finish()
    }

    fn tenant_is_expanded(&self, tenant: &TenantProfile) -> bool {
        self.expanded_tenants.contains(&tenant.name)
    }
}

impl Entity for MonolithCockpitView {
    type Event = ();
}

impl TypedActionView for MonolithCockpitView {
    type Action = MonolithCockpitAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            MonolithCockpitAction::OpenCommand { command } => ctx.dispatch_global_action(
                OPEN_SUBSHELL_ACTION,
                SubshellCommandArg {
                    command: command.clone(),
                    shell_type: ShellType::from_name("bash"),
                },
            ),
            MonolithCockpitAction::StartTenantChat { prompt } => {
                ctx.dispatch_typed_action(&WorkspaceAction::InsertInInput {
                    content: prompt.clone(),
                    replace_buffer: true,
                    ensure_agent_mode: true,
                });
            }
            MonolithCockpitAction::ShowTenantFilter { filter } => {
                self.tenant_filter = *filter;
                ctx.notify();
            }
            MonolithCockpitAction::ExpandAllTenants { tenant_names } => {
                self.expanded_tenants = tenant_names.iter().cloned().collect();
                ctx.notify();
            }
            MonolithCockpitAction::CollapseAllTenants => {
                self.expanded_tenants.clear();
                ctx.notify();
            }
            MonolithCockpitAction::ToggleTenant { tenant_name } => {
                if !self.expanded_tenants.insert(tenant_name.clone()) {
                    self.expanded_tenants.remove(tenant_name);
                }
            }
            MonolithCockpitAction::SwitchEnvironment { environment } => {
                MonolithSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let (api_url, profile_path) = if environment == "prod" {
                        (PROD_API_URL, PROD_PROFILE_PATH)
                    } else {
                        (STAGING_API_URL, STAGING_PROFILE_PATH)
                    };
                    let _ = settings
                        .cockpit_environment
                        .set_value(environment.clone(), ctx);
                    let _ = settings.api_url.set_value(api_url.to_string(), ctx);
                    let _ = settings
                        .cockpit_profile_path
                        .set_value(profile_path.to_string(), ctx);
                });
            }
        }
    }
}

impl View for MonolithCockpitView {
    fn ui_name() -> &'static str {
        "MonolithCockpitView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let (profile, profile_status) = Self::load_profile(app);
        let settings = MonolithSettings::as_ref(app);
        let active_environment = settings.cockpit_environment.value().clone();
        let api_url = settings.api_url.value().clone();
        let filtered_tenants = profile
            .tenants
            .iter()
            .filter(|tenant| Self::tenant_matches_filter(tenant, self.tenant_filter))
            .collect::<Vec<_>>();

        let mut tenants = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(12.);
        let mut button_index = 0;
        for (tenant_index, tenant) in filtered_tenants.iter().enumerate() {
            let tenant_mouse_state = self
                .tenant_mouse_states
                .get(tenant_index)
                .cloned()
                .unwrap_or_default();
            tenants.add_child(Self::tenant_card(
                tenant,
                self.tenant_is_expanded(tenant),
                tenant_mouse_state,
                &self.button_mouse_states,
                &mut button_index,
                &active_environment,
                &api_url,
                app,
            ));
        }

        let mut body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(14.)
            .with_child(
                Text::new("Monolith", appearance.ui_font_family(), 18.)
                    .with_color(theme.active_ui_text_color().into_solid())
                    .with_style(Properties::default().weight(Weight::Bold))
                    .finish(),
            )
            .with_child(self.render_environment_switcher(app))
            .with_child(self.render_cloud_toolbar(app))
            .with_child(self.render_cockpit_summary(&profile, app))
            .with_child(Self::section_label("TENANT > VM > AGENT RUNTIME", app))
            .with_child(
                Text::new(
                    "Select a tenant runtime, open VM shells through gcloud, inspect logs and Git, then run guarded lifecycle commands in Warp.",
                    appearance.ui_font_family(),
                    12.,
                )
                .with_color(theme.nonactive_ui_text_color().into_solid())
                .finish(),
            );

        if let Some(status) = profile_status {
            body.add_child(
                Text::new(status, appearance.ui_font_family(), 11.)
                    .with_color(theme.disabled_ui_text_color().into_solid())
                    .finish(),
            );
        }

        let tenant_names = filtered_tenants
            .iter()
            .map(|tenant| tenant.name.clone())
            .collect::<Vec<_>>();
        let mut header_button_index = 0;
        body.add_child(
            Flex::row()
                .with_spacing(6.)
                .with_child(Self::tenant_filter_button(
                    "all",
                    TenantFilter::All,
                    self.tenant_filter == TenantFilter::All,
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .with_child(Self::tenant_filter_button(
                    "active",
                    TenantFilter::Active,
                    self.tenant_filter == TenantFilter::Active,
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .with_child(Self::tenant_filter_button(
                    "offboarded",
                    TenantFilter::Offboarded,
                    self.tenant_filter == TenantFilter::Offboarded,
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .with_child(Self::tenant_filter_button(
                    "with vms",
                    TenantFilter::WithVms,
                    self.tenant_filter == TenantFilter::WithVms,
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .with_child(Self::typed_button(
                    "expand all",
                    MonolithCockpitAction::ExpandAllTenants { tenant_names },
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .with_child(Self::typed_button(
                    "collapse all",
                    MonolithCockpitAction::CollapseAllTenants,
                    Self::next_mouse_state(&self.cloud_mouse_states, &mut header_button_index),
                    app,
                ))
                .finish(),
        );

        if profile.tenants.is_empty() {
            body.add_child(
                Text::new(
                    "No live Monolith cockpit profile is configured.".to_string(),
                    appearance.ui_font_family(),
                    12.,
                )
                .with_color(theme.disabled_ui_text_color().into_solid())
                .finish(),
            );
        } else if filtered_tenants.is_empty() {
            body.add_child(
                Text::new(
                    format!(
                        "No tenants match filter: {}",
                        Self::tenant_filter_label(self.tenant_filter)
                    ),
                    appearance.ui_font_family(),
                    12.,
                )
                .with_color(theme.disabled_ui_text_color().into_solid())
                .finish(),
            );
        }

        body.add_child(tenants.finish());

        let scrollable = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            Container::new(body.finish())
                .with_padding(Padding::uniform(12.))
                .finish(),
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into(),
            theme.active_ui_detail().into(),
            Fill::None,
        )
        .with_overlayed_scrollbar()
        .finish();

        Shrinkable::new(
            1.0,
            Container::new(scrollable)
                .with_padding(Padding::uniform(0.))
                .finish(),
        )
        .finish()
    }
}
