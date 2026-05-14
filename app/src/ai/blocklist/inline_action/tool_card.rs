//! 统一的工具卡片渲染 helper,对齐 opencode TUI 的 `InlineTool` / `BlockTool`。
//!
//! ## 设计哲学
//!
//! opencode 把每个 ToolPart 渲染严格按 4 状态机切样式:
//! - `pending`(args 还在累积):浅灰文本 "Writing command..." / "Reading file..."
//! - `running`(args 完整,正在执行):BrailleSpinner + 标题文字
//! - `completed`(成功结束):静态 icon + 工具描述,可折叠
//! - `error`(失败 / 拒绝):红色错误文字,denied 时全文 STRIKETHROUGH
//!
//! 所有 12 个内置工具(Bash/Read/Glob/Grep/Edit/Write/...)只用 InlineTool /
//! BlockTool 两个组件;新工具接入时**只填语义**,不重新实现卡片骨架。
//!
//! ## warp 现状
//!
//! warp 的 inline_action/ 目录每个 view(web_search.rs / web_fetch.rs /
//! requested_command.rs / requested_action.rs / ...)各
//! 自完整渲染卡片(header + body + footer + permission ring + 状态切换),
//! 重复样板 ~150 行起。这是历史包袱,**全量重构需要一次性改 12+ 个 view**,
//! 风险大、阻力大。
//!
//! 本模块作为**渐进式重构入口**:
//! 1. 定义统一 API([`ToolCardState`] 状态机 + [`ToolCardSpec`] builder);
//! 2. 提供 [`render_inline_tool_card`] / [`render_block_tool_card`] 两个 helper;
//! 3. **新加的 inline_action 优先用本模块**;旧 view 保留不动,等单独 PR 收敛。
//!
//! 当前已经先在 `search_results_common.rs` 加了 `render_loading_header_animated` /
//! `render_terminal_header_strikethrough`,本模块在其上叠加完整 spec 抽象。

use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warpui::elements::shimmering_text::ShimmeringTextStateHandle;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Element, Flex, MainAxisAlignment,
    ParentElement, Radius, Shrinkable,
};
use warpui::{AppContext, SingletonEntity};

use super::inline_action_header::{
    ICON_MARGIN, INLINE_ACTION_HEADER_VERTICAL_PADDING, INLINE_ACTION_HORIZONTAL_PADDING,
};
use super::inline_action_icons::icon_size;
use crate::ui_components::spinner::SpinnerStateHandle;

/// 工具卡的当前状态。**严格 5 态对齐 opencode TUI**:
/// 不要为图省事新增中间态——所有渲染分支只接受这 5 个 case。
///
/// 5 态而非 opencode 4 态:多了 [`Self::PermissionPending`],对应 warp 的
/// `AIActionStatus::Blocked`(等用户允许)。opencode 把这个塞进 InlineTool 的
/// 整卡 fg→warning 色逻辑里,我们抽成显式 case 更清晰。
#[derive(Clone)]
pub enum ToolCardState {
    /// args 还在累积或工具尚未实际执行。视觉:静态 icon + "Writing command..." 等
    /// 进行时短语 + 浅灰文字。
    Pending {
        /// 进行时短语,如 "Writing command", "Reading file"。无需结尾 `...`,
        /// 渲染时自动补。
        verb: String,
    },
    /// 工具正在执行。视觉:`BrailleSpinner`(80ms 帧切换)+ ShimmeringText 标题。
    Running {
        title: String,
        spinner_handle: SpinnerStateHandle,
        shimmer_handle: ShimmeringTextStateHandle,
    },
    /// 等待用户允许执行(`AIActionStatus::Blocked`)。
    /// 视觉:**header background 切 warning 黄**,文字保持高对比度,
    /// 对齐 opencode 的 `if (permission()) return theme.warning`。
    /// detail 通常是 "OK if I run this command?" / "OK if I call this MCP tool?"。
    PermissionPending { title: String, detail: String },
    /// 工具成功完成。视觉:绿色 check icon + 工具描述。
    Completed { title: String },
    /// 工具失败 / 用户拒绝。`denied=true` 时标题文字带 STRIKETHROUGH 删除线
    /// 表达"被驳回",对齐 opencode `<text attributes={STRIKETHROUGH}>`。
    Error {
        title: String,
        denied: bool,
        detail: Option<String>,
    },
}

impl ToolCardState {
    /// 等价 opencode `part.state.status === "running"`。spinner 仅在 Running 时显示。
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// 等价 opencode `part.state.status === "completed"`。可被 hide_completed_tool_cards
    /// setting 隐藏。
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// 是否为 denied(用户拒绝),用于切删除线视觉。
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Error { denied: true, .. })
    }

    /// 是否为 permission pending(等用户允许),用于切 warning 背景色。
    pub fn is_permission_pending(&self) -> bool {
        matches!(self, Self::PermissionPending { .. })
    }
}

/// 工具卡 spec —— caller 填的所有必要信息。
pub struct ToolCardSpec {
    /// 工具图标(终态用,Pending/Running 时根据状态自行选 spinner)。
    pub icon: warpui::elements::Icon,
    /// 当前状态。
    pub state: ToolCardState,
}

/// 渲染 inline 模式工具卡(单行 icon + 文本)。对齐 opencode `InlineTool`。
///
/// 适合简短描述:Glob "*.rs" / Grep "TODO" / WebFetch URL。
/// **限制**:body 高度始终 1 行;复杂内容(diff / file list)走 [`render_block_tool_card`]。
pub fn render_inline_tool_card(spec: ToolCardSpec, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    // T3-6:permission pending 走 warning 黄背景,其它走 surface_2 默认背景。
    let header_background: Fill = if spec.state.is_permission_pending() {
        Fill::Solid(theme.ui_warning_color())
    } else {
        theme.surface_2()
    };

    let mut row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);

    // icon: Running 时换 BrailleSpinner,其它状态用传入的 icon。
    let icon_element: Box<dyn Element> = match &spec.state {
        ToolCardState::Running { spinner_handle, .. } => {
            use warp_core::ui::theme::AnsiColorIdentifier;
            let color = AnsiColorIdentifier::Yellow.to_ansi_color(&theme.terminal_colors().normal);
            Box::new(crate::ui_components::spinner::BrailleSpinner::new(
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
                color,
                spinner_handle.clone(),
            ))
        }
        _ => spec.icon.finish(),
    };
    let icon_box = ConstrainedBox::new(icon_element)
        .with_width(icon_size(app))
        .with_height(icon_size(app))
        .finish();
    row.add_child(
        Container::new(icon_box)
            .with_margin_right(ICON_MARGIN)
            .finish(),
    );

    // 文本:四种状态各自构造。
    let title_element = build_title_text(&spec.state, header_background, app);
    row.add_child(Shrinkable::new(1.0, title_element).finish());

    Container::new(row.finish())
        .with_horizontal_padding(INLINE_ACTION_HORIZONTAL_PADDING)
        .with_vertical_padding(INLINE_ACTION_HEADER_VERTICAL_PADDING)
        .with_background(header_background)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
}

/// 渲染 block 模式工具卡(header + body)。对齐 opencode `BlockTool`。
///
/// header 同 inline_tool_card;body 是用户传入的任意 Element(diff、文件列表、
/// 输出预览等)。Running 时 header 走 spinner,body 通常是 in-progress 数据。
pub fn render_block_tool_card(
    spec: ToolCardSpec,
    body: Box<dyn Element>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let body_background = theme.surface_1();

    let header = render_inline_tool_card(spec, app);
    let body_container = Container::new(body)
        .with_background(body_background)
        .with_horizontal_padding(INLINE_ACTION_HORIZONTAL_PADDING)
        .with_vertical_padding(INLINE_ACTION_HEADER_VERTICAL_PADDING)
        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(8.)))
        .finish();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(header);
    col.add_child(body_container);
    col.finish()
}

fn build_title_text(
    state: &ToolCardState,
    header_background: Fill,
    app: &AppContext,
) -> Box<dyn Element> {
    use warpui::elements::shimmering_text::{ShimmerConfig, ShimmeringTextElement};
    use warpui::elements::Text;

    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    match state {
        ToolCardState::Pending { verb } => {
            let color = theme.sub_text_color(header_background).into_solid();
            Text::new_inline(
                format!("{verb}..."),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(color)
            .finish()
        }
        ToolCardState::Running {
            title,
            shimmer_handle,
            ..
        } => {
            let base_color = theme.sub_text_color(header_background).into_solid();
            let shimmer_color = theme.main_text_color(header_background).into_solid();
            ShimmeringTextElement::new(
                title.clone(),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
                base_color,
                shimmer_color,
                ShimmerConfig::default(),
                shimmer_handle.clone(),
            )
            .finish()
        }
        ToolCardState::Completed { title } => {
            let color = theme.main_text_color(header_background).into();
            Text::new_inline(
                title.clone(),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(color)
            .finish()
        }
        ToolCardState::PermissionPending { title, detail } => {
            // 主标题 + detail 副行。background 已切 warning,文字用主色保证对比。
            let main_color = theme.main_text_color(header_background).into();
            let detail_color = theme.sub_text_color(header_background).into_solid();
            let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
            col.add_child(
                Text::new_inline(
                    title.clone(),
                    appearance.ui_font_family(),
                    appearance.monospace_font_size(),
                )
                .with_color(main_color)
                .finish(),
            );
            col.add_child(
                Text::new_inline(
                    detail.clone(),
                    appearance.ui_font_family(),
                    (appearance.monospace_font_size() - 1.).max(10.),
                )
                .with_color(detail_color)
                .finish(),
            );
            col.finish()
        }
        ToolCardState::Error {
            title,
            denied,
            detail,
        } => {
            use warpui::elements::{Highlight, HighlightedRange};
            use warpui::text_layout::TextStyle;

            // 主文本:denied 时打 STRIKETHROUGH,error 不打但走 sub 色 + detail 副行。
            let text_color = theme.sub_text_color(header_background).into_solid();
            let mut text_widget = Text::new_inline(
                title.clone(),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(text_color);

            if *denied {
                let strike_style = TextStyle::new()
                    .with_show_strikethrough(true)
                    .with_foreground_color(text_color);
                let highlight = Highlight::default().with_text_style(strike_style);
                let len = title.chars().count();
                text_widget = text_widget.with_highlights(vec![HighlightedRange {
                    highlight,
                    highlight_indices: (0..len).collect(),
                }]);
            }

            // detail 行:有就 column 拼接;没有就只一行。
            if let Some(detail_text) = detail {
                let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
                col.add_child(text_widget.finish());
                let detail_color = theme.ui_error_color();
                col.add_child(
                    Text::new_inline(
                        detail_text.clone(),
                        appearance.ui_font_family(),
                        (appearance.monospace_font_size() - 1.).max(10.),
                    )
                    .with_color(detail_color)
                    .finish(),
                );
                col.finish()
            } else {
                text_widget.finish()
            }
        }
    }
}
