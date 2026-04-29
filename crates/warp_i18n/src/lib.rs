//! Lightweight application text localization for Warp.
//!
//! The crate deliberately keeps catalog lookup dependency-free so UI code can
//! adopt localized strings incrementally without adding runtime setup beyond
//! [`init_from_env`].

use std::borrow::Cow;
use std::env;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Locale {
    EnUs = 1,
    ZhCn = 2,
}

impl Locale {
    pub fn from_locale_identifier(identifier: &str) -> Option<Self> {
        let normalized = identifier
            .split(['.', '@'])
            .next()
            .unwrap_or(identifier)
            .replace('_', "-")
            .to_ascii_lowercase();

        match normalized.as_str() {
            "" | "c" | "posix" => None,
            "en" | "en-us" => Some(Self::EnUs),
            "zh" | "zh-cn" | "zh-hans" | "zh-hans-cn" | "zh-sg" | "zh-hans-sg" => {
                Some(Self::ZhCn)
            }
            _ => None,
        }
    }

    pub fn from_env() -> Option<Self> {
        for var in ["WARP_LOCALE", "LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
            let Ok(value) = env::var(var) else {
                continue;
            };

            for candidate in value.split(':') {
                if let Some(locale) = Self::from_locale_identifier(candidate) {
                    return Some(locale);
                }
            }
        }

        None
    }

    fn from_u8(value: u8) -> Self {
        match value {
            value if value == Self::ZhCn as u8 => Self::ZhCn,
            _ => Self::EnUs,
        }
    }
}

static ACTIVE_LOCALE: AtomicU8 = AtomicU8::new(Locale::EnUs as u8);

/// Initializes the active UI locale from environment variables.
///
/// `WARP_LOCALE` takes precedence, followed by common POSIX locale variables.
pub fn init_from_env() {
    if let Some(locale) = Locale::from_env() {
        set_locale(locale);
    }
}

pub fn set_locale(locale: Locale) {
    ACTIVE_LOCALE.store(locale as u8, Ordering::Relaxed);
}

pub fn current_locale() -> Locale {
    Locale::from_u8(ACTIVE_LOCALE.load(Ordering::Relaxed))
}

pub fn t(key: Key) -> Cow<'static, str> {
    match current_locale() {
        Locale::EnUs => Cow::Borrowed(english(key)),
        Locale::ZhCn => zh_cn(key)
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Borrowed(english(key))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    AgentManagementAllFilter,
    AgentManagementAllFilterTooltip,
    AgentManagementArtifactFile,
    AgentManagementArtifactPlan,
    AgentManagementArtifactPullRequest,
    AgentManagementArtifactScreenshot,
    AgentManagementClearAll,
    AgentManagementClearFilters,
    AgentManagementCloudAgent,
    AgentManagementCloudAgentDescription,
    AgentManagementCreatedBy,
    AgentManagementCreatedOn,
    AgentManagementDone,
    AgentManagementEnvironment,
    AgentManagementFailed,
    AgentManagementGetStarted,
    AgentManagementHasArtifact,
    AgentManagementHarness,
    AgentManagementLast24Hours,
    AgentManagementLastWeek,
    AgentManagementLocalAgent,
    AgentManagementLocalAgentDescription,
    AgentManagementNewAgent,
    AgentManagementPast3Days,
    AgentManagementPersonalFilter,
    AgentManagementPersonalFilterTooltip,
    AgentManagementSearch,
    AgentManagementSource,
    AgentManagementStatus,
    AgentManagementSuggested,
    AgentManagementViewAgents,
    AgentManagementWorking,
    ChooseYourAgent,
    CommonAll,
    CommonClose,
    CommonNone,
    ConversationActionCancelTask,
    ConversationActionCopyLinkToRun,
    ConversationActionForkConversation,
    ConversationActionOpenConversation,
    ConversationActionViewDetails,
    Notifications,
    NotificationsAllTabs,
    NotificationsErrors,
    NotificationsMarkAllAsRead,
    NotificationsNoNotifications,
    NotificationsUnread,
    SettingsAbout,
    SettingsAccount,
    SettingsAgents,
    SettingsAppearance,
    SettingsBillingAndUsage,
    SettingsCloudEnvironments,
    SettingsCloudPlatform,
    SettingsCode,
    SettingsCodeIndexing,
    SettingsEditorAndCodeReview,
    SettingsFeatures,
    SettingsKeybindings,
    SettingsKnowledge,
    SettingsMcpServers,
    SettingsOzCloudApiKeys,
    SettingsPrivacy,
    SettingsProfiles,
    SettingsReferrals,
    SettingsSharedBlocks,
    SettingsTeams,
    SettingsThirdPartyCliAgents,
    SettingsWarpAgent,
    SettingsWarpDrive,
    SettingsWarpify,
}

fn english(key: Key) -> &'static str {
    match key {
        Key::AgentManagementAllFilter => "All",
        Key::AgentManagementAllFilterTooltip => "View your agent tasks plus all shared team tasks",
        Key::AgentManagementArtifactFile => "File",
        Key::AgentManagementArtifactPlan => "Plan",
        Key::AgentManagementArtifactPullRequest => "Pull Request",
        Key::AgentManagementArtifactScreenshot => "Screenshot",
        Key::AgentManagementClearAll => "Clear all",
        Key::AgentManagementClearFilters => "Clear filters",
        Key::AgentManagementCloudAgent => "Cloud agent",
        Key::AgentManagementCloudAgentDescription => {
            "Runs autonomously in a cloud environment you choose. Best for parallel or long-running work."
        }
        Key::AgentManagementCreatedBy => "Created by",
        Key::AgentManagementCreatedOn => "Created on",
        Key::AgentManagementDone => "Done",
        Key::AgentManagementEnvironment => "Environment",
        Key::AgentManagementFailed => "Failed",
        Key::AgentManagementGetStarted => "Get started",
        Key::AgentManagementHasArtifact => "Has artifact",
        Key::AgentManagementHarness => "Harness",
        Key::AgentManagementLast24Hours => "Last 24 hours",
        Key::AgentManagementLastWeek => "Last week",
        Key::AgentManagementLocalAgent => "Local agent",
        Key::AgentManagementLocalAgentDescription => {
            "Runs on your machine and requires supervision. Best for quick, interactive tasks."
        }
        Key::AgentManagementNewAgent => "New agent",
        Key::AgentManagementPast3Days => "Past 3 days",
        Key::AgentManagementPersonalFilter => "Personal",
        Key::AgentManagementPersonalFilterTooltip => "View agent tasks you created",
        Key::AgentManagementSearch => "Search",
        Key::AgentManagementSource => "Source",
        Key::AgentManagementStatus => "Status",
        Key::AgentManagementSuggested => "Suggested",
        Key::AgentManagementViewAgents => "View Agents",
        Key::AgentManagementWorking => "Working",
        Key::ChooseYourAgent => "Choose your agent",
        Key::CommonAll => "All",
        Key::CommonClose => "Close",
        Key::CommonNone => "None",
        Key::ConversationActionCancelTask => "Cancel task",
        Key::ConversationActionCopyLinkToRun => "Copy link to run",
        Key::ConversationActionForkConversation => "Fork conversation",
        Key::ConversationActionOpenConversation => "Open conversation",
        Key::ConversationActionViewDetails => "View details",
        Key::Notifications => "Notifications",
        Key::NotificationsAllTabs => "All tabs",
        Key::NotificationsErrors => "Errors",
        Key::NotificationsMarkAllAsRead => "Mark all as read",
        Key::NotificationsNoNotifications => "No notifications",
        Key::NotificationsUnread => "Unread",
        Key::SettingsAbout => "About",
        Key::SettingsAccount => "Account",
        Key::SettingsAgents => "Agents",
        Key::SettingsAppearance => "Appearance",
        Key::SettingsBillingAndUsage => "Billing and usage",
        Key::SettingsCloudEnvironments => "Environments",
        Key::SettingsCloudPlatform => "Cloud platform",
        Key::SettingsCode => "Code",
        Key::SettingsCodeIndexing => "Indexing and projects",
        Key::SettingsEditorAndCodeReview => "Editor and Code Review",
        Key::SettingsFeatures => "Features",
        Key::SettingsKeybindings => "Keyboard shortcuts",
        Key::SettingsKnowledge => "Knowledge",
        Key::SettingsMcpServers => "MCP Servers",
        Key::SettingsOzCloudApiKeys => "Oz Cloud API Keys",
        Key::SettingsPrivacy => "Privacy",
        Key::SettingsProfiles => "Profiles",
        Key::SettingsReferrals => "Referrals",
        Key::SettingsSharedBlocks => "Shared blocks",
        Key::SettingsTeams => "Teams",
        Key::SettingsThirdPartyCliAgents => "Third party CLI agents",
        Key::SettingsWarpAgent => "Warp Agent",
        Key::SettingsWarpDrive => "Warp Drive",
        Key::SettingsWarpify => "Warpify",
    }
}

fn zh_cn(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::AgentManagementAllFilter => "全部",
        Key::AgentManagementAllFilterTooltip => "查看你的智能体任务以及团队共享的所有任务",
        Key::AgentManagementArtifactFile => "文件",
        Key::AgentManagementArtifactPlan => "计划",
        Key::AgentManagementArtifactPullRequest => "拉取请求",
        Key::AgentManagementArtifactScreenshot => "截图",
        Key::AgentManagementClearAll => "全部清除",
        Key::AgentManagementClearFilters => "清除筛选",
        Key::AgentManagementCloudAgent => "云端智能体",
        Key::AgentManagementCloudAgentDescription => {
            "在你选择的云环境中自主运行。适合并行任务或耗时较长的工作。"
        }
        Key::AgentManagementCreatedBy => "创建者",
        Key::AgentManagementCreatedOn => "创建时间",
        Key::AgentManagementDone => "已完成",
        Key::AgentManagementEnvironment => "环境",
        Key::AgentManagementFailed => "失败",
        Key::AgentManagementGetStarted => "开始使用",
        Key::AgentManagementHasArtifact => "包含产物",
        Key::AgentManagementHarness => "执行框架",
        Key::AgentManagementLast24Hours => "过去 24 小时",
        Key::AgentManagementLastWeek => "上周",
        Key::AgentManagementLocalAgent => "本地智能体",
        Key::AgentManagementLocalAgentDescription => {
            "在你的机器上运行，需要人工监督。适合快速、交互式任务。"
        }
        Key::AgentManagementNewAgent => "新建智能体",
        Key::AgentManagementPast3Days => "过去 3 天",
        Key::AgentManagementPersonalFilter => "个人",
        Key::AgentManagementPersonalFilterTooltip => "查看你创建的智能体任务",
        Key::AgentManagementSearch => "搜索",
        Key::AgentManagementSource => "来源",
        Key::AgentManagementStatus => "状态",
        Key::AgentManagementSuggested => "推荐",
        Key::AgentManagementViewAgents => "查看智能体",
        Key::AgentManagementWorking => "运行中",
        Key::ChooseYourAgent => "选择智能体",
        Key::CommonAll => "全部",
        Key::CommonClose => "关闭",
        Key::CommonNone => "无",
        Key::ConversationActionCancelTask => "取消任务",
        Key::ConversationActionCopyLinkToRun => "复制运行链接",
        Key::ConversationActionForkConversation => "派生会话",
        Key::ConversationActionOpenConversation => "打开会话",
        Key::ConversationActionViewDetails => "查看详情",
        Key::Notifications => "通知",
        Key::NotificationsAllTabs => "所有标签页",
        Key::NotificationsErrors => "错误",
        Key::NotificationsMarkAllAsRead => "全部标为已读",
        Key::NotificationsNoNotifications => "没有通知",
        Key::NotificationsUnread => "未读",
        Key::SettingsAbout => "关于",
        Key::SettingsAccount => "账户",
        Key::SettingsAgents => "智能体",
        Key::SettingsAppearance => "外观",
        Key::SettingsBillingAndUsage => "账单和用量",
        Key::SettingsCloudEnvironments => "环境",
        Key::SettingsCloudPlatform => "云平台",
        Key::SettingsCode => "代码",
        Key::SettingsCodeIndexing => "索引和项目",
        Key::SettingsEditorAndCodeReview => "编辑器和代码审查",
        Key::SettingsFeatures => "功能",
        Key::SettingsKeybindings => "键盘快捷键",
        Key::SettingsKnowledge => "知识",
        Key::SettingsMcpServers => "MCP 服务器",
        Key::SettingsOzCloudApiKeys => "Oz Cloud API 密钥",
        Key::SettingsPrivacy => "隐私",
        Key::SettingsProfiles => "配置",
        Key::SettingsReferrals => "推荐",
        Key::SettingsSharedBlocks => "共享块",
        Key::SettingsTeams => "团队",
        Key::SettingsThirdPartyCliAgents => "第三方 CLI 智能体",
        Key::SettingsWarpAgent => "Warp 智能体",
        Key::SettingsWarpDrive => "Warp Drive",
        Key::SettingsWarpify => "Warpify",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_supported_locale_identifiers() {
        assert_eq!(Locale::from_locale_identifier("zh_CN.UTF-8"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_locale_identifier("zh-Hans-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_locale_identifier("en_US.UTF-8"), Some(Locale::EnUs));
        assert_eq!(Locale::from_locale_identifier("C"), None);
    }

    #[test]
    fn translates_simplified_chinese() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        set_locale(Locale::ZhCn);
        assert_eq!(t(Key::Notifications), "通知");
        assert_eq!(t(Key::SettingsKeybindings), "键盘快捷键");
        set_locale(Locale::EnUs);
    }

    #[test]
    fn defaults_to_english() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        set_locale(Locale::EnUs);
        assert_eq!(t(Key::Notifications), "Notifications");
    }
}
