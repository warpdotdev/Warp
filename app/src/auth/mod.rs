//! OpenWarp 本地身份 facade。
//!
//! 该模块保留 `AuthState` / `AuthStateProvider` / `AuthManager` / `User` / `UserUid` /
//! `Credentials` 等类型表面 + pub 方法签名,**所有方法体本地化**:
//! - `is_logged_in()` / 各 `is_*` 谓词:固定返回本地用户对应的常量。
//! - `user_id()`:返回基于 `TEST_USER_UID` 的常量 [`UserUid`]。
//! - `username_for_display` / `display_name`:基于 [`User::test`] 占位元数据。
//! - 外部账号回调触发点(`AuthManager::initialize_user_from_auth_payload` 等):no-op,
//!   不再依赖远端账号客户端。
//!
//! 167 处 `crate::auth::AuthStateProvider::as_ref(ctx).get()` 调用一行不改即可继续编译,
//! 运行时永远拿到"已登录、Free Tier 无限额"的本地占位状态。
//!
//! 物理删除清单见 README:21 个 UI / RPC / token 持久化 / web handoff /
//! login_slide / paste_auth_token_modal / web_handoff 等文件随外部账号体系一并下线。

use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use crate::server_time::ServerTimestamp;

pub const TEST_USER_EMAIL: &str = "test_user@warp.dev";
pub const TEST_USER_UID: &str = "test_user_uid";

pub mod user_uid;

pub use user_uid::UserUid;

#[derive(Clone, Copy, Debug)]
pub enum OwnerType {
    Team,
    User,
}

/// OpenWarp 本地 API key 前缀。
///
/// 历史上用于识别"以 wk- 开头的字符串为托管 API key",在 BYOP 路径上
/// 已无托管账号 API key 概念。常量仍被 `AuthState::initialize` 内部消费 + 少量遗留
/// 调用点匹配前缀,因此保留。
pub const API_KEY_PREFIX: &str = "wk-";

// ---------- Credentials / AuthToken / LoginToken ----------
//
// 原来用于托管 token / API key / SessionCookie 几种认证方式的运行时分支。OpenWarp
// 本地化后只保留 `ApiKey` / `Test` 两种实际用得到的 variant 加上为编译兼容保留的
// `SessionCookie`。托管 token variant 已物理删除,所有原外部账号分支在 OpenWarp 下
// 永远走 `None` / 早 return。

/// 表示用户与 Warp 的认证方式。
///
/// OpenWarp 本地化分支:
/// - `ApiKey`:BYOP 路径下用户自携 LLM provider API key,实际由 settings/keychain
///   各自管理,这里只保留 enum facade 给 `AuthState::credentials()` 等读取方法。
/// - `SessionCookie`:web 端 stub,native 永远不会构造。
/// - `Test`:测试 / `skip_login` 构建下使用。
#[derive(Clone, Debug)]
pub enum Credentials {
    /// BYOP / Warp Inc API key,保留 owner_type 供旧代码读取(永远 `None`)。
    ApiKey {
        key: String,
        owner_type: Option<OwnerType>,
    },
    /// Web 端 session cookie。
    SessionCookie,
    /// 测试 / `skip_login` 构建占位。
    Test,
}

impl Credentials {
    /// 返回 API key 字符串(仅当 variant 为 [`Credentials::ApiKey`])。
    pub fn as_api_key(&self) -> Option<&str> {
        match self {
            Credentials::ApiKey { key, .. } => Some(key),
            Credentials::SessionCookie | Credentials::Test => None,
        }
    }

    /// 返回 API key owner type(OpenWarp 路径下永远 `None`)。
    pub fn api_key_owner_type(&self) -> Option<OwnerType> {
        match self {
            Credentials::ApiKey { owner_type, .. } => *owner_type,
            Credentials::SessionCookie | Credentials::Test => None,
        }
    }

    /// 返回要写入 Authorization 头的 bearer token。
    ///
    /// 本地化后只有 `ApiKey` 产出真实值;`SessionCookie` / `Test` 返回 [`AuthToken::NoAuth`]。
    pub fn bearer_token(&self) -> AuthToken {
        match self {
            Credentials::ApiKey { key, .. } => AuthToken::ApiKey(key.clone()),
            Credentials::SessionCookie | Credentials::Test => AuthToken::NoAuth,
        }
    }
}

/// HTTP 请求头使用的短期 token。
#[derive(Debug, Clone)]
pub enum AuthToken {
    /// BYOP / 平台层 API key。
    ApiKey(String),
    /// 无任何 token(session cookie / test / OpenWarp 本地模式)。
    NoAuth,
}

impl AuthToken {
    /// 返回 bearer token 字符串(若有)。
    pub fn bearer_token(&self) -> Option<String> {
        match self {
            AuthToken::ApiKey(key) => Some(key.clone()),
            AuthToken::NoAuth => None,
        }
    }

    /// 返回 Authorization 头使用的 token 引用。
    pub fn as_bearer_token(&self) -> Option<&str> {
        match self {
            AuthToken::ApiKey(key) => Some(key),
            AuthToken::NoAuth => None,
        }
    }
}

// ---------- User 元数据 ----------

/// 匿名用户类型 facade。OpenWarp 本地化后无匿名用户概念,保留 enum 是为了让
/// 散落在 telemetry / settings 中的 match arm 仍能编译。所有 OpenWarp 代码路径
/// 均不会构造 `Some(AnonymousUserType::...)`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnonymousUserType {
    NativeClientAnonymousUser,
    NativeClientAnonymousUserFeatureGated,
    WebClientAnonymousUser,
}

/// 认证 principal 类型 facade。OpenWarp 永远等同 `User`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrincipalType {
    #[default]
    User,
    ServiceAccount,
}

/// 个人对象限额 facade(原匿名用户 Free Tier 限额)。OpenWarp 永不构造此值,
/// 但保留 struct 让消费方继续编译。
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PersonalObjectLimits {
    pub env_var_limit: usize,
    pub notebook_limit: usize,
    pub workflow_limit: usize,
}

/// 用户元数据 facade,只保留少数字段供 telemetry / display 使用。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserMetadata {
    pub email: String,
    pub display_name: Option<String>,
    pub photo_url: Option<String>,
}

/// 当前登录用户(本地占位)。
#[derive(Debug, Clone)]
pub struct User {
    pub local_id: UserUid,
    pub metadata: UserMetadata,
    pub is_onboarded: bool,
    pub needs_sso_link: bool,
    pub anonymous_user_type: Option<AnonymousUserType>,
    pub is_on_work_domain: bool,
    pub linked_at: Option<ServerTimestamp>,
    pub personal_object_limits: Option<PersonalObjectLimits>,
    pub principal_type: PrincipalType,
}

impl User {
    /// 用于显示的用户名 — display_name 优先,否则 email。
    pub fn username_for_display(&self) -> &str {
        self.metadata
            .display_name
            .as_deref()
            .unwrap_or(self.metadata.email.as_str())
    }

    /// 用户显示名,不回退到 email。
    pub fn display_name(&self) -> Option<String> {
        self.metadata.display_name.clone()
    }

    /// 测试/默认用户占位。OpenWarp 在所有路径下都使用此用户。
    pub fn test() -> Self {
        Self {
            local_id: UserUid::new(TEST_USER_UID),
            metadata: UserMetadata {
                email: TEST_USER_EMAIL.to_string(),
                display_name: None,
                photo_url: None,
            },
            is_onboarded: true,
            needs_sso_link: false,
            anonymous_user_type: None,
            is_on_work_domain: false,
            linked_at: None,
            personal_object_limits: None,
            principal_type: PrincipalType::User,
        }
    }

    /// 用户是否匿名。OpenWarp 永远返回 `false`。
    pub fn is_user_anonymous(&self) -> bool {
        false
    }

    pub fn anonymous_user_type(&self) -> Option<AnonymousUserType> {
        self.anonymous_user_type
    }

    pub fn personal_object_limits(&self) -> Option<PersonalObjectLimits> {
        self.personal_object_limits
    }

    pub fn linked_at(&self) -> Option<ServerTimestamp> {
        self.linked_at
    }
}

// ---------- AuthState ----------

/// 当前认证状态(本地化 stub)。
///
/// 所有"是否登录、是否匿名、是否需要重新认证"的查询都返回固定值;
/// `user_id()` 永远返回 `Some(UserUid::new(TEST_USER_UID))`。
/// 167+ 个消费点零改动即可编译。
pub struct AuthState {
    user: RwLock<Option<User>>,
    credentials: RwLock<Option<Credentials>>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new_for_test()
    }
}

impl AuthState {
    /// 创建本地默认 AuthState(永远视为已登录的测试用户)。
    pub fn new() -> Self {
        Self {
            user: RwLock::new(Some(User::test())),
            credentials: RwLock::new(Some(Credentials::Test)),
        }
    }

    /// 测试场景下构造 AuthState(等价于 [`AuthState::new`])。
    pub fn new_for_test() -> Self {
        Self::new()
    }

    /// 初始化 AuthState。`api_key` 参数被忠实保留(BYOP 入口仍可能传入),
    /// 但其他外部账号检查路径全部 no-op。
    #[cfg_attr(target_family = "wasm", allow(unused_variables))]
    pub fn initialize(_ctx: &AppContext, api_key: Option<String>) -> Self {
        let state = Self::new();
        if let Some(api_key_value) = api_key {
            let formatted = if api_key_value.starts_with(API_KEY_PREFIX) {
                api_key_value
            } else {
                format!("{API_KEY_PREFIX}{api_key_value}")
            };
            *state.credentials.write() = Some(Credentials::ApiKey {
                key: formatted,
                owner_type: None,
            });
        }
        state
    }

    /// 用户是否已登录。OpenWarp 永远 `true`。
    pub fn is_logged_in(&self) -> bool {
        true
    }

    /// 是否匿名或登出。OpenWarp 永远 `false`。
    pub fn is_anonymous_or_logged_out(&self) -> bool {
        false
    }

    /// 返回缓存的 access token(忽略有效性)。OpenWarp 路径下仅当用户挂了
    /// `Credentials::ApiKey` 才有值。
    pub fn get_access_token_ignoring_validity(&self) -> Option<String> {
        self.credentials
            .read()
            .as_ref()?
            .bearer_token()
            .bearer_token()
    }

    pub fn username_for_display(&self) -> Option<String> {
        Some(self.user.read().as_ref()?.username_for_display().to_owned())
    }

    pub fn display_name(&self) -> Option<String> {
        self.user
            .read()
            .as_ref()
            .and_then(|user| user.display_name())
    }

    pub fn user_email(&self) -> Option<String> {
        self.user
            .read()
            .as_ref()
            .map(|user| user.metadata.email.clone())
    }

    pub fn is_onboarded(&self) -> Option<bool> {
        self.user.read().as_ref().map(|user| user.is_onboarded)
    }

    pub fn user_email_domain(&self) -> Option<String> {
        self.user.read().as_ref().map(|user| {
            user.metadata
                .email
                .split('@')
                .nth(1)
                .unwrap_or("")
                .to_string()
        })
    }

    pub fn is_user_anonymous(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_user_web_anonymous_user(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_anonymous_user_feature_gated(&self) -> Option<bool> {
        Some(false)
    }

    /// OpenWarp 本地用户永不会撞 Free Tier 限额。
    pub fn is_anonymous_user_past_object_limit(
        &self,
        _object_type: crate::cloud_object::ObjectType,
        _num_objects: usize,
    ) -> Option<bool> {
        Some(false)
    }

    pub fn user_photo_url(&self) -> Option<String> {
        self.user
            .read()
            .as_ref()
            .and_then(|user| user.metadata.photo_url.clone())
    }

    pub fn needs_sso_link(&self) -> Option<bool> {
        Some(false)
    }

    pub fn anonymous_user_type(&self) -> Option<AnonymousUserType> {
        None
    }

    pub fn personal_object_limits(&self) -> Option<PersonalObjectLimits> {
        None
    }

    /// 标记用户为已 onboarded。
    pub fn set_is_onboarded(&self, is_onboarded: bool) {
        if let Some(user) = self.user.write().as_mut() {
            user.is_onboarded = is_onboarded;
        }
    }

    pub fn user_id(&self) -> Option<UserUid> {
        self.user.read().as_ref().map(|user| user.local_id)
    }

    /// 返回 nil UUID 字符串。OpenWarp 本地化后,该 ID 不再出现在
    /// 任何外发 HTTP 头中,仅为给 telemetry 上下文 / session 头提供形式上的占位。
    pub fn anonymous_id(&self) -> String {
        Uuid::nil().to_string()
    }

    /// 返回是否需要重新认证。OpenWarp 永远 `false`。
    pub fn needs_reauth(&self) -> bool {
        false
    }

    /// 返回当前用户的 anonymous renotification block 是否过期。OpenWarp 用户
    /// 不被视作匿名用户,该函数返回 `false`(永不弹注册提示)。
    pub fn anonymous_user_renotification_block_expired(
        &self,
        _last_time_opt: Option<String>,
    ) -> bool {
        false
    }

    pub fn is_on_work_domain(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_api_key_authenticated(&self) -> bool {
        matches!(
            self.credentials.read().as_ref(),
            Some(Credentials::ApiKey { .. })
        )
    }

    pub fn api_key(&self) -> Option<String> {
        self.credentials
            .read()
            .as_ref()
            .and_then(|c| c.as_api_key().map(|s| s.to_owned()))
    }

    pub fn principal_type(&self) -> Option<PrincipalType> {
        Some(PrincipalType::User)
    }

    pub fn is_service_account(&self) -> bool {
        false
    }

    pub fn api_key_owner_type(&self) -> Option<OwnerType> {
        self.credentials.read().as_ref()?.api_key_owner_type()
    }

    /// 返回当前 credentials 的克隆。
    pub fn credentials(&self) -> Option<Credentials> {
        self.credentials.read().clone()
    }

    /// 将本地 auth 状态恢复到本地占位用户的默认快照，用于 `log_out` 及本地重置路径。
    pub fn reset_local_defaults(&self) {
        *self.user.write() = Some(User::test());
        *self.credentials.write() = Some(Credentials::Test);
    }
}

impl warp_managed_secrets::ActorProvider for AuthState {
    fn actor_uid(&self) -> Option<String> {
        self.user_id().map(|uid| uid.as_string())
    }
}

/// AuthState 的 singleton 包装。
pub struct AuthStateProvider {
    auth_state: Arc<AuthState>,
}

impl AuthStateProvider {
    pub fn new(auth_state: Arc<AuthState>) -> Self {
        Self { auth_state }
    }

    pub fn new_for_test() -> Self {
        Self {
            auth_state: Arc::new(AuthState::new_for_test()),
        }
    }

    /// 构造一个"已登出"的 AuthState provider。
    ///
    /// OpenWarp 不再有真正的登出状态,本函数返回与 `new_for_test` 等价的
    /// "已登录测试用户"provider,以保证旧测试代码继续编译。
    pub fn new_logged_out_for_test() -> Self {
        Self::new_for_test()
    }

    pub fn get(&self) -> &Arc<AuthState> {
        &self.auth_state
    }
}

impl Entity for AuthStateProvider {
    type Event = ();
}

impl SingletonEntity for AuthStateProvider {}

// ---------- AuthManager facade ----------

/// 旧 UI 遗留的 "登录被门控 " 标识,作为字符串常量(原 `&'static str`)。
pub type LoginGatedFeature = &'static str;

/// `AuthManager::open_url_maybe_with_anonymous_token` 的 url 构造回调。
///
/// 在原 UI 中,该回调会收到匿名用户 token 后拼装 出”打开浏览器 可附带身份“的 URL。
/// OpenWarp 下匿名身份不再存在,回调被丢弃。
pub type AnonymousTokenUrlBuilder = Box<dyn FnOnce(Option<&str>) -> String>;

/// 跨 modal / login 流程透传的回跳 payload,OpenWarp 本地化后只保留 struct 表面
/// 以兼容 `AuthManagerEvent::LoginOverrideDetected` 与 `AuthRedirectPayload::from_url`
/// 调用点。OpenWarp 永远不会通过浏览器回跳触发外部账号登录。
#[derive(Clone, Debug, Default)]
pub struct AuthRedirectPayload {
    pub refresh_token_placeholder: (),
}

impl AuthRedirectPayload {
    pub fn from_url(_url: url::Url) -> Result<Self, anyhow::Error> {
        Err(anyhow::anyhow!(
            "OpenWarp 已下线云端登录,不再处理浏览器回跳 URL"
        ))
    }
}

/// AuthView 变体 facade。OpenWarp 已物理删 AuthView UI,所有派发点在 stub 中
/// 仅产生 log,但 enum 表面保留供旧 `match` arm 编译通过。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthViewVariant {
    Initial,
    RequireLoginCloseable,
    ShareRequirementCloseable,
}

// ---------- UI view facade(原物理删 UI 的占位) ----------
//
// `root_view.rs` / `workspace/view.rs` 原本持有 6 个 `ViewHandle<T>` 字段,
// 以及起源于这些 view 的事件。Wave 3-1 物理删 view body 后,保留这些
// view + event enum facade使`ViewHandle<AuthView>` 类型、事件 match arm 、
// `ctx.add_typed_action_view(AuthView::new)` 调用仍能编译。
//
// 运行时这些 view 代码路径仍会被创建但不渲染(`View::render` 返回 `Empty`)、
// 事件不被触发(原 UI 交互点已不存在)。

use warpui::elements::Empty;
use warpui::{Element, View, ViewContext};

/// AuthView facade。原 UI 包含《登录 / 注册》表单,本地化后已物理删除。
pub struct AuthView {
    variant: AuthViewVariant,
    /// 原 UI 记录上一次登录失败原因,Wave 3-1 仅作字段保留 以供原赋值点编译。
    pub last_login_failure_reason: Option<LoginFailureReason>,
}

impl AuthView {
    pub fn new(variant: AuthViewVariant, _ctx: &mut ViewContext<Self>) -> Self {
        Self {
            variant,
            last_login_failure_reason: None,
        }
    }

    pub fn set_variant(&mut self, _ctx: &mut ViewContext<Self>, variant: AuthViewVariant) {
        self.variant = variant;
    }

    /// 返回当前 variant。OpenWarp 路径下不使用。
    pub fn variant(&self) -> AuthViewVariant {
        self.variant
    }

    /// 原原生登录 UI 跳过 ”输入口令 “ 进 入后续 ”在浏览器中打开 “步。 OpenWarp:no-op。
    pub fn skip_to_browser_open_step(&mut self, _ctx: &mut ViewContext<Self>) {}
}

impl Entity for AuthView {
    type Event = AuthViewEvent;
}

impl View for AuthView {
    fn ui_name() -> &'static str {
        "AuthView (stub)"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Box::new(Empty::new())
    }
}

impl warpui::TypedActionView for AuthView {
    type Action = ();
    fn handle_action(&mut self, _action: &(), _ctx: &mut ViewContext<Self>) {}
}

#[derive(Debug)]
pub enum AuthViewEvent {
    Close,
}

/// AuthOverrideWarningModal facade。
pub struct AuthOverrideWarningModal;

impl AuthOverrideWarningModal {
    pub fn new(_ctx: &mut ViewContext<Self>, _variant: AuthOverrideWarningModalVariant) -> Self {
        Self
    }

    /// 设置 被中断的 auth payload。OpenWarp:no-op。
    pub fn set_interrupted_auth_payload(&mut self, _payload: AuthRedirectPayload) {}
}

impl Entity for AuthOverrideWarningModal {
    type Event = AuthOverrideWarningModalEvent;
}

impl View for AuthOverrideWarningModal {
    fn ui_name() -> &'static str {
        "AuthOverrideWarningModal (stub)"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Box::new(Empty::new())
    }
}

impl warpui::TypedActionView for AuthOverrideWarningModal {
    type Action = ();
    fn handle_action(&mut self, _action: &(), _ctx: &mut ViewContext<Self>) {}
}

#[derive(Debug)]
pub enum AuthOverrideWarningModalEvent {
    Close,
    BulkExport,
}

#[derive(Clone, Copy, Debug)]
pub enum AuthOverrideWarningModalVariant {
    OnboardingView,
    WorkspaceModal,
}

/// NeedsSsoLinkView facade。
pub struct NeedsSsoLinkView;

impl NeedsSsoLinkView {
    pub fn new() -> Self {
        Self
    }

    pub fn set_email(&mut self, _email: String) {}
}

impl Default for NeedsSsoLinkView {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for NeedsSsoLinkView {
    type Event = ();
}

impl View for NeedsSsoLinkView {
    fn ui_name() -> &'static str {
        "NeedsSsoLinkView (stub)"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Box::new(Empty::new())
    }
}

impl warpui::TypedActionView for NeedsSsoLinkView {
    type Action = ();
    fn handle_action(&mut self, _action: &(), _ctx: &mut ViewContext<Self>) {}
}

/// WebHandoffView facade (wasm-only 重新登录入口)。
pub struct WebHandoffView;

impl WebHandoffView {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }
}

impl Entity for WebHandoffView {
    type Event = WebHandoffEvent;
}

impl View for WebHandoffView {
    fn ui_name() -> &'static str {
        "WebHandoffView (stub)"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Box::new(Empty::new())
    }
}

#[derive(Debug)]
pub enum WebHandoffEvent {
    Unsupported,
}

/// 登录失败原因 facade(原 `auth/login_failure_notification.rs` 定义)。
#[derive(Clone, Debug)]
pub enum LoginFailureReason {
    InvalidRedirectUrl { was_pasted: bool },
    Generic,
}

/// AuthManager 事件 facade。`AuthManagerEvent::AuthComplete` 仍可被
/// `AuthManager::new` 内部触发以兼容部分订阅方对"已认证"信号的依赖。
#[derive(Debug)]
pub enum AuthManagerEvent {
    AuthComplete,
    AuthFailed(UserAuthenticationError),
    SkippedLogin,
    NeedsReauth,
    AttemptedLoginGatedFeature {
        auth_view_variant: AuthViewVariant,
    },
    LoginOverrideDetected(AuthRedirectPayload),
    /// CLI headless device auth 路径中原发出的"已拿到 device authorization code"事件。
    /// OpenWarp 路径下不再触发,但 enum variant 保留以免旧 `match` 调用点报错。
    ReceivedDeviceAuthorizationCode {
        verification_url: String,
        verification_url_complete: Option<String>,
        user_code: String,
    },
    /// 低频 失败:同上。
    CreateAnonymousUserFailed,
}

/// 用户认证错误 facade。少量订阅方仍 match 各 variant,因此保留 enum;
/// OpenWarp 不再触发任何 variant 的构造。
#[derive(Debug, thiserror::Error)]
pub enum UserAuthenticationError {
    #[error("Access token denied")]
    DeniedAccessToken,
    #[error("User account disabled")]
    UserAccountDisabled,
    #[error("Invalid state parameter")]
    InvalidStateParameter,
    #[error("Missing state parameter")]
    MissingStateParameter,
    #[error("Unexpected error: {0}")]
    Unexpected(anyhow::Error),
}

/// 服务端持久化的用户隐私设置 facade,仍被 `settings/privacy.rs` 消费。
#[derive(Copy, Clone, Debug, Default)]
pub struct SyncedUserSettings {
    pub is_crash_reporting_enabled: bool,
    pub is_telemetry_enabled: bool,
}

/// 持久化在 SQLite `current_user_information` 表里的当前用户信息。
/// `persistence/sqlite.rs` 与 `persistence/mod.rs` 仍消费该 struct,保留。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedCurrentUserInformation {
    pub email: String,
}

/// AuthManager facade。OpenWarp 本地化后所有外部账号/RPC 入口都成为 no-op,
/// `AuthManager` 仍作为 singleton 模型挂在 App 中,以保证 `subscribe_to_model` /
/// `handle(ctx).update(...)` 调用 0 改动,同时保留本地身份 / onboarded 标记 /
/// logout reset 语义。
pub struct AuthManager {
    auth_state: Arc<AuthState>,
}

impl AuthManager {
    /// 创建 AuthManager。本地化后不再接受外部账号客户端参数。
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
        Self { auth_state }
    }

    /// 测试场景构造,与 [`Self::new`] 等价。
    pub fn new_for_test(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(ctx)
    }

    /// 历史上从浏览器回跳 URL 重建用户态。本地化:no-op,只 log。
    pub fn initialize_user_from_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _enforce_state_validation: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
        log::debug!("AuthManager::initialize_user_from_auth_payload 已 no-op(OpenWarp)");
    }

    /// 恢复被中断的 auth payload。本地化:no-op。
    pub fn resume_interrupted_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _ctx: &mut ModelContext<Self>,
    ) {
        log::debug!("AuthManager::resume_interrupted_auth_payload 已 no-op(OpenWarp)");
    }

    /// 刷新当前用户态。
    ///
    /// 历史上这里会走云端 token 刷新;OpenWarp 本地化后认证状态在启动时已完成
    /// 本地初始化,不再发任何外部账号请求。
    pub fn refresh_user(&self, _ctx: &mut ModelContext<Self>) {}

    /// 设备授权码流(CLI 启动登录)。本地化:no-op。
    pub fn authorize_device(&self, _ctx: &mut ModelContext<Self>) {
        log::debug!("AuthManager::authorize_device 已 no-op(OpenWarp)");
    }

    /// 主动登出。
    ///
    /// OpenWarp 不再进入“云端已登出”状态,这里仅把本地身份快照恢复成默认占位用户,
    /// 供设置重置 / 会话清理等调用点复用。
    pub(crate) fn log_out(&mut self, _ctx: &mut ModelContext<Self>) {
        self.auth_state.reset_local_defaults();
        log::debug!("AuthManager::log_out 已本地 reset: 已切换为本地占位用户态");
    }

    /// 标记需要重新认证。本地化:no-op。
    pub fn set_needs_reauth(&mut self, _new_value: bool, _ctx: &mut ModelContext<Self>) {}

    /// 创建匿名用户。本地化:no-op,直接发出 `AuthComplete` 让 onboarding 流推进。
    pub fn create_anonymous_user(
        &mut self,
        _referral_code: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        ctx.emit(AuthManagerEvent::AuthComplete);
    }

    /// 派发"匿名用户尝试触碰登录门控功能"。本地化:no-op。
    pub fn attempt_login_gated_feature(
        &mut self,
        _feature: LoginGatedFeature,
        _auth_view_variant: AuthViewVariant,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// 匿名用户撞 Drive 限额提醒。本地化:no-op。
    pub fn anonymous_user_hit_drive_object_limit(&mut self, _ctx: &mut ModelContext<Self>) {}

    /// 启动匿名用户 → 完整用户的浏览器登录链路。本地化:no-op。
    pub fn initiate_anonymous_user_linking(
        &mut self,
        _entrypoint: crate::server::telemetry::AnonymousUserSignupEntrypoint,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// 用户引导走完后置本地 onboarded 标记。
    pub fn set_user_onboarded(&mut self, ctx: &mut ModelContext<Self>) {
        self.auth_state.set_is_onboarded(true);
        ctx.emit(AuthManagerEvent::AuthComplete);
    }

    // ---------- URL 构造 facade ----------
    //
    // 旧 UI(login_slide / paste_auth_token_modal / auth_view_modal)在物理删除前
    // 会调用这些方法以填充历史登录提示链接;OpenWarp 不再打开 Warp 云登录页。
    // 物理删 UI 后已无调用方,但 enum/trait 仍可能被反射式消费,保留 stub。

    pub fn sign_up_url(&self) -> String {
        String::new()
    }

    pub fn sign_in_url(&self) -> String {
        String::new()
    }

    pub fn upgrade_url(&self) -> String {
        String::new()
    }

    pub fn login_options_url(&self) -> String {
        String::new()
    }

    pub fn link_sso_url(&self) -> String {
        String::new()
    }

    /// 用浏览器打开 url,可选附带匿名 token。本地化:no-op。
    pub fn open_url_maybe_with_anonymous_token(
        &mut self,
        _ctx: &mut ModelContext<Self>,
        _url_constructor: AnonymousTokenUrlBuilder,
    ) {
    }

    /// 复制匿名用户登录链接到剪贴板。本地化:no-op。
    pub fn copy_anonymous_user_linking_url_to_clipboard(&mut self, _ctx: &mut ModelContext<Self>) {}
}

impl Entity for AuthManager {
    type Event = AuthManagerEvent;
}

impl SingletonEntity for AuthManager {}

// ---------- 全模块 init ----------

/// OpenWarp 本地身份 facade 的 init(no-op)。
///
/// 原 `init` 中挂载的 `init` / `auth_view_body::init` /
/// `auth_override_warning_body::init` / `login_slide::init` /
/// `paste_auth_token_modal::init` 子模块均已物理删除。
pub fn init(_app: &mut AppContext) {}
