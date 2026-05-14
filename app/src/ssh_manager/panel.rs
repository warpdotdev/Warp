//! SSH 管理器主 panel — 左侧 Tool Panel 内容:树形列表 + 工具条 + 右键菜单
//! + 文件夹内联重命名。
//!
//! UX 规则:
//! - **单击 server**:直接连接(打开 terminal pane 跑 ssh)。要编辑用右键。
//! - **单击 folder**:仅选中(高亮);编辑名走右键 "重命名" 或新建后立刻输入。
//! - **新建文件夹后立即进入重命名态**(Drive 风格)。
//! - 右键 server:编辑 / 连接 / 删除
//! - 右键 folder:新建文件夹 / 新建服务器 / 重命名 / 删除
//! - 右键空白:新建文件夹 / 新建服务器
//!
//! 视觉打磨参考 `app/src/drive/index.rs` 的常量(ITEM_FONT_SIZE=14 / 缩进 16 /
//! 行 padding 4×8)。

use std::collections::HashMap;

use pathfinder_geometry::vector::Vector2F;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    AcceptedByDropTarget, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Dismiss, Draggable, DraggableState, DropTarget, DropTargetData, Element,
    Empty, Flex, Hoverable, MainAxisSize, MouseStateHandle, OffsetPositioning, ParentAnchor,
    ParentElement, ParentOffsetBounds, Radius, SavePosition, Stack, Text,
};
use warpui::platform::Cursor;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    AppContext, Entity, FocusContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use warp_ssh_manager::{
    AuthType, KeychainSecretStore, NodeKind, SecretKind, SshNode, SshRepository, SshSecretStore,
    SshServerInfo,
};

use crate::editor::{
    EditorView, Event as EditorEvent, SingleLineEditorOptions, TextColors, TextOptions,
};
use crate::ssh_manager::{SshTreeChangedEvent, SshTreeChangedNotifier};

// ---- 视觉常量(参考 Drive) ----
const ITEM_FONT_SIZE: f32 = 14.0;
const TOOLBAR_BUTTON_SIZE: f32 = 26.0;
const TOOLBAR_ICON_SIZE: f32 = 14.0;
const ITEM_PADDING_VERTICAL: f32 = 5.0;
const ITEM_PADDING_HORIZONTAL: f32 = 8.0;
const ITEM_ICON_TEXT_SPACING: f32 = 8.0;
const ITEM_MARGIN_BOTTOM: f32 = 2.0;
const ITEM_ICON_SIZE: f32 = 14.0;
const FOLDER_DEPTH_INDENT: f32 = 16.0;
const PANEL_HORIZONTAL_PADDING: f32 = 8.0;

const CONTEXT_MENU_WIDTH: f32 = 200.0;
const CONTEXT_MENU_ITEM_PADDING_V: f32 = 7.0;
const CONTEXT_MENU_ITEM_PADDING_H: f32 = 12.0;
const MAX_CONTEXT_MENU_ITEMS: usize = 4;
const SSH_PANEL_POSITION_ID: &str = "ssh_manager_panel_root";

#[derive(Clone, Debug)]
pub enum SshManagerPanelAction {
    AddFolder,
    AddServer,
    DeleteSelected,
    Connect,
    Edit,
    /// 单击行,处理逻辑根据 node 种类:
    /// - server: 选中 + emit OpenSshTerminal(直接连接)
    /// - folder: 仅选中
    Click(String),
    StartRename(String),
    CommitRename,
    CancelRename,
    OpenContextMenu {
        target: Option<String>,
        position: Vector2F,
    },
    DismissContextMenu,
    /// 拖拽完成 → 把 `node_id` 移到 `new_parent_id` 下(None = root)。
    MoveNode {
        node_id: String,
        new_parent_id: Option<String>,
    },
    /// 折叠/展开单个 folder。Server 节点忽略。
    ToggleNodeCollapsed(String),
    /// 顶部按钮:智能切换 — 任何 folder 还展开 → 全收;否则全展。
    ToggleAllFolders,
    /// 双击 server 行 = 连接(开新 tab)。Folder 双击 = 两次 toggle 抵消 no-op。
    DoubleClick(String),
}

#[derive(Clone, Debug)]
pub enum SshManagerPanelEvent {
    /// 用户右键 "编辑" 选了个 server,中央 pane 应打开/聚焦该 server 的编辑
    /// (`Workspace::open_ssh_server`)。
    OpenServerEditor {
        node_id: String,
    },
    /// 用户单击 server 或右键 "连接",请求开 terminal pane 跑 ssh +
    /// SecretInjector。
    OpenSshTerminal {
        node_id: String,
        server: SshServerInfo,
    },
    PersistenceError(String),
}

struct RenameState {
    node_id: String,
    editor: ViewHandle<EditorView>,
}

/// 拖拽落点 metadata。`parent_id = None` 表示拖到 panel 空白处(放回 root);
/// `Some(folder_id)` 表示拖进该文件夹;**不允许**直接拖到 server 上(server
/// 不能有 children)— 这种情况 drop_data 解释为"拖到 server 的兄弟位置",即
/// `parent_id = server.parent_id`,在 dispatch action 时已展开。
#[derive(Debug, Clone)]
struct SshDropData {
    parent_id: Option<String>,
}

impl DropTargetData for SshDropData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct SshManagerPanel {
    nodes: Vec<SshNode>,
    depths: HashMap<String, usize>,
    selected_id: Option<String>,

    add_folder_btn: MouseStateHandle,
    add_server_btn: MouseStateHandle,
    toggle_all_btn: MouseStateHandle,
    row_states: HashMap<String, MouseStateHandle>,
    /// 每行的 DraggableState — 跨渲染保持拖拽进度,所以必须 cache 在 view state。
    row_drag_states: HashMap<String, DraggableState>,

    context_menu_position: Option<Vector2F>,
    context_menu_target: Option<String>,
    context_menu_item_states: Vec<MouseStateHandle>,

    /// 当前正在重命名的节点(编辑器 + node_id)。
    rename_state: Option<RenameState>,
}

impl SshManagerPanel {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let mut me = Self {
            nodes: Vec::new(),
            depths: HashMap::new(),
            selected_id: None,
            add_folder_btn: MouseStateHandle::default(),
            add_server_btn: MouseStateHandle::default(),
            toggle_all_btn: MouseStateHandle::default(),
            row_states: HashMap::new(),
            row_drag_states: HashMap::new(),
            context_menu_position: None,
            context_menu_target: None,
            context_menu_item_states: (0..MAX_CONTEXT_MENU_ITEMS)
                .map(|_| MouseStateHandle::default())
                .collect(),
            rename_state: None,
        };
        me.refresh_tree(ctx);

        ctx.subscribe_to_model(
            &SshTreeChangedNotifier::handle(ctx),
            |me, _, event, ctx| match event {
                SshTreeChangedEvent::TreeChanged => me.refresh_tree(ctx),
            },
        );

        me
    }

    fn refresh_tree(&mut self, ctx: &mut ViewContext<Self>) {
        match warp_ssh_manager::with_conn(|c| Ok(SshRepository::list_nodes(c)?)) {
            Ok(nodes) => {
                self.depths = compute_depths(&nodes);
                self.nodes = sort_for_display(nodes, &self.depths);
                if let Some(id) = self.selected_id.clone() {
                    if !self.nodes.iter().any(|n| n.id == id) {
                        self.selected_id = None;
                    }
                }
                // 重命名中的节点若被外部删除,清掉 rename_state
                if let Some(rs) = self.rename_state.as_ref() {
                    if !self.nodes.iter().any(|n| n.id == rs.node_id) {
                        self.rename_state = None;
                    }
                }
            }
            Err(e) => {
                log::error!("ssh_manager: failed to load tree: {e:?}");
                ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            }
        }

        let active_ids: std::collections::HashSet<&str> =
            self.nodes.iter().map(|n| n.id.as_str()).collect();
        self.row_states
            .retain(|k, _| active_ids.contains(k.as_str()));
        self.row_drag_states
            .retain(|k, _| active_ids.contains(k.as_str()));
        for n in &self.nodes {
            self.row_states.entry(n.id.clone()).or_default();
            self.row_drag_states.entry(n.id.clone()).or_default();
        }

        ctx.notify();
    }

    fn on_add_folder(&mut self, ctx: &mut ViewContext<Self>) {
        let parent = self.parent_for_new_node();
        let result = warp_ssh_manager::with_conn(|c| {
            let name = unique_name(c, parent.as_deref(), "New folder")?;
            Ok(SshRepository::create_folder(c, parent.as_deref(), &name)?)
        });
        match result {
            Ok(node) => {
                let new_id = node.id.clone();
                self.selected_id = Some(new_id.clone());
                self.refresh_tree(ctx);
                // 新建即重命名 — Drive 习惯。
                self.enter_rename(new_id, ctx);
            }
            Err(e) => {
                log::error!("ssh_manager: create folder failed: {e:?}");
                ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            }
        }
    }

    fn on_add_server(&mut self, ctx: &mut ViewContext<Self>) {
        let parent = self.parent_for_new_node();
        let info_template = SshServerInfo {
            node_id: String::new(),
            host: "example.com".into(),
            port: 22,
            username: "user".into(),
            auth_type: AuthType::Password,
            key_path: None,
            last_connected_at: None,
        };
        let result = warp_ssh_manager::with_conn(|c| {
            let name = unique_name(c, parent.as_deref(), "New server")?;
            Ok(SshRepository::create_server(
                c,
                parent.as_deref(),
                &name,
                &info_template,
            )?)
        });
        match result {
            Ok(node) => {
                let new_id = node.id.clone();
                self.selected_id = Some(new_id.clone());
                self.refresh_tree(ctx);
                // 服务器新建后打开中央编辑 pane(用户填字段)— 名字编辑跟字段
                // 一起在那里改,不在树里内联编辑。
                ctx.emit(SshManagerPanelEvent::OpenServerEditor { node_id: new_id });
            }
            Err(e) => {
                log::error!("ssh_manager: create server failed: {e:?}");
                ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            }
        }
    }

    fn on_delete_selected(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        let result = warp_ssh_manager::with_conn(|c| Ok(SshRepository::delete_node(c, &id)?));
        if let Err(e) = result {
            log::error!("ssh_manager: delete failed: {e:?}");
            ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            return;
        }
        let store = KeychainSecretStore;
        let _ = store.delete(&id, SecretKind::Password);
        let _ = store.delete(&id, SecretKind::Passphrase);

        self.selected_id = None;
        self.refresh_tree(ctx);
        SshTreeChangedNotifier::handle(ctx).update(ctx, |_, ctx| {
            ctx.emit(SshTreeChangedEvent::TreeChanged);
        });
    }

    fn on_connect(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        self.dispatch_connect_for(&id, ctx);
    }

    fn dispatch_connect_for(&self, id: &str, ctx: &mut ViewContext<Self>) {
        let kind = self.nodes.iter().find(|n| n.id == id).map(|n| n.kind);
        if !matches!(kind, Some(NodeKind::Server)) {
            return;
        }
        let server = warp_ssh_manager::with_conn(|c| Ok(SshRepository::get_server(c, id)?))
            .ok()
            .flatten();
        if let Some(server) = server {
            ctx.emit(SshManagerPanelEvent::OpenSshTerminal {
                node_id: id.to_string(),
                server,
            });
        }
    }

    fn on_edit(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        let kind = self.nodes.iter().find(|n| n.id == id).map(|n| n.kind);
        if !matches!(kind, Some(NodeKind::Server)) {
            // folder 的 "编辑" = 重命名
            self.enter_rename(id, ctx);
            return;
        }
        ctx.emit(SshManagerPanelEvent::OpenServerEditor { node_id: id });
    }

    /// 双击 server = 连接(开新 tab)。Folder 双击 = 两次 toggle 相互抵消,no-op。
    fn on_double_click(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let kind = self.nodes.iter().find(|n| n.id == id).map(|n| n.kind);
        if matches!(kind, Some(NodeKind::Server)) {
            self.dispatch_connect_for(&id, ctx);
        }
    }

    /// 切换单个 folder 的折叠状态;server 节点忽略。
    fn on_toggle_node_collapsed(&mut self, node_id: &str, ctx: &mut ViewContext<Self>) {
        let kind = self.nodes.iter().find(|n| n.id == node_id).map(|n| n.kind);
        if !matches!(kind, Some(NodeKind::Folder)) {
            return;
        }
        let new_collapsed = !self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.is_collapsed)
            .unwrap_or(false);
        let id = node_id.to_string();
        let result = warp_ssh_manager::with_conn(move |c| {
            Ok(SshRepository::set_collapsed(c, &id, new_collapsed)?)
        });
        if let Err(e) = result {
            log::error!("ssh_manager: toggle collapse failed: {e:?}");
            ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            return;
        }
        self.refresh_tree(ctx);
        SshTreeChangedNotifier::handle(ctx).update(ctx, |_, ctx| {
            ctx.emit(SshTreeChangedEvent::TreeChanged);
        });
    }

    /// 顶部按钮:任何 folder 当前是展开 → 全部折叠;全部都已折叠 → 全部展开。
    fn on_toggle_all_folders(&mut self, ctx: &mut ViewContext<Self>) {
        let any_expanded = self
            .nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::Folder) && !n.is_collapsed);
        let new_collapsed = any_expanded; // 至少一个展开 → 全收;否则全展
        let result = warp_ssh_manager::with_conn(|c| {
            Ok(SshRepository::set_all_folders_collapsed(c, new_collapsed)?)
        });
        if let Err(e) = result {
            log::error!("ssh_manager: toggle all failed: {e:?}");
            ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            return;
        }
        self.refresh_tree(ctx);
        SshTreeChangedNotifier::handle(ctx).update(ctx, |_, ctx| {
            ctx.emit(SshTreeChangedEvent::TreeChanged);
        });
    }

    /// 节点是否在视觉上可见 — 任一祖先 folder 是 collapsed 就隐藏。
    /// root-level 节点永远可见。
    fn is_visible(&self, node: &SshNode) -> bool {
        let mut cursor = node.parent_id.as_deref();
        while let Some(pid) = cursor {
            let parent = match self.nodes.iter().find(|n| n.id == pid) {
                Some(p) => p,
                None => return true, // 数据不一致,保险起见显示
            };
            if matches!(parent.kind, NodeKind::Folder) && parent.is_collapsed {
                return false;
            }
            cursor = parent.parent_id.as_deref();
        }
        true
    }

    fn on_click(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        // 点击其他行 = 退出当前重命名(commit)
        if self
            .rename_state
            .as_ref()
            .map(|rs| rs.node_id != id)
            .unwrap_or(false)
        {
            self.commit_rename(ctx);
        }

        self.selected_id = Some(id.clone());
        let kind = self.nodes.iter().find(|n| n.id == id).map(|n| n.kind);
        match kind {
            Some(NodeKind::Server) => {
                // 单击 server = 仅选中。**连接走双击**(`on_double_click`)。
            }
            Some(NodeKind::Folder) => {
                // 单击 folder = 折叠/展开切换(选中已经在上面做了)
                self.on_toggle_node_collapsed(&id, ctx);
                return; // on_toggle 内部已 ctx.notify
            }
            None => {}
        }
        ctx.notify();
    }

    fn on_open_context_menu(
        &mut self,
        target: Option<String>,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        // 打开菜单前关掉 rename(否则重命名 buffer 会丢)。
        if self.rename_state.is_some() {
            self.commit_rename(ctx);
        }
        if let Some(t) = target.as_ref() {
            self.selected_id = Some(t.clone());
        }
        self.context_menu_target = target;
        self.context_menu_position = Some(position);
        ctx.notify();
    }

    fn on_dismiss_context_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.context_menu_position = None;
        self.context_menu_target = None;
        ctx.notify();
    }

    fn enter_rename(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        let current_name = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.name.clone())
            .unwrap_or_default();

        let editor = ctx.add_typed_action_view(move |ctx| {
            let appearance = warp_core::ui::appearance::Appearance::as_ref(ctx);
            let theme = appearance.theme();
            let options = SingleLineEditorOptions {
                is_password: false,
                text: TextOptions {
                    font_size_override: Some(ITEM_FONT_SIZE),
                    font_family_override: Some(appearance.ui_font_family()),
                    text_colors_override: Some(TextColors {
                        default_color: theme.active_ui_text_color(),
                        disabled_color: theme.disabled_ui_text_color(),
                        hint_color: theme.disabled_ui_text_color(),
                    }),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut editor = EditorView::single_line(options, ctx);
            editor.set_buffer_text(&current_name, ctx);
            editor
        });

        // 监听 Enter / Blurred → commit;Escape → cancel。
        ctx.subscribe_to_view(&editor, |me, _, event, ctx| match event {
            EditorEvent::Enter => me.commit_rename(ctx),
            EditorEvent::Blurred => me.commit_rename(ctx),
            EditorEvent::Escape => me.cancel_rename(ctx),
            _ => {}
        });

        ctx.focus(&editor);
        self.rename_state = Some(RenameState { node_id, editor });
        ctx.notify();
    }

    fn commit_rename(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(rs) = self.rename_state.take() else {
            return;
        };
        let new_name = rs.editor.as_ref(ctx).buffer_text(ctx).trim().to_string();
        if new_name.is_empty() {
            // 名字不能为空:撤销
            ctx.notify();
            return;
        }
        let id = rs.node_id.clone();
        let result =
            warp_ssh_manager::with_conn(|c| Ok(SshRepository::rename_node(c, &id, &new_name)?));
        if let Err(e) = result {
            log::error!("ssh_manager: rename failed: {e:?}");
            ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            return;
        }
        self.refresh_tree(ctx);
        SshTreeChangedNotifier::handle(ctx).update(ctx, |_, ctx| {
            ctx.emit(SshTreeChangedEvent::TreeChanged);
        });
    }

    fn cancel_rename(&mut self, ctx: &mut ViewContext<Self>) {
        self.rename_state = None;
        ctx.notify();
    }

    /// 检查把 `dragged` 移到 `new_parent` 是否会产生环(`new_parent` 是
    /// `dragged` 的子孙 / 自己 / 已经是当前 parent 也直接 reject 省一次写)。
    fn move_is_legal(&self, dragged: &str, new_parent: Option<&str>) -> bool {
        // 移到自身下:禁止
        if Some(dragged) == new_parent {
            return false;
        }
        // 不动:reject(避免 idempotent 写)
        let current_parent = self
            .nodes
            .iter()
            .find(|n| n.id == dragged)
            .and_then(|n| n.parent_id.as_deref());
        if current_parent == new_parent {
            return false;
        }
        // 把 folder 移到自己的子孙下:禁止(环)
        if let Some(target_parent) = new_parent {
            let mut cursor = Some(target_parent);
            while let Some(id) = cursor {
                if id == dragged {
                    return false;
                }
                cursor = self
                    .nodes
                    .iter()
                    .find(|n| n.id == id)
                    .and_then(|n| n.parent_id.as_deref());
            }
        }
        true
    }

    fn on_move_node(
        &mut self,
        node_id: String,
        new_parent_id: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.move_is_legal(&node_id, new_parent_id.as_deref()) {
            // 升级到 warn:用户拖拽看不到效果时这条日志比 debug 更易找。
            // 大多数 false 来自"拖到当前所在 parent / 拖到自己"。
            let current_parent = self
                .nodes
                .iter()
                .find(|n| n.id == node_id)
                .and_then(|n| n.parent_id.clone());
            log::warn!(
                "ssh_manager: move rejected. node={node_id} current_parent={current_parent:?} target_parent={new_parent_id:?}"
            );
            return;
        }
        // sort_order 取目标 parent 当前最大值 +1(排在末尾)。简化的方式:
        // 用 i32::MAX/2 让 SQL 层把它放最后(后续 normalize)。这里走 SQL
        // 查询拿真实 next_sort_order。
        let result = warp_ssh_manager::with_conn(|c| {
            use diesel::prelude::*;
            use persistence::schema::ssh_nodes;
            let max: Option<i32> = match new_parent_id.as_deref() {
                Some(p) => ssh_nodes::table
                    .filter(ssh_nodes::parent_id.eq(p))
                    .select(diesel::dsl::max(ssh_nodes::sort_order))
                    .first(c)?,
                None => ssh_nodes::table
                    .filter(ssh_nodes::parent_id.is_null())
                    .select(diesel::dsl::max(ssh_nodes::sort_order))
                    .first(c)?,
            };
            let next_sort = max.unwrap_or(-1) + 1;
            Ok(SshRepository::move_node(
                c,
                &node_id,
                new_parent_id.as_deref(),
                next_sort,
            )?)
        });
        if let Err(e) = result {
            log::error!("ssh_manager: move failed: {e:?}");
            ctx.emit(SshManagerPanelEvent::PersistenceError(e.to_string()));
            return;
        }
        self.refresh_tree(ctx);
        SshTreeChangedNotifier::handle(ctx).update(ctx, |_, ctx| {
            ctx.emit(SshTreeChangedEvent::TreeChanged);
        });
    }

    fn parent_for_new_node(&self) -> Option<String> {
        let id = self.selected_id.as_ref()?;
        let node = self.nodes.iter().find(|n| &n.id == id)?;
        match node.kind {
            NodeKind::Folder => Some(node.id.clone()),
            NodeKind::Server => node.parent_id.clone(),
        }
    }

    fn render_toolbar(
        &self,
        appearance: &warp_core::ui::appearance::Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let icon_color = theme.sub_text_color(theme.background());

        let make_btn = |icon: crate::ui_components::icons::Icon,
                        state: MouseStateHandle,
                        action: SshManagerPanelAction|
         -> Box<dyn Element> {
            let icon_el = ConstrainedBox::new(icon.to_warpui_icon(icon_color).finish())
                .with_width(TOOLBAR_ICON_SIZE)
                .with_height(TOOLBAR_ICON_SIZE)
                .finish();
            Hoverable::new(state, move |_| {
                Container::new(
                    ConstrainedBox::new(icon_el)
                        .with_width(TOOLBAR_BUTTON_SIZE)
                        .with_height(TOOLBAR_BUTTON_SIZE)
                        .finish(),
                )
                .with_uniform_padding(2.0)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
            })
            .finish()
        };

        // 左侧组:新建按钮
        let left_group = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(4.0)
            .with_child(make_btn(
                crate::ui_components::icons::Icon::Folder,
                self.add_folder_btn.clone(),
                SshManagerPanelAction::AddFolder,
            ))
            .with_child(make_btn(
                crate::ui_components::icons::Icon::Plus,
                self.add_server_btn.clone(),
                SshManagerPanelAction::AddServer,
            ))
            .with_main_axis_size(MainAxisSize::Min)
            .finish();

        // 右侧:折叠/展开全部按钮 — 智能切换。任一 folder 当前展开 → 显示
        // ChevronUp(意思是"折起"),否则显示 ChevronDown(意思是"展开")。
        let any_expanded = self
            .nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::Folder) && !n.is_collapsed);
        let toggle_icon = if any_expanded {
            crate::ui_components::icons::Icon::ChevronUp
        } else {
            crate::ui_components::icons::Icon::ChevronDown
        };
        let right_group = make_btn(
            toggle_icon,
            self.toggle_all_btn.clone(),
            SshManagerPanelAction::ToggleAllFolders,
        );

        // 整条 toolbar:左右两端对齐(MainAxisAlignment::SpaceBetween)。
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(warpui::elements::MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(left_group)
            .with_child(right_group)
            .finish()
    }

    fn render_tree(&self, appearance: &warp_core::ui::appearance::Appearance) -> Box<dyn Element> {
        let mut col = Flex::column();

        if self.nodes.is_empty() {
            let theme = appearance.theme();
            let muted = theme.sub_text_color(theme.background());
            col.add_child(
                Container::new(
                    Text::new_inline(
                        crate::t!("workspace-left-panel-ssh-manager-tree-empty"),
                        appearance.ui_font_family(),
                        ITEM_FONT_SIZE,
                    )
                    .with_color(muted.into())
                    .finish(),
                )
                .with_padding_top(20.0)
                .with_padding_bottom(20.0)
                .with_padding_left(ITEM_PADDING_HORIZONTAL)
                .with_padding_right(ITEM_PADDING_HORIZONTAL)
                .finish(),
            );
        } else {
            for node in &self.nodes {
                if !self.is_visible(node) {
                    continue;
                }
                col.add_child(self.render_row(node, appearance));
            }
        }
        let inner = col
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .finish();
        // 空白处右键 = 节点 None 的 OpenContextMenu。
        let hoverable = Hoverable::new(MouseStateHandle::default(), move |_| inner)
            .on_right_click(|ctx, _, position| {
                let offset = match ctx.element_position_by_id(SSH_PANEL_POSITION_ID) {
                    Some(bounds) => position - bounds.origin(),
                    None => position,
                };
                ctx.dispatch_typed_action(SshManagerPanelAction::OpenContextMenu {
                    target: None,
                    position: offset,
                });
            })
            .finish();
        // 整个 tree 区域也是个 drop target,parent_id=None 表示拖到 root。
        // 行级 DropTarget 优先级高(更小),所以拖到 folder 上还是会进 folder。
        DropTarget::new(hoverable, SshDropData { parent_id: None }).finish()
    }

    fn render_row(
        &self,
        node: &SshNode,
        appearance: &warp_core::ui::appearance::Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let depth = self.depths.get(&node.id).copied().unwrap_or(0);
        let is_selected = self.selected_id.as_deref() == Some(node.id.as_str());
        let is_renaming = self
            .rename_state
            .as_ref()
            .map(|rs| rs.node_id == node.id)
            .unwrap_or(false);

        let icon = match node.kind {
            NodeKind::Folder => crate::ui_components::icons::Icon::Folder,
            NodeKind::Server => crate::ui_components::icons::Icon::Key,
        };
        let icon_color = theme.sub_text_color(theme.background());
        let icon_el = ConstrainedBox::new(icon.to_warpui_icon(icon_color).finish())
            .with_width(ITEM_ICON_SIZE)
            .with_height(ITEM_ICON_SIZE)
            .finish();

        // Folder 行前面加 chevron(▼ 展开 / ▶ 折叠);Server 行用等宽空白占位
        // 让所有行的图标对齐。
        let chevron_el: Box<dyn Element> = match node.kind {
            NodeKind::Folder => {
                let chevron_icon = if node.is_collapsed {
                    crate::ui_components::icons::Icon::ChevronRight
                } else {
                    crate::ui_components::icons::Icon::ChevronDown
                };
                ConstrainedBox::new(chevron_icon.to_warpui_icon(icon_color).finish())
                    .with_width(ITEM_ICON_SIZE)
                    .with_height(ITEM_ICON_SIZE)
                    .finish()
            }
            NodeKind::Server => ConstrainedBox::new(Empty::new().finish())
                .with_width(ITEM_ICON_SIZE)
                .finish(),
        };

        // 右半 — 文本或重命名输入框。
        // EditorView 必须在有限宽度容器里渲染,否则 element.rs:1670 会
        // panic("infinite width constraint on buffer elements")。Flex::row 的 child
        // 没有 column-stretch 语义,所以这里包 ConstrainedBox 给个固定宽度。
        let label_or_editor: Box<dyn Element> = if is_renaming {
            let editor_handle = self
                .rename_state
                .as_ref()
                .map(|rs| rs.editor.clone())
                .expect("is_renaming implies rename_state.is_some");
            let input = appearance
                .ui_builder()
                .text_input(editor_handle)
                .with_style(UiComponentStyles {
                    padding: Some(Coords {
                        left: 4.0,
                        right: 4.0,
                        top: 1.0,
                        bottom: 1.0,
                    }),
                    background: Some(theme.surface_2().into()),
                    border_color: Some(theme.accent().into()),
                    border_width: Some(1.0),
                    border_radius: Some(CornerRadius::with_all(Radius::Pixels(3.0))),
                    font_size: Some(ITEM_FONT_SIZE),
                    ..Default::default()
                })
                .build()
                .finish();
            ConstrainedBox::new(input).with_width(180.0).finish()
        } else {
            Text::new_inline(
                node.name.clone(),
                appearance.ui_font_family(),
                ITEM_FONT_SIZE,
            )
            .with_color(theme.main_text_color(theme.background()).into())
            .finish()
        };

        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(ITEM_ICON_TEXT_SPACING)
            .with_child(
                ConstrainedBox::new(Empty::new().finish())
                    .with_width(depth as f32 * FOLDER_DEPTH_INDENT)
                    .finish(),
            )
            .with_child(chevron_el)
            .with_child(icon_el)
            .with_child(label_or_editor)
            .with_main_axis_size(MainAxisSize::Min)
            .finish();

        let state = self.row_states.get(&node.id).cloned().unwrap_or_default();
        let id_for_click = node.id.clone();
        let id_for_double_click = node.id.clone();
        let id_for_right_click = node.id.clone();

        // 重命名时不接收点击/右键(交给 EditorView)。
        if is_renaming {
            return Container::new(row)
                .with_padding_top(ITEM_PADDING_VERTICAL)
                .with_padding_bottom(ITEM_PADDING_VERTICAL)
                .with_padding_left(ITEM_PADDING_HORIZONTAL)
                .with_padding_right(ITEM_PADDING_HORIZONTAL)
                .with_margin_bottom(ITEM_MARGIN_BOTTOM)
                .finish();
        }

        let hoverable = Hoverable::new(state, move |_| {
            let mut c = Container::new(row)
                .with_padding_top(ITEM_PADDING_VERTICAL)
                .with_padding_bottom(ITEM_PADDING_VERTICAL)
                .with_padding_left(ITEM_PADDING_HORIZONTAL)
                .with_padding_right(ITEM_PADDING_HORIZONTAL)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)));
            if is_selected {
                c = c.with_background(internal_colors::fg_overlay_3(theme));
            }
            c.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(SshManagerPanelAction::Click(id_for_click.clone()));
        })
        .on_double_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(SshManagerPanelAction::DoubleClick(
                id_for_double_click.clone(),
            ));
        })
        .on_right_click(move |ctx, _, position| {
            let offset = match ctx.element_position_by_id(SSH_PANEL_POSITION_ID) {
                Some(bounds) => position - bounds.origin(),
                None => position,
            };
            ctx.dispatch_typed_action(SshManagerPanelAction::OpenContextMenu {
                target: Some(id_for_right_click.clone()),
                position: offset,
            });
        })
        .finish();

        // 把 row 包成"既可拖也接受 drop"的元素。
        //
        // **关键嵌套**:`DropTarget(Container(Draggable(Hoverable)))`。
        // 没有 Container 层会出 bug —— `Draggable::origin()` 返回 `child.origin()`
        // (`crates/warpui_core/src/elements/drag/draggable.rs:746-757`),而
        // child 在 Dragging 状态被 paint 到 drag_origin,导致 child.origin() =
        // ghost 位置。结果 DropTarget 直接套 Draggable 时,bounds 跟着 ghost
        // 跑 → drop target 永远在鼠标下,落不到别的行。Container.origin/size
        // 在自己的 paint 里锁定 layout 值(`container.rs:288 self.origin = ...`),
        // 给 DropTarget 提供稳定 bounds。
        let drag_state = self
            .row_drag_states
            .get(&node.id)
            .cloned()
            .unwrap_or_default();
        let dragged_id = node.id.clone();
        let draggable = Draggable::new(drag_state, hoverable)
            .with_accepted_by_drop_target_fn(move |drop_data, _app| {
                if drop_data.as_any().downcast_ref::<SshDropData>().is_some() {
                    AcceptedByDropTarget::Yes
                } else {
                    AcceptedByDropTarget::No
                }
            })
            .on_drop(move |ctx, _app, _bounds, data| {
                if let Some(drop) = data.and_then(|d| d.as_any().downcast_ref::<SshDropData>()) {
                    ctx.dispatch_typed_action(SshManagerPanelAction::MoveNode {
                        node_id: dragged_id.clone(),
                        new_parent_id: drop.parent_id.clone(),
                    });
                }
            })
            .finish();

        // 中间锁定 layout 原点的 Container — 看上面注释。
        let stable_anchor = Container::new(draggable).finish();

        let drop_parent_id = match node.kind {
            NodeKind::Folder => Some(node.id.clone()),
            NodeKind::Server => node.parent_id.clone(),
        };
        DropTarget::new(
            stable_anchor,
            SshDropData {
                parent_id: drop_parent_id,
            },
        )
        .finish()
    }

    fn context_menu_items(&self) -> Vec<(String, SshManagerPanelAction)> {
        match self.context_menu_target.as_ref() {
            None => vec![
                (
                    crate::t!("workspace-left-panel-ssh-manager-menu-new-folder"),
                    SshManagerPanelAction::AddFolder,
                ),
                (
                    crate::t!("workspace-left-panel-ssh-manager-menu-new-server"),
                    SshManagerPanelAction::AddServer,
                ),
            ],
            Some(id) => {
                let kind = self.nodes.iter().find(|n| &n.id == id).map(|n| n.kind);
                match kind {
                    Some(NodeKind::Folder) => vec![
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-new-folder"),
                            SshManagerPanelAction::AddFolder,
                        ),
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-new-server"),
                            SshManagerPanelAction::AddServer,
                        ),
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-rename"),
                            SshManagerPanelAction::StartRename(id.clone()),
                        ),
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-delete"),
                            SshManagerPanelAction::DeleteSelected,
                        ),
                    ],
                    Some(NodeKind::Server) => vec![
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-edit"),
                            SshManagerPanelAction::Edit,
                        ),
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-connect"),
                            SshManagerPanelAction::Connect,
                        ),
                        (
                            crate::t!("workspace-left-panel-ssh-manager-menu-delete"),
                            SshManagerPanelAction::DeleteSelected,
                        ),
                    ],
                    None => vec![],
                }
            }
        }
    }

    fn render_context_menu(
        &self,
        appearance: &warp_core::ui::appearance::Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let items = self.context_menu_items();
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for (i, (label, action)) in items.into_iter().enumerate() {
            let state = self
                .context_menu_item_states
                .get(i)
                .cloned()
                .unwrap_or_default();
            let label_el = Text::new_inline(label, appearance.ui_font_family(), ITEM_FONT_SIZE)
                .with_color(theme.main_text_color(theme.background()).into())
                .finish();
            let row_action = action.clone();
            let item = Hoverable::new(state, move |mouse| {
                let mut c = Container::new(label_el)
                    .with_padding_top(CONTEXT_MENU_ITEM_PADDING_V)
                    .with_padding_bottom(CONTEXT_MENU_ITEM_PADDING_V)
                    .with_padding_left(CONTEXT_MENU_ITEM_PADDING_H)
                    .with_padding_right(CONTEXT_MENU_ITEM_PADDING_H)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.0)));
                if mouse.is_hovered() {
                    c = c.with_background(internal_colors::fg_overlay_3(theme));
                }
                c.finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(row_action.clone());
                ctx.dispatch_typed_action(SshManagerPanelAction::DismissContextMenu);
            })
            .finish();
            col.add_child(item);
        }
        let menu_inner = ConstrainedBox::new(
            Container::new(col.with_main_axis_size(MainAxisSize::Min).finish())
                .with_background(theme.surface_2())
                .with_border(Border::all(1.0).with_border_color(theme.surface_3().into()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .with_uniform_padding(4.0)
                .finish(),
        )
        .with_width(CONTEXT_MENU_WIDTH)
        .finish();

        Dismiss::new(menu_inner)
            .on_dismiss(|ctx, _| {
                ctx.dispatch_typed_action(SshManagerPanelAction::DismissContextMenu);
            })
            .finish()
    }
}

impl Entity for SshManagerPanel {
    type Event = SshManagerPanelEvent;
}

impl TypedActionView for SshManagerPanel {
    type Action = SshManagerPanelAction;

    fn handle_action(&mut self, action: &SshManagerPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            SshManagerPanelAction::AddFolder => self.on_add_folder(ctx),
            SshManagerPanelAction::AddServer => self.on_add_server(ctx),
            SshManagerPanelAction::DeleteSelected => self.on_delete_selected(ctx),
            SshManagerPanelAction::Connect => self.on_connect(ctx),
            SshManagerPanelAction::Edit => self.on_edit(ctx),
            SshManagerPanelAction::Click(id) => self.on_click(id.clone(), ctx),
            SshManagerPanelAction::StartRename(id) => self.enter_rename(id.clone(), ctx),
            SshManagerPanelAction::CommitRename => self.commit_rename(ctx),
            SshManagerPanelAction::CancelRename => self.cancel_rename(ctx),
            SshManagerPanelAction::OpenContextMenu { target, position } => {
                self.on_open_context_menu(target.clone(), *position, ctx)
            }
            SshManagerPanelAction::DismissContextMenu => self.on_dismiss_context_menu(ctx),
            SshManagerPanelAction::MoveNode {
                node_id,
                new_parent_id,
            } => self.on_move_node(node_id.clone(), new_parent_id.clone(), ctx),
            SshManagerPanelAction::ToggleNodeCollapsed(id) => {
                self.on_toggle_node_collapsed(id, ctx)
            }
            SshManagerPanelAction::ToggleAllFolders => self.on_toggle_all_folders(ctx),
            SshManagerPanelAction::DoubleClick(id) => self.on_double_click(id.clone(), ctx),
        }
    }
}

impl View for SshManagerPanel {
    fn ui_name() -> &'static str {
        "SshManagerPanel"
    }

    fn on_focus(&mut self, _focus_ctx: &FocusContext, _ctx: &mut ViewContext<Self>) {}

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = warp_core::ui::appearance::Appearance::as_ref(app);

        let toolbar = Container::new(self.render_toolbar(appearance))
            .with_uniform_padding(8.0)
            .finish();

        let tree = Container::new(self.render_tree(appearance))
            .with_padding_left(PANEL_HORIZONTAL_PADDING - ITEM_PADDING_HORIZONTAL)
            .with_padding_right(PANEL_HORIZONTAL_PADDING - ITEM_PADDING_HORIZONTAL)
            .finish();

        // 让 tree 占满剩余垂直空间 — 这样 root DropTarget 覆盖到 panel 底部,
        // 用户在树最底下空白处拖也能落到 root(`SshDropData{parent_id:None}`)。
        let tree_filled = warpui::elements::Shrinkable::new(1.0, tree).finish();

        let panel_content = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(toolbar)
                .with_child(tree_filled)
                .finish(),
        )
        .finish();

        let positioned_panel = SavePosition::new(panel_content, SSH_PANEL_POSITION_ID).finish();

        let Some(position) = self.context_menu_position else {
            return positioned_panel;
        };

        let menu_el = self.render_context_menu(appearance);
        let positioning = OffsetPositioning::offset_from_parent(
            position,
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        );

        let mut stack = Stack::new();
        stack.add_child(positioned_panel);
        stack.add_positioned_overlay_child(menu_el, positioning);
        stack.finish()
    }
}

// --- helpers --------------------------------------------------------------

fn sort_for_display(nodes: Vec<SshNode>, depths: &HashMap<String, usize>) -> Vec<SshNode> {
    use std::collections::BTreeMap;
    let mut by_parent: BTreeMap<Option<String>, Vec<SshNode>> = BTreeMap::new();
    for n in nodes {
        by_parent.entry(n.parent_id.clone()).or_default().push(n);
    }
    for v in by_parent.values_mut() {
        v.sort_by_key(|n| (n.sort_order, n.name.clone()));
    }
    let mut out = Vec::with_capacity(depths.len());
    fn walk(
        parent: Option<&String>,
        by_parent: &BTreeMap<Option<String>, Vec<SshNode>>,
        out: &mut Vec<SshNode>,
    ) {
        if let Some(children) = by_parent.get(&parent.cloned()) {
            for c in children {
                out.push(c.clone());
                walk(Some(&c.id), by_parent, out);
            }
        }
    }
    walk(None, &by_parent, &mut out);
    out
}

fn compute_depths(nodes: &[SshNode]) -> HashMap<String, usize> {
    let by_id: HashMap<&str, &SshNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut depths = HashMap::with_capacity(nodes.len());
    for n in nodes {
        let mut d = 0;
        let mut p = n.parent_id.as_deref();
        while let Some(pid) = p {
            d += 1;
            p = by_id.get(pid).and_then(|nn| nn.parent_id.as_deref());
            if d > 64 {
                break;
            }
        }
        depths.insert(n.id.clone(), d);
    }
    depths
}

fn unique_name(
    conn: &mut diesel::sqlite::SqliteConnection,
    parent: Option<&str>,
    base: &str,
) -> Result<String, anyhow::Error> {
    use diesel::prelude::*;
    use persistence::schema::ssh_nodes;
    let existing: Vec<String> = match parent {
        Some(p) => ssh_nodes::table
            .filter(ssh_nodes::parent_id.eq(p))
            .select(ssh_nodes::name)
            .load(conn)?,
        None => ssh_nodes::table
            .filter(ssh_nodes::parent_id.is_null())
            .select(ssh_nodes::name)
            .load(conn)?,
    };
    let set: std::collections::HashSet<String> = existing.into_iter().collect();
    if !set.contains(base) {
        return Ok(base.to_string());
    }
    for i in 2..1000 {
        let cand = format!("{base} {i}");
        if !set.contains(&cand) {
            return Ok(cand);
        }
    }
    Ok(format!("{base} {}", uuid::Uuid::new_v4()))
}
