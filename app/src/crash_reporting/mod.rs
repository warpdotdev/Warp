#[cfg(linux_or_windows)]
mod local_minidump;

#[cfg(target_os = "linux")]
mod linux;

use std::borrow::Cow;

use lazy_static::lazy_static;
use warp_core::channel::Channel;
use warpui::{r#async::block_on, AppContext, SingletonEntity};

use crate::antivirus::{AntivirusInfo, AntivirusInfoEvent};
use crate::auth::{AuthStateProvider, UserUid};
use crate::channel::ChannelState;
use crate::features::FeatureFlag;
use crate::settings::{PrivacySettings, PrivacySettingsChangedEvent};
use parking_lot::RwLock;
use regex::Regex;
use std::collections::HashMap;
use warpui::rendering::GPUDeviceInfo;
use warpui::windowing::state::ApplicationStage;
use warpui::windowing::{self, StateEvent, WindowManager};

#[cfg(linux_or_windows)]
pub use local_minidump::run_server as run_minidump_server;

lazy_static! {
    /// A map from sensitive error messages to "scrubbed" error messages.
    static ref ERROR_MESSAGES_TO_SCRUB: Vec<(Regex, &'static str)> = vec![
         // The following are panic messages for invalid string slicing.
         // See here for source: https://cs.github.com/rust-lang/rust/blob/9c0bc3028a575eece6d4e8fbc6624cb95b9c9893/library/core/src/str/mod.rs?q=%22byte+index+%22+repo%3Arust-lang%2Frust#L100.
        (Regex::new(r"byte index .+ is out of bounds.+").unwrap(), "byte index is out of bounds"),
        (Regex::new(r"byte index .+ is not a char boundary.+").unwrap(), "byte index is not a char boundary"),
        (Regex::new(r"begin <= end .+ when slicing.+").unwrap(), "begin <= end when slicing"),
    ];

    /// The current [`ApplicationStage`] of the application. Stored as local crash metadata.
    static ref APPLICATION_LIFECYCLE_STAGE: RwLock<ApplicationStage> = RwLock::new(ApplicationStage::Starting);

    /// The set of tags that we want to attach to local crash logs and minidumps.
    static ref TAGS: RwLock<HashMap<String, String>> = Default::default();
}

/// Sets a kv-pair as local crash metadata for the rest of the application's lifetime.
pub(crate) fn set_tag<'a, 'b>(key: impl Into<Cow<'a, str>>, value: impl Into<Cow<'b, str>>) {
    set_tag_internal(key.into(), value.into());
}

/// Non-generic internal implementation of [`set_tag`].
fn set_tag_internal(key: Cow<'_, str>, value: Cow<'_, str>) {
    // Empty metadata values do not help local diagnostics.
    if value.is_empty() {
        return;
    }

    #[cfg(linux_or_windows)]
    local_minidump::set_tag(key.clone().into_owned(), value.clone().into_owned());

    TAGS.write().insert(key.into_owned(), value.into_owned());
}

/// Sets the [`GPUDeviceInfo`] of the last opened window for use in local crash logs.
pub(crate) fn set_gpu_device_info(gpu_device_info: GPUDeviceInfo) {
    for (key, value) in gpu_device_info.to_crash_report_tags() {
        set_tag(key, value);
    }
}

/// Sets the [`AntivirusInfo`] for use in local crash logs.
/// Only reports the first detected antivirus product.
pub fn set_antivirus_info(antivirus_info: &AntivirusInfo) {
    for (key, value) in antivirus_info.to_crash_report_tags() {
        set_tag(key, value);
    }
}

/// Sets the current application lifecycle stage for local crash diagnostics.
fn set_lifecycle_stage(stage: ApplicationStage) {
    #[cfg(linux_or_windows)]
    local_minidump::set_tags_from(&stage);

    *APPLICATION_LIFECYCLE_STAGE.write() = stage;
}

/// Sets the detected virtual environment info for local crash diagnostics.
fn set_virtual_environment(env: Option<VirtualEnvironment>) {
    for (key, value) in env.to_crash_report_tags() {
        set_tag(key, value);
    }
}

fn set_windowing_system(windowing_system: Option<windowing::System>) {
    for (key, value) in windowing_system.to_crash_report_tags() {
        set_tag(key, value);
    }
}

/// Checks if crash reporting is currently enabled.
fn is_crash_reporting_enabled(ctx: &mut AppContext) -> bool {
    PrivacySettings::handle(ctx)
        .as_ref(ctx)
        .is_crash_reporting_enabled
}
#[derive(Default)]
struct CrashRecoveryMetadata {
    /// Whether the previous crash was unhandled.
    was_unhandled_event: bool,
    /// Whether the crash recovery process is currently running, indicating that an unhandled event
    /// should actually be marked as handled.
    is_crash_recovery_process_running: bool,
}

impl CrashRecoveryMetadata {
    #[cfg(enable_crash_recovery)]
    fn new() -> Self {
        Self {
            was_unhandled_event: false,
            is_crash_recovery_process_running:
                crate::crash_recovery::is_crash_recovery_process_running(),
        }
    }

    #[cfg(not(enable_crash_recovery))]
    fn new() -> Self {
        Self {
            was_unhandled_event: false,
            is_crash_recovery_process_running: false,
        }
    }

    fn was_unhandled_event(&mut self) {
        self.was_unhandled_event = true;
    }
}

impl ToCrashReportTags for CrashRecoveryMetadata {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        #[cfg(enable_crash_recovery)]
        {
            fn bool_to_crash_metadata_value(value: bool) -> String {
                (if value { "yes" } else { "no" }).into()
            }

            [
                (
                    "warp.crash_recovery_process.running",
                    bool_to_crash_metadata_value(self.is_crash_recovery_process_running),
                ),
                (
                    "warp.handled_by_crash_recovery_process",
                    bool_to_crash_metadata_value(self.was_unhandled_event),
                ),
            ]
        }

        #[cfg(not(enable_crash_recovery))]
        std::iter::empty()
    }
}

/// Initializes the crash reporting subsystem.  Returns whether or not crash
/// reporting is active.
pub(crate) fn init(ctx: &mut AppContext) -> bool {
    if !FeatureFlag::CrashReporting.is_enabled() {
        log::info!(
            "Crash reporting FeatureFlag is disabled; not initializing local crash reporting."
        );
        return false;
    }

    let window_manager = WindowManager::handle(ctx);
    ctx.subscribe_to_model(&window_manager, |_, event, _| match event {
        StateEvent::ValueChanged { current, previous } => {
            if current.stage != previous.stage {
                set_lifecycle_stage(current.stage);
            }
        }
    });

    let antivirus_info = AntivirusInfo::handle(ctx);
    ctx.subscribe_to_model(&antivirus_info, |antivirus_info, event, ctx| match event {
        AntivirusInfoEvent::ScannedComplete => {
            let antivirus_info = antivirus_info.as_ref(ctx);
            set_antivirus_info(antivirus_info);
        }
    });

    let is_crash_reporting_enabled = is_crash_reporting_enabled(ctx);

    if is_crash_reporting_enabled {
        AuthStateProvider::handle(ctx).update(ctx, |auth_state_provider, ctx| {
            init_local_crash_reporting(
                auth_state_provider.get().user_id(),
                auth_state_provider.get().user_email(),
                ctx,
            );
        });
    } else {
        log::info!("Crash reporting setting is disabled; not initializing local crash reporting.");
    }

    set_windowing_system(ctx.windows().windowing_system());

    let privacy_settings = PrivacySettings::handle(ctx);
    ctx.subscribe_to_model(&privacy_settings, |_, event, ctx| {
        if let &PrivacySettingsChangedEvent::UpdateIsCrashReportingEnabled { new_value, .. } = event
        {
            if new_value {
                AuthStateProvider::handle(ctx).update(ctx, |auth_state_provider, ctx| {
                    init_local_crash_reporting(
                        auth_state_provider.get().user_id(),
                        auth_state_provider.get().user_email(),
                        ctx,
                    );
                });
            } else {
                uninit_sentry();
            }
        }
    });

    // Having initialized the SDK above, we can now set the initial value of
    // some tags.
    set_lifecycle_stage(window_manager.as_ref(ctx).stage());
    init_virtual_environment_tag(ctx);

    is_crash_reporting_enabled
}

/// Returns the local crash-reporting environment label.
fn get_environment() -> Cow<'static, str> {
    let channel = ChannelState::channel();

    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            let operating_system = "mac";
        } else if #[cfg(target_os = "linux")] {
            let operating_system = "linux";
        }
        else if #[cfg(target_os = "windows")] {
            let operating_system = "windows";
        } else {
           let operating_system = "";
        }
    };

    let base_environment_name = match channel {
        Channel::Stable => "stable_release",
        Channel::Preview => "preview_release",
        Channel::Local => "local",
        Channel::Integration => "integration_test",
        Channel::Dev => "dev_release",
        Channel::Oss => "oss_release",
    };

    if operating_system.is_empty() {
        base_environment_name.into()
    } else {
        format!("{operating_system}_{base_environment_name}").into()
    }
}

fn init_local_crash_reporting(
    _user_id: Option<UserUid>,
    _email: Option<String>,
    _ctx: &mut AppContext,
) {
    log::info!("openWarp: crash reporting 使用本地 panic 日志,不向远端上报");

    use std::sync::Once;
    static PANIC_HOOK_INSTALLED: Once = Once::new();
    PANIC_HOOK_INSTALLED.call_once(|| {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let location = panic_info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "<unknown location>".to_string());

            let payload = panic_info
                .payload()
                .downcast_ref::<&'static str>()
                .map(|s| s.to_string())
                .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());

            let thread = std::thread::current()
                .name()
                .unwrap_or("<unnamed>")
                .to_string();

            let backtrace = std::backtrace::Backtrace::force_capture();

            log::error!(
                "panic in thread '{thread}' at {location}: {payload}\nbacktrace:\n{backtrace}"
            );

            // 仍调原 hook 以便 stderr 等默认行为继续
            original_hook(panic_info);
        }));
    });
}

/// Stops local minidump crash reporting if it was started by this process.
pub fn uninit_sentry() {
    #[cfg(linux_or_windows)]
    local_minidump::uninit();
}

/// Compatibility no-op for call sites that temporarily disable crash reporting around process spawn.
pub fn init_cocoa_sentry() {
    log::info!("openWarp: macOS native crash reporter 已剥离,跳过初始化");
}

pub fn uninit_cocoa_sentry() {
    log::info!("openWarp: macOS native crash reporter 已剥离,跳过关闭");
}

pub fn crash() {
    panic!("openWarp: crash() invoked for local panic-hook smoke test");
}

/// 保留公开签名给登录状态回调使用,但本地 crash reporting 不写入用户身份。
pub fn set_user_id(_user_id: UserUid, _email: Option<String>, _ctx: &mut AppContext) {}

fn release_version() -> &'static str {
    ChannelState::app_version().unwrap_or("<no tag>")
}

/// Sets the warp.client_type local crash metadata.
pub fn set_client_type_tag(client_id: &str) {
    set_tag("warp.client_type", client_id);
}

/// Initializes the warp.virtual_env local crash metadata group.
fn init_virtual_environment_tag(ctx: &mut AppContext) {
    let (tx, rx) = async_channel::unbounded();

    // Compute the virtual environment in a background thread, as we don't want
    // to block application startup at all.
    std::thread::spawn(move || {
        let virt_env = VirtualEnvironment::detect();
        let _ = block_on(tx.send(virt_env));
    });
    ctx.foreground_executor()
        .spawn(async move {
            if let Ok(virt_env) = rx.recv().await {
                set_virtual_environment(virt_env);
            }
        })
        .detach();
}

/// Represents a virtualized environment that the operating system is running
/// under.
#[derive(Clone)]
struct VirtualEnvironment {
    name: String,
}

impl VirtualEnvironment {
    /// Detects the current virtual environment, if any.
    fn detect() -> Option<Self> {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                linux::get_virtualized_environment()
            } else {
                None
            }
        }
    }
}

trait ToCrashReportTags {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)>;
}

impl ToCrashReportTags for ApplicationStage {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        [("warp.application_stage", self.to_string())]
    }
}

impl ToCrashReportTags for GPUDeviceInfo {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        [
            ("warp.gpu.device.name", self.device_name.to_string()),
            ("warp.gpu.device.type", self.device_type.to_string()),
            ("warp.gpu.backend", self.backend.to_string()),
            ("warp.gpu.driver.name", self.driver_name.to_string()),
            ("warp.gpu.driver.info", self.driver_info.to_string()),
        ]
    }
}

impl ToCrashReportTags for Option<VirtualEnvironment> {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        let env = self.clone();
        [(
            "warp.virtual_env.name",
            env.map(|env| env.name).unwrap_or_else(|| "none".to_owned()),
        )]
    }
}

impl ToCrashReportTags for Option<windowing::System> {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        [(
            "warp.window.system",
            self.as_ref()
                .map(|windowing_system| windowing_system.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
        )]
    }
}

impl ToCrashReportTags for &AntivirusInfo {
    fn to_crash_report_tags(&self) -> impl IntoIterator<Item = (&str, String)> {
        [(
            "warp.window.antivirus.name",
            self.get().unwrap_or("none").into(),
        )]
    }
}
