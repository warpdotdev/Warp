use crate::t;
use std::collections::HashMap;

use super::{
    settings_page::{
        render_sub_header, LocalOnlyIconState, MatchData, PageType, SettingsPageMeta,
        SettingsPageViewHandle, SettingsWidget,
    },
    SettingsSection,
};
use crate::send_telemetry_from_ctx;
use crate::{appearance::Appearance, themes};
use crate::{
    editor::EditorView, keyboard::write_custom_keybinding, util::bindings::CommandBinding,
};
use crate::{
    editor::{
        Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions, TextOptions,
    },
    keyboard::UserDefinedKeybinding,
};
use crate::{search_bar::SearchBar, settings::CloudPreferencesSettings};
use crate::{
    util::bindings::{
        filter_bindings_including_keystroke, reset_keybinding_to_default, set_custom_keybinding,
    },
    TelemetryEvent,
};
use itertools::Itertools;

use warp_core::ui::theme::color::internal_colors;
use warpui::{elements::Wrap, units::Pixels};
use warpui::{
    elements::{
        Align, Border, ClippedScrollStateHandle, ClippedScrollable, Container, CornerRadius, Empty,
        EventHandler, Fill, Flex, Hoverable, MouseState, MouseStateHandle, ParentElement, Radius,
        SavePosition, ScrollbarWidth, Shrinkable,
    },
    fonts::Weight,
    keymap::{Keystroke, Trigger},
    ui_components::components::{Coords, UiComponent, UiComponentStyles},
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};
use warpui::{
    elements::{ConstrainedBox, DispatchEventResult},
    presenter::ChildView,
};
use warpui::{
    elements::{CrossAxisAlignment, Text},
    keymap::DescriptionContext,
};

const FONT_DELTA: f32 = 2.;
const CANCEL_SAVE_BUTTONS_SPACING: f32 = 4.0;
const CLEAR_CANCEL_BUTTONS_SPACING: f32 = 8.0;
const ROW_INTERNAL_VERTICAL_PADDING: f32 = 8.0;
const ROW_LEFT_MARGIN: f32 = 20.0;
const ROW_HEIGHT: f32 = 28.;
const EDIT_BUTTONS_BORDER_RADIUS: f32 = 4.0;

pub const SEARCH_PLACEHOLDER: &str = "Search by name or by keys (ex. \"cmd d\")";
const SHORTCUT_CONFLICT_WARNING_TEXT: &str = "This shortcut conflicts with other keybinds";
const KEYBINDINGS_PAGE_SHORTCUT: &str = "workspace:toggle_keybindings_page";
const RESET_BUTTON_TEXT: &str = "Default";
const CANCEL_BUTTON_TEXT: &str = "Cancel";
const CLEAR_BUTTON_TEXT: &str = "Clear";
const SAVE_BUTTON_TEXT: &str = "Save";

/// Notifier for custom keybinding changed. Views could subscribe to this for
/// KeybindingChangedEvent.
#[derive(Default)]
pub struct KeybindingChangedNotifier {}

impl KeybindingChangedNotifier {
    pub fn new() -> Self {
        Default::default()
    }

    #[cfg(test)]
    pub fn mock() -> Self {
        Self::new()
    }
}

pub enum KeybindingChangedEvent {
    BindingChanged {
        /// Name of the keybinding that is being changed.
        binding_name: String,
        new_trigger: Option<Keystroke>,
    },
}

impl Entity for KeybindingChangedNotifier {
    type Event = KeybindingChangedEvent;
}

impl SingletonEntity for KeybindingChangedNotifier {}

#[derive(Clone, Debug)]
pub struct KeyBindingModifyingState {
    pub current_binding: Option<Keystroke>,
    pub unsaved_binding: Option<Keystroke>,
}

impl KeyBindingModifyingState {
    pub fn new(state: Option<Keystroke>) -> KeyBindingModifyingState {
        Self {
            current_binding: state.clone(),
            unsaved_binding: state,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.current_binding != self.unsaved_binding
    }
}

#[derive(Debug, Clone, Default)]
struct ConflictMap {
    map: HashMap<Keystroke, usize>,
}

impl ConflictMap {
    fn update(&mut self, old: &Option<Keystroke>, new: Option<Keystroke>) {
        if let Some(old) = old {
            if let Some(old_conflict_count) = self.map.get_mut(old) {
                *old_conflict_count = old_conflict_count.saturating_sub(1);
            }
        }

        if let Some(new) = new {
            let new_conflict_count = self.map.entry(new).or_default();
            *new_conflict_count += 1;
        }
    }

    fn has_conflict(&self, key: &Option<Keystroke>) -> bool {
        match key {
            Some(key) => self
                .map
                .get(key)
                .map(|count| *count > 1)
                .unwrap_or_default(),
            None => false,
        }
    }
}

impl FromIterator<Option<Keystroke>> for ConflictMap {
    fn from_iter<I: IntoIterator<Item = Option<Keystroke>>>(iter: I) -> Self {
        let mut map = HashMap::new();

        for binding in iter.into_iter().flatten() {
            let counter = map.entry(binding).or_default();
            *counter += 1;
        }

        ConflictMap { map }
    }
}

pub struct KeybindingsView {
    page: PageType<Self>,
    search_editor: ViewHandle<EditorView>,
    search_bar: ViewHandle<SearchBar>,
    clipped_scroll_state: ClippedScrollStateHandle,
    bindings: Option<Vec<CommandBinding>>,
    modifying_row: Option<KeyBindingModifyingState>,
    pub rows: Option<Vec<KeybindingRow>>,
    // Map between the keystroke and the number of conflicting bindings associated with the keystroke.
    // The bindings could be unsaved.
    conflict_map: ConflictMap,
}

#[derive(Debug)]
pub enum KeybindingsViewAction {
    KeybindingRowClicked(usize),
    KeystrokeDefined(usize, Keystroke),
    ResetToDefaultKeyStroke(usize),
    CancelKeyStrokeEditing(usize),
    ConfirmKeyStroke(usize),
    RemoveKeyStroke(usize),
}

#[derive(Default, Clone)]
struct RowMouseStates {
    keystroke_row_mouse_state: MouseStateHandle,
    reset_to_default_mouse_state: MouseStateHandle,
    remove_mouse_state: MouseStateHandle,
    cancel_mouse_state: MouseStateHandle,
    save_mouse_state: MouseStateHandle,
}

/// Wrapper around the CommandBinding structure that includes the styling/render-specific
/// attributes (such as MouseStateHandles)
#[derive(Clone)]
pub struct KeybindingRow {
    pub binding: CommandBinding,
    mouse_state_handles: RowMouseStates,
    editor_open: bool,
}

impl From<(Option<Vec<usize>>, &CommandBinding)> for KeybindingRow {
    fn from(orig: (Option<Vec<usize>>, &CommandBinding)) -> Self {
        Self {
            binding: orig.1.clone(),
            mouse_state_handles: Default::default(),
            editor_open: false,
        }
    }
}


fn translate_command_name<'a>(name: &'a str, app: &AppContext) -> &'a str {
    use crate::t;
    match name {
        // Navigation
        "Accept Autosuggestion" => t!(app, "Accept Autosuggestion", "接受自动建议"),
        "Accept Prompt Suggestion" => t!(app, "Accept Prompt Suggestion", "接受提示建议"),
        "Activate Next Pane" => t!(app, "Activate Next Pane", "激活下一面板"),
        "Activate Next Tab" => t!(app, "Activate Next Tab", "激活下一标签页"),
        "Activate Previous Pane" => t!(app, "Activate Previous Pane", "激活上一面板"),
        "Activate Previous Tab" => t!(app, "Activate Previous Tab", "激活上一标签页"),
        "Add Cursor Above" => t!(app, "Add Cursor Above", "在上方添加光标"),
        "Add Cursor Below" => t!(app, "Add Cursor Below", "在下方添加光标"),
        "Add Repository" => t!(app, "Add Repository", "添加仓库"),
        "Add Selection For Next Occurrence" => t!(app, "Add Selection For Next Occurrence", "选中下一个相同内容"),
        "Alternate Terminal Paste" => t!(app, "Alternate Terminal Paste", "备用终端粘贴"),
        "Ask Warp Ai" => t!(app, "Ask Warp Ai", "询问 Warp AI"),
        "Ask Warp Ai About Selection" => t!(app, "Ask Warp Ai About Selection", "询问 Warp AI 关于所选内容"),
        "Ask Warp Ai About Last Block" => t!(app, "Ask Warp Ai About Last Block", "询问 Warp AI 关于最后一个块"),
        "Attach Selected Block As Agent Context" => t!(app, "Attach Selected Block As Agent Context", "将所选块附加为代理上下文"),
        "Attach Selected Text As Agent Context" => t!(app, "Attach Selected Text As Agent Context", "将所选文本附加为代理上下文"),
        "Backward Tabulation Within An Executing Command" => t!(app, "Backward Tabulation Within An Executing Command", "在执行命令中反向制表"),
        "Bookmark Selected Block" => t!(app, "Bookmark Selected Block", "为所选块添加书签"),
        "Check For Updates" => t!(app, "Check For Updates", "检查更新"),
        "Clear Blocks" => t!(app, "Clear Blocks", "清除块"),
        "Clear And Reset Ai Context Menu Query" => t!(app, "Clear And Reset Ai Context Menu Query", "清除并重置 AI 上下文菜单查询"),
        "Clear Command Editor" => t!(app, "Clear Command Editor", "清除命令编辑器"),
        "Clear Screen" => t!(app, "Clear Screen", "清屏"),
        "Clear Selected Lines" => t!(app, "Clear Selected Lines", "清除所选行"),
        "Close" => t!(app, "Close", "关闭"),
        "Close Current Session" => t!(app, "Close Current Session", "关闭当前会话"),
        "Close All Tabs" => t!(app, "Close All Tabs", "关闭所有标签页"),
        "Close Other Tabs" => t!(app, "Close Other Tabs", "关闭其他标签页"),
        "Close Saved Tabs" => t!(app, "Close Saved Tabs", "关闭已保存标签页"),
        "Close The Current Tab" => t!(app, "Close The Current Tab", "关闭当前标签页"),
        "Close Tabs To The Right" => t!(app, "Close Tabs To The Right", "关闭右侧标签页"),
        "Close Focused Panel" => t!(app, "Close Focused Panel", "关闭聚焦面板"),
        "Close Window" => t!(app, "Close Window", "关闭窗口"),
        "Command Search" => t!(app, "Command Search", "命令搜索"),
        "Copy" => t!(app, "Copy", "复制"),
        "Copy Access Token To Clipboard" => t!(app, "Copy Access Token To Clipboard", "复制访问令牌到剪贴板"),
        "Copy And Clear Selected Lines" => t!(app, "Copy And Clear Selected Lines", "复制并清除所选行"),
        "Copy Command" => t!(app, "Copy Command", "复制命令"),
        "Copy Command And Output" => t!(app, "Copy Command And Output", "复制命令和输出"),
        "Copy Command Output" => t!(app, "Copy Command Output", "复制命令输出"),
        "Copy Git Branch" => t!(app, "Copy Git Branch", "复制 Git 分支"),
        "Copy Rich-Text Buffer" => t!(app, "Copy Rich-Text Buffer", "复制富文本缓冲区"),
        "Copy Rich-Text Selection" => t!(app, "Copy Rich-Text Selection", "复制富文本选区"),
        "Create New Project" => t!(app, "Create New Project", "创建新项目"),
        "Create New Tab" => t!(app, "Create New Tab", "创建新标签页"),
        "Create New Window" => t!(app, "Create New Window", "创建新窗口"),
        "Create Or Edit Link" => t!(app, "Create Or Edit Link", "创建或编辑链接"),
        "Cursor At Buffer End" => t!(app, "Cursor At Buffer End", "光标移至缓冲区末尾"),
        "Cursor At Buffer Start" => t!(app, "Cursor At Buffer Start", "光标移至缓冲区开头"),
        "Cut All Left" => t!(app, "Cut All Left", "剪切光标左侧全部"),
        "Cut All Right" => t!(app, "Cut All Right", "剪切光标右侧全部"),
        "Cut Word Left" => t!(app, "Cut Word Left", "剪切左侧单词"),
        "Cut Word Right" => t!(app, "Cut Word Right", "剪切右侧单词"),
        "De-Select Shell Commands" => t!(app, "De-Select Shell Commands", "取消选中 Shell 命令"),
        "Decrease Font Size" => t!(app, "Decrease Font Size", "减小字体大小"),
        "Decrease Notebook Font Size" => t!(app, "Decrease Notebook Font Size", "减小笔记本字体"),
        "Decrease Zoom Level" => t!(app, "Decrease Zoom Level", "降低缩放级别"),
        "Delete" => t!(app, "Delete", "删除"),
        "Delete All Left" => t!(app, "Delete All Left", "删除左侧全部"),
        "Delete All Right" => t!(app, "Delete All Right", "删除右侧全部"),
        "Delete To Line End Within An Executing Command" => t!(app, "Delete To Line End Within An Executing Command", "删除至行尾（执行中命令）"),
        "Delete To Line Start Within An Executing Command" => t!(app, "Delete To Line Start Within An Executing Command", "删除至行首（执行中命令）"),
        "Delete Word Left" => t!(app, "Delete Word Left", "删除左侧单词"),
        "Delete Word Left Within An Executing Command" => t!(app, "Delete Word Left Within An Executing Command", "删除左侧单词（执行中命令）"),
        "Delete Word Right" => t!(app, "Delete Word Right", "删除右侧单词"),
        "Edit Prompt" => t!(app, "Edit Prompt", "编辑提示"),
        "Exit Vim Insert Mode" => t!(app, "Exit Vim Insert Mode", "退出 Vim 插入模式"),
        "Expand Selected Blocks Above" => t!(app, "Expand Selected Blocks Above", "向上展开所选块"),
        "Expand Selected Blocks Below" => t!(app, "Expand Selected Blocks Below", "向下展开所选块"),
        "Export All Warp Drive Objects" => t!(app, "Export All Warp Drive Objects", "导出所有 Warp Drive 对象"),
        "Find In Notebook" => t!(app, "Find In Notebook", "在笔记本中查找"),
        "Find In Terminal" => t!(app, "Find In Terminal", "在终端中查找"),
        "Find In Code Editor" => t!(app, "Find In Code Editor", "在代码编辑器中查找"),
        "Find The Next Occurrence Of Your Search Query" => t!(app, "Find The Next Occurrence Of Your Search Query", "查找搜索词的下一处"),
        "Find The Previous Occurrence Of Your Search Query" => t!(app, "Find The Previous Occurrence Of Your Search Query", "查找搜索词的上一处"),
        "Find Within Selected Block" => t!(app, "Find Within Selected Block", "在所选块内查找"),
        "Focus Terminal Input From Warp Ai" => t!(app, "Focus Terminal Input From Warp Ai", "从 Warp AI 聚焦终端输入"),
        "Focus Terminal Input From File" => t!(app, "Focus Terminal Input From File", "从文件聚焦终端输入"),
        "Focus Terminal Input From Notebook" => t!(app, "Focus Terminal Input From Notebook", "从笔记本聚焦终端输入"),
        "Focus Next Match" => t!(app, "Focus Next Match", "聚焦下一匹配"),
        "Focus Previous Match" => t!(app, "Focus Previous Match", "聚焦上一匹配"),
        "Focus Terminal Input" => t!(app, "Focus Terminal Input", "聚焦终端输入"),
        "Fold" => t!(app, "Fold", "折叠"),
        "Fold Selected Ranges" => t!(app, "Fold Selected Ranges", "折叠所选范围"),
        "Go To Line" => t!(app, "Go To Line", "跳转到行"),
        "History Search" => t!(app, "History Search", "历史搜索"),
        "Home" => t!(app, "Home", "行首"),
        "Import External Settings" => t!(app, "Import External Settings", "导入外部设置"),
        "Import To Personal Drive" => t!(app, "Import To Personal Drive", "导入到个人 Drive"),
        "Import To Team Drive" => t!(app, "Import To Team Drive", "导入到团队 Drive"),
        "Increase Font Size" => t!(app, "Increase Font Size", "增大字体大小"),
        "Increase Notebook Font Size" => t!(app, "Increase Notebook Font Size", "增大笔记本字体"),
        "Increase Zoom Level" => t!(app, "Increase Zoom Level", "提高缩放级别"),
        "Insert Command Correction" => t!(app, "Insert Command Correction", "插入命令更正"),
        "Insert Last Word Of Previous Command" => t!(app, "Insert Last Word Of Previous Command", "插入上一命令的最后一个词"),
        "Insert Newline" => t!(app, "Insert Newline", "插入换行"),
        "Insert Non-Expanding Space" => t!(app, "Insert Non-Expanding Space", "插入不扩展空格"),
        "Inspect Command" => t!(app, "Inspect Command", "检查命令"),
        "Install Oz Cli Command" => t!(app, "Install Oz Cli Command", "安装 Oz CLI 命令"),
        "Install Update And Relaunch" => t!(app, "Install Update And Relaunch", "安装更新并重启"),
        "Invite People..." => t!(app, "Invite People...", "邀请成员…"),
        "Join Our Slack Community (Opens External Link)" => t!(app, "Join Our Slack Community (Opens External Link)", "加入 Slack 社区（外部链接）"),
        "Jump To Latest Agent Task" => t!(app, "Jump To Latest Agent Task", "跳转到最新代理任务"),
        "Launch Configuration Palette" => t!(app, "Launch Configuration Palette", "启动配置面板"),
        "Log Out" => t!(app, "Log Out", "退出登录"),
        "Move Backward One Subword" => t!(app, "Move Backward One Subword", "向后移动一个子词"),
        "Move Backward One Word" => t!(app, "Move Backward One Word", "向后移动一个单词"),
        "Move Forward One Subword" => t!(app, "Move Forward One Subword", "向前移动一个子词"),
        "Move Forward One Word" => t!(app, "Move Forward One Word", "向前移动一个单词"),
        "Move Cursor Down" => t!(app, "Move Cursor Down", "光标下移"),
        "Move Cursor Left" => t!(app, "Move Cursor Left", "光标左移"),
        "Move Cursor Right" => t!(app, "Move Cursor Right", "光标右移"),
        "Move Cursor Up" => t!(app, "Move Cursor Up", "光标上移"),
        "Move Cursor To The Bottom" => t!(app, "Move Cursor To The Bottom", "光标移至底部"),
        "Move Cursor To The Top" => t!(app, "Move Cursor To The Top", "光标移至顶部"),
        "Move Cursor End Within An Executing Command" => t!(app, "Move Cursor End Within An Executing Command", "光标移至行尾（执行中命令）"),
        "Move Cursor Home Within An Executing Command" => t!(app, "Move Cursor Home Within An Executing Command", "光标移至行首（执行中命令）"),
        "Move Cursor One Word To The Left Within An Executing Command" => t!(app, "Move Cursor One Word To The Left Within An Executing Command", "光标左移一词（执行中命令）"),
        "Move Cursor One Word To The Right Within An Executing Command" => t!(app, "Move Cursor One Word To The Right Within An Executing Command", "光标右移一词（执行中命令）"),
        "Move Tab Left" => t!(app, "Move Tab Left", "标签页左移"),
        "Move Tab Right" => t!(app, "Move Tab Right", "标签页右移"),
        "Move To End Of Line" => t!(app, "Move To End Of Line", "移至行尾"),
        "Move To End Of Paragraph" => t!(app, "Move To End Of Paragraph", "移至段落末尾"),
        "Move To Line End" => t!(app, "Move To Line End", "移至行尾"),
        "Move To Line Start" => t!(app, "Move To Line Start", "移至行首"),
        "Move To Start Of Line" => t!(app, "Move To Start Of Line", "移至行首"),
        "Move To The End Of The Buffer" => t!(app, "Move To The End Of The Buffer", "移至缓冲区末尾"),
        "Move To The End Of The Paragraph" => t!(app, "Move To The End Of The Paragraph", "移至段落末尾"),
        "Move To The Start Of The Buffer" => t!(app, "Move To The Start Of The Buffer", "移至缓冲区开头"),
        "Move To The Start Of The Paragraph" => t!(app, "Move To The Start Of The Paragraph", "移至段落开头"),
        "New Agent Tab" => t!(app, "New Agent Tab", "新建代理标签页"),
        "New Cloud Agent Tab" => t!(app, "New Cloud Agent Tab", "新建云代理标签页"),
        "New File" => t!(app, "New File", "新建文件"),
        "New Personal Environment Variables" => t!(app, "New Personal Environment Variables", "新建个人环境变量"),
        "New Team Environment Variables" => t!(app, "New Team Environment Variables", "新建团队环境变量"),
        "New Terminal Tab" => t!(app, "New Terminal Tab", "新建终端标签页"),
        "Open Ai Command Suggestions" => t!(app, "Open Ai Command Suggestions", "打开 AI 命令建议"),
        "Open Block Context Menu" => t!(app, "Open Block Context Menu", "打开块上下文菜单"),
        "Open Keybindings Editor" => t!(app, "Open Keybindings Editor", "打开快捷键编辑器"),
        "Open Left Panel" => t!(app, "Open Left Panel", "打开左侧面板"),
        "Open Repository" => t!(app, "Open Repository", "打开仓库"),
        "Open Settings File" => t!(app, "Open Settings File", "打开设置文件"),
        "Open Settings: Account" => t!(app, "Open Settings: Account", "打开设置：账户"),
        "Open Settings: Features" => t!(app, "Open Settings: Features", "打开设置：功能"),
        "Open Theme Picker" => t!(app, "Open Theme Picker", "打开主题选择器"),
        "Paste" => t!(app, "Paste", "粘贴"),
        "Quit Warp" => t!(app, "Quit Warp", "退出 Warp"),
        "Reinput Selected Commands" => t!(app, "Reinput Selected Commands", "重新输入所选命令"),
        "Reinput Selected Commands As Root" => t!(app, "Reinput Selected Commands As Root", "以 root 身份重新输入所选命令"),
        "Reload File" => t!(app, "Reload File", "重新加载文件"),
        "Remove The Previous Character" => t!(app, "Remove The Previous Character", "删除前一个字符"),
        "Rename The Current Tab" => t!(app, "Rename The Current Tab", "重命名当前标签页"),
        "Reopen Closed Session" => t!(app, "Reopen Closed Session", "重新打开已关闭会话"),
        "Reset Font Size To Default" => t!(app, "Reset Font Size To Default", "重置字体大小为默认值"),
        "Reset Notebook Font Size" => t!(app, "Reset Notebook Font Size", "重置笔记本字体大小"),
        "Reset Zoom Level To Default" => t!(app, "Reset Zoom Level To Default", "重置缩放级别为默认值"),
        "Resize Pane > Move Divider Down" => t!(app, "Resize Pane > Move Divider Down", "调整面板 > 分隔线下移"),
        "Resize Pane > Move Divider Left" => t!(app, "Resize Pane > Move Divider Left", "调整面板 > 分隔线左移"),
        "Resize Pane > Move Divider Right" => t!(app, "Resize Pane > Move Divider Right", "调整面板 > 分隔线右移"),
        "Resize Pane > Move Divider Up" => t!(app, "Resize Pane > Move Divider Up", "调整面板 > 分隔线上移"),
        "Restart Warp Ai" => t!(app, "Restart Warp Ai", "重启 Warp AI"),
        "Run Selected Commands" => t!(app, "Run Selected Commands", "运行所选命令"),
        "Save All Unsaved Files In Code Review" => t!(app, "Save All Unsaved Files In Code Review", "保存代码审查中所有未保存文件"),
        "Save File As" => t!(app, "Save File As", "另存为"),
        "Save New Launch Configuration" => t!(app, "Save New Launch Configuration", "保存新启动配置"),
        "Save Workflow" => t!(app, "Save Workflow", "保存工作流"),
        "Scroll Terminal Output Down One Line" => t!(app, "Scroll Terminal Output Down One Line", "终端输出下滚一行"),
        "Scroll Terminal Output Up One Line" => t!(app, "Scroll Terminal Output Up One Line", "终端输出上滚一行"),
        "Scroll To Bottom Of Selected Block" => t!(app, "Scroll To Bottom Of Selected Block", "滚动至所选块底部"),
        "Scroll To Top Of Selected Block" => t!(app, "Scroll To Top Of Selected Block", "滚动至所选块顶部"),
        "Search Warp Drive" => t!(app, "Search Warp Drive", "搜索 Warp Drive"),
        "Select All" => t!(app, "Select All", "全选"),
        "Select All Blocks" => t!(app, "Select All Blocks", "选中所有块"),
        "Select And Move To The Bottom" => t!(app, "Select And Move To The Bottom", "选中并移至底部"),
        "Select And Move To The Top" => t!(app, "Select And Move To The Top", "选中并移至顶部"),
        "Select Down" => t!(app, "Select Down", "向下选择"),
        "Select Next Command" => t!(app, "Select Next Command", "选择下一个命令"),
        "Select One Character To The Left" => t!(app, "Select One Character To The Left", "向左选择一个字符"),
        "Select One Character To The Right" => t!(app, "Select One Character To The Right", "向右选择一个字符"),
        "Select One Subword To The Left" => t!(app, "Select One Subword To The Left", "向左选择一个子词"),
        "Select One Subword To The Right" => t!(app, "Select One Subword To The Right", "向右选择一个子词"),
        "Select One Word To The Left" => t!(app, "Select One Word To The Left", "向左选择一个单词"),
        "Select One Word To The Right" => t!(app, "Select One Word To The Right", "向右选择一个单词"),
        "Select Previous Command" => t!(app, "Select Previous Command", "选择上一个命令"),
        "Select Shell Command At Cursor" => t!(app, "Select Shell Command At Cursor", "选中光标处 Shell 命令"),
        "Select The Closest Bookmark Down" => t!(app, "Select The Closest Bookmark Down", "选中向下最近书签"),
        "Select The Closest Bookmark Up" => t!(app, "Select The Closest Bookmark Up", "选中向上最近书签"),
        "Select To End Of Line" => t!(app, "Select To End Of Line", "选中至行尾"),
        "Select To End Of Paragraph" => t!(app, "Select To End Of Paragraph", "选中至段落末尾"),
        "Select To Line End" => t!(app, "Select To Line End", "选中至行尾"),
        "Select To Line Start" => t!(app, "Select To Line Start", "选中至行首"),
        "Select To Start Of Line" => t!(app, "Select To Start Of Line", "选中至行首"),
        "Select To Start Of Paragraph" => t!(app, "Select To Start Of Paragraph", "选中至段落开头"),
        "Select Up" => t!(app, "Select Up", "向上选择"),
        "Send Feedback (Opens External Link)" => t!(app, "Send Feedback (Opens External Link)", "发送反馈（外部链接）"),
        "Setup Guide" => t!(app, "Setup Guide", "设置指南"),
        "Share Current Session" => t!(app, "Share Current Session", "共享当前会话"),
        "Share Pane" => t!(app, "Share Pane", "共享面板"),
        "Share Selected Block" => t!(app, "Share Selected Block", "共享所选块"),
        "Show Warp Network Log" => t!(app, "Show Warp Network Log", "显示 Warp 网络日志"),
        "Show Find Bar In Code Review" => t!(app, "Show Find Bar In Code Review", "在代码审查中显示查找栏"),
        "Split Pane Down" => t!(app, "Split Pane Down", "向下分割面板"),
        "Split Pane Left" => t!(app, "Split Pane Left", "向左分割面板"),
        "Split Pane Right" => t!(app, "Split Pane Right", "向右分割面板"),
        "Split Pane Up" => t!(app, "Split Pane Up", "向上分割面板"),
        "Stop Synchronizing Any Panes" => t!(app, "Stop Synchronizing Any Panes", "停止同步所有面板"),
        "Stop Sharing Current Session" => t!(app, "Stop Sharing Current Session", "停止共享当前会话"),
        "Switch Focus To Left Panel" => t!(app, "Switch Focus To Left Panel", "切换焦点至左侧面板"),
        "Switch Focus To Right Panel" => t!(app, "Switch Focus To Right Panel", "切换焦点至右侧面板"),
        "Switch Panes Down" => t!(app, "Switch Panes Down", "切换到下方面板"),
        "Switch Panes Left" => t!(app, "Switch Panes Left", "切换到左侧面板"),
        "Switch Panes Right" => t!(app, "Switch Panes Right", "切换到右侧面板"),
        "Switch Panes Up" => t!(app, "Switch Panes Up", "切换到上方面板"),
        "Switch To 1St Tab" => t!(app, "Switch To 1St Tab", "切换到第 1 个标签页"),
        "Switch To 2Nd Tab" => t!(app, "Switch To 2Nd Tab", "切换到第 2 个标签页"),
        "Switch To 3Rd Tab" => t!(app, "Switch To 3Rd Tab", "切换到第 3 个标签页"),
        "Switch To 4Th Tab" => t!(app, "Switch To 4Th Tab", "切换到第 4 个标签页"),
        "Switch To 5Th Tab" => t!(app, "Switch To 5Th Tab", "切换到第 5 个标签页"),
        "Switch To 6Th Tab" => t!(app, "Switch To 6Th Tab", "切换到第 6 个标签页"),
        "Switch To 7Th Tab" => t!(app, "Switch To 7Th Tab", "切换到第 7 个标签页"),
        "Switch To 8Th Tab" => t!(app, "Switch To 8Th Tab", "切换到第 8 个标签页"),
        "Switch To Last Tab" => t!(app, "Switch To Last Tab", "切换到最后标签页"),
        "Switch To Next Tab" => t!(app, "Switch To Next Tab", "切换到下一标签页"),
        "Switch To Previous Tab" => t!(app, "Switch To Previous Tab", "切换到上一标签页"),
        "Terminal Session" => t!(app, "Terminal Session", "终端会话"),
        "Toggle Agent Conversation List View" => t!(app, "Toggle Agent Conversation List View", "切换代理对话列表视图"),
        "Toggle Case-Sensitive Search" => t!(app, "Toggle Case-Sensitive Search", "切换大小写敏感搜索"),
        "Toggle Comment" => t!(app, "Toggle Comment", "切换注释"),
        "Toggle Fullscreen" => t!(app, "Toggle Fullscreen", "切换全屏"),
        "Toggle Inline Code Styling" => t!(app, "Toggle Inline Code Styling", "切换行内代码样式"),
        "Toggle Keyboard Shortcuts" => t!(app, "Toggle Keyboard Shortcuts", "切换键盘快捷键"),
        "Toggle Maximize Active Pane" => t!(app, "Toggle Maximize Active Pane", "切换最大化活动面板"),
        "Toggle Maximize Code Review Panel" => t!(app, "Toggle Maximize Code Review Panel", "切换最大化代码审查面板"),
        "Toggle Mouse Reporting" => t!(app, "Toggle Mouse Reporting", "切换鼠标报告"),
        "Toggle Pty Recording For Session" => t!(app, "Toggle Pty Recording For Session", "切换会话 PTY 录制"),
        "Toggle Regular Expression Search" => t!(app, "Toggle Regular Expression Search", "切换正则表达式搜索"),
        "Toggle Resource Center" => t!(app, "Toggle Resource Center", "切换资源中心"),
        "Toggle Rich-Text Debug Mode" => t!(app, "Toggle Rich-Text Debug Mode", "切换富文本调试模式"),
        "Toggle Sticky Command Header" => t!(app, "Toggle Sticky Command Header", "切换固定命令标题"),
        "Toggle Sticky Command Header In Active Pane" => t!(app, "Toggle Sticky Command Header In Active Pane", "切换活动面板固定命令标题"),
        "Toggle Strikethrough Styling" => t!(app, "Toggle Strikethrough Styling", "切换删除线样式"),
        "Toggle Synchronizing All Panes In All Tabs" => t!(app, "Toggle Synchronizing All Panes In All Tabs", "切换所有标签页同步"),
        "Toggle Synchronizing All Panes In Current Tab" => t!(app, "Toggle Synchronizing All Panes In Current Tab", "切换当前标签页面板同步"),
        "Toggle Team Workflows Modal" => t!(app, "Toggle Team Workflows Modal", "切换团队工作流模态框"),
        "Toggle The Agent Management View" => t!(app, "Toggle The Agent Management View", "切换代理管理视图"),
        "Toggle Underline Styling" => t!(app, "Toggle Underline Styling", "切换下划线样式"),
        "Toggle Vertical Tabs Panel" => t!(app, "Toggle Vertical Tabs Panel", "切换垂直标签面板"),
        "Toggle Warp Ai" => t!(app, "Toggle Warp Ai", "切换 Warp AI"),
        "Toggle Warp Drive" => t!(app, "Toggle Warp Drive", "切换 Warp Drive"),
        "Toggle Code Review" => t!(app, "Toggle Code Review", "切换代码审查"),
        "Toggle Command Palette" => t!(app, "Toggle Command Palette", "切换命令面板"),
        "Toggle Files Palette" => t!(app, "Toggle Files Palette", "切换文件面板"),
        "Toggle Navigation Palette" => t!(app, "Toggle Navigation Palette", "切换导航面板"),
        "Toggle Project Explorer" => t!(app, "Toggle Project Explorer", "切换项目资源管理器"),
        "Trigger Auto Detection" => t!(app, "Trigger Auto Detection", "触发自动检测"),
        "Turn Notifications Off" => t!(app, "Turn Notifications Off", "关闭通知"),
        "Turn Notifications On" => t!(app, "Turn Notifications On", "开启通知"),
        "Unfold" => t!(app, "Unfold", "展开"),
        "Uninstall Oz Cli Command" => t!(app, "Uninstall Oz Cli Command", "卸载 Oz CLI 命令"),
        "View Warp Logs" => t!(app, "View Warp Logs", "查看 Warp 日志"),
        "View Latest Changelog" => t!(app, "View Latest Changelog", "查看最新更新日志"),
        "Warpify Ssh Session" => t!(app, "Warpify Ssh Session", "Warpify SSH 会话"),
        "Warpify Subshell" => t!(app, "Warpify Subshell", "Warpify 子 Shell"),
        "Workflows" => t!(app, "Workflows", "工作流"),
        // Settings shortcuts
        "Open Settings" => t!(app, "Open Settings", "打开设置"),
        "Open Settings: Ai" => t!(app, "Open Settings: Ai", "打开设置：AI"),
        "Open Settings: About" => t!(app, "Open Settings: About", "打开设置：关于"),
        "Open Settings: Appearance" => t!(app, "Open Settings: Appearance", "打开设置：外观"),
        "Open Settings: Billing And Usage" => t!(app, "Open Settings: Billing And Usage", "打开设置：账单与用量"),
        "Open Settings: Code" => t!(app, "Open Settings: Code", "打开设置：代码"),
        "Open Settings: Environments" => t!(app, "Open Settings: Environments", "打开设置：环境"),
        "Open Settings: Keyboard Shortcuts" => t!(app, "Open Settings: Keyboard Shortcuts", "打开设置：键盘快捷键"),
        "Open Settings: Mcp Servers" => t!(app, "Open Settings: Mcp Servers", "打开设置：MCP 服务器"),
        "Open Settings: Privacy" => t!(app, "Open Settings: Privacy", "打开设置：隐私"),
        "Open Settings: Referrals" => t!(app, "Open Settings: Referrals", "打开设置：推荐"),
        "Open Settings: Shared Blocks" => t!(app, "Open Settings: Shared Blocks", "打开设置：共享块"),
        "Open Settings: Teams" => t!(app, "Open Settings: Teams", "打开设置：团队"),
        "Open Settings: Warpify" => t!(app, "Open Settings: Warpify", "打开设置：Warpify"),
        "Open Mcp Servers" => t!(app, "Open Mcp Servers", "打开 MCP 服务器"),
        "Open Ai Rules" => t!(app, "Open Ai Rules", "打开 AI 规则"),
        "Open Global Search" => t!(app, "Open Global Search", "打开全局搜索"),
        // Additional
        "Add Current Folder As Project" => t!(app, "Add Current Folder As Project", "将当前文件夹添加为项目"),
        "Initiate Project For Warp" => t!(app, "Initiate Project For Warp", "为 Warp 初始化项目"),
        "Load Agent Mode Conversation (From Debug Link In Clipboard)" => t!(app, "Load Agent Mode Conversation (From Debug Link In Clipboard)", "从剪贴板调试链接加载代理对话"),
        _ => name,
    }
}

impl KeybindingRow {
    fn render(
        &self,
        index: usize,
        is_disabled: bool,
        has_conflicting_binding: bool,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let inner = if !is_disabled {
            let mut row = Hoverable::new(
                self.mouse_state_handles.keystroke_row_mouse_state.clone(),
                |state| {
                    let background = if state.is_hovered() {
                        Some(appearance.theme().accent().with_opacity(40).into())
                    } else if index.is_multiple_of(2) {
                        Some(internal_colors::fg_overlay_1(appearance.theme()).into())
                    } else {
                        None
                    };
                    if self.editor_open {
                        self.render_clicked(index, has_conflicting_binding, appearance, app)
                    } else {
                        self.render_summary(None, background, has_conflicting_binding, appearance, app)
                    }
                },
            );

            if !self.editor_open {
                row = row.on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(KeybindingsViewAction::KeybindingRowClicked(index));
                });
            }

            row.finish()
        } else {
            let background = if index.is_multiple_of(2) {
                Some(internal_colors::fg_overlay_1(appearance.theme()).into())
            } else {
                None
            };

            Container::new(self.render_summary(
                None,
                background,
                has_conflicting_binding,
                appearance,
                app,
            ))
            .with_foreground_overlay(appearance.theme().keybinding_row_overlay())
            .finish()
        };

        if index == 0 {
            SavePosition::new(inner, "first_keybinding_setting").finish()
        } else {
            inner
        }
    }

    fn render_summary(
        &self,
        index: Option<usize>,
        background: Option<Fill>,
        has_conflicting_binding: bool,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let binding = &self.binding;
        let keystroke = match binding.trigger.clone() {
            None => Empty::new().finish(),
            Some(keystroke) => {
                let mut keyshortcut = appearance.ui_builder().keyboard_shortcut(&keystroke);

                if has_conflicting_binding {
                    keyshortcut = keyshortcut.with_style(UiComponentStyles {
                        border_width: Some(2.),
                        border_color: Some(themes::theme::Fill::warn().into()),
                        ..Default::default()
                    });
                }

                keyshortcut.build().finish()
            }
        };
        let element = render_columns(
            render_text(
                translate_command_name(binding.description.in_context(DescriptionContext::Default), app),
                None,
                appearance,
            ),
            keystroke,
            0.7,
            background,
            None,
        );
        if let Some(index) = index {
            EventHandler::new(element)
                .on_keydown(move |ctx, _, keystroke| {
                    ctx.dispatch_typed_action(KeybindingsViewAction::KeystrokeDefined(
                        index,
                        keystroke.clone(),
                    ));
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            element
        }
    }

    fn render_clicked(
        &self,
        index: usize,
        has_conflicting_binding: bool,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let conflict_warning = if has_conflicting_binding {
            render_text(
                t!(app, "This shortcut conflicts with other keybinds", "该快捷键与其他绑定冲突"),
                Some(UiComponentStyles {
                    font_weight: Some(Weight::Bold),
                    ..Default::default()
                }),
                appearance,
            )
        } else {
            Empty::new().finish()
        };

        let press_new_shortcut_text = render_text("Press new keyboard shortcut", None, appearance);

        let new_shortcut_element = Container::new(press_new_shortcut_text)
            .with_margin_left(ROW_LEFT_MARGIN)
            .with_margin_top(8.0)
            .finish();

        Container::new(
            Flex::column()
                .with_child(self.render_summary(
                    Some(index),
                    Some(appearance.theme().accent().into()),
                    has_conflicting_binding,
                    appearance,
                    app,
                ))
                .with_child(
                    Container::new(new_shortcut_element)
                        .with_margin_bottom(ROW_INTERNAL_VERTICAL_PADDING)
                        .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_child(
                            Shrinkable::new(
                                1.,
                                Align::new(
                                    Container::new(conflict_warning)
                                        .with_margin_left(ROW_LEFT_MARGIN)
                                        .finish(),
                                )
                                .left()
                                .finish(),
                            )
                            .finish(),
                        )
                        .with_child(
                            Container::new(self.get_edit_button_row(appearance, index))
                                .with_margin_right(CLEAR_CANCEL_BUTTONS_SPACING)
                                .finish(),
                        )
                        .with_cross_axis_alignment(warpui::elements::CrossAxisAlignment::Center)
                        .finish(),
                )
                .finish(),
        )
        .with_padding_bottom(ROW_INTERNAL_VERTICAL_PADDING)
        .with_background(appearance.theme().accent().with_opacity(40))
        .finish()
    }

    fn get_button_text_color(
        &self,
        appearance: &Appearance,
        state: &MouseState,
    ) -> themes::theme::Fill {
        let main_text_color: themes::theme::Fill = appearance
            .theme()
            .main_text_color(appearance.theme().surface_2());

        if state.is_hovered() {
            main_text_color
        } else if state.is_clicked() {
            main_text_color.with_opacity(50)
        } else {
            main_text_color.with_opacity(90)
        }
    }

    fn get_edit_button_row(&self, appearance: &Appearance, index: usize) -> Box<dyn Element> {
        let mut edit_buttons_based_on_state = Vec::new();

        if self.binding.trigger.is_some() {
            let clear = Hoverable::new(
                self.mouse_state_handles.remove_mouse_state.clone(),
                |state| {
                    render_button(
                        CLEAR_BUTTON_TEXT,
                        appearance,
                        self.get_button_text_color(appearance, state),
                    )
                },
            )
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(KeybindingsViewAction::RemoveKeyStroke(index));
            })
            .finish();

            edit_buttons_based_on_state.push(clear);
        }

        let clear = Container::new(
            Hoverable::new(
                self.mouse_state_handles
                    .reset_to_default_mouse_state
                    .clone(),
                |state| {
                    render_button(
                        RESET_BUTTON_TEXT,
                        appearance,
                        self.get_button_text_color(appearance, state),
                    )
                },
            )
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(KeybindingsViewAction::ResetToDefaultKeyStroke(index));
            })
            .finish(),
        )
        .with_padding_left(CLEAR_CANCEL_BUTTONS_SPACING)
        .finish();
        edit_buttons_based_on_state.push(clear);

        let cancel = Container::new(
            Hoverable::new(
                self.mouse_state_handles.cancel_mouse_state.clone(),
                |state| {
                    let cancel_button_color = self.get_button_text_color(appearance, state);
                    if index == 0 {
                        SavePosition::new(
                            render_button(CANCEL_BUTTON_TEXT, appearance, cancel_button_color),
                            "first_keybinding_cancel",
                        )
                        .finish()
                    } else {
                        render_button("Cancel", appearance, cancel_button_color)
                    }
                },
            )
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(KeybindingsViewAction::CancelKeyStrokeEditing(index));
            })
            .finish(),
        )
        .with_padding_left(CLEAR_CANCEL_BUTTONS_SPACING)
        .finish();

        edit_buttons_based_on_state.push(cancel);

        let save = Container::new(
            Hoverable::new(self.mouse_state_handles.save_mouse_state.clone(), |state| {
                render_button(
                    SAVE_BUTTON_TEXT,
                    appearance,
                    self.get_button_text_color(appearance, state),
                )
            })
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(KeybindingsViewAction::ConfirmKeyStroke(index));
            })
            .finish(),
        )
        .with_padding_left(CANCEL_SAVE_BUTTONS_SPACING)
        .finish();
        edit_buttons_based_on_state.push(save);

        Flex::row()
            .with_children(edit_buttons_based_on_state)
            .finish()
    }
}

impl KeybindingsView {
    pub fn new(ctx: &mut ViewContext<KeybindingsView>) -> Self {
        let search_editor = {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions {
                    font_size_override: Some(appearance.ui_font_size()),
                    ..Default::default()
                },
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                ..Default::default()
            };
            ctx.add_typed_action_view(|ctx| EditorView::single_line(options, ctx))
        };
        ctx.subscribe_to_view(&search_editor, move |me, _, event, ctx| {
            me.handle_search_editor_event(event, ctx);
        });

        search_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.set_placeholder_text(SEARCH_PLACEHOLDER, ctx);
        });

        let search_bar = ctx.add_typed_action_view(|_| SearchBar::new(search_editor.clone()));

        let page = PageType::new_monolith(KeybindingsWidget::default(), None, false);
        Self {
            page,
            clipped_scroll_state: Default::default(),
            bindings: None,
            rows: Default::default(),
            modifying_row: None,
            search_bar,
            search_editor,
            conflict_map: Default::default(),
        }
    }

    /// Searches for a keybinding as if the user had typed the query into the search
    /// box. Will filter the keybinding list by the query.
    pub fn search_for_binding(&mut self, query: &str, ctx: &mut ViewContext<Self>) {
        self.search_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text(query, ctx);
        });
        self.filter_bindings(query, ctx);
    }

    /// Filter the list of visible bindings by the given query.
    fn filter_bindings(&mut self, query: &str, ctx: &mut ViewContext<Self>) {
        self.rows = Some(
            filter_bindings_including_keystroke(
                self.bindings.iter().flatten(),
                query,
                DescriptionContext::Default,
            )
            .map(KeybindingRow::from)
            .collect(),
        );

        self.clipped_scroll_state.scroll_to(Pixels::zero());
        ctx.notify();
    }

    fn handle_search_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Edited(_) => {
                let search_term = self.search_editor.as_ref(ctx).buffer_text(ctx);
                self.filter_bindings(&search_term, ctx);
            }
            EditorEvent::Enter => ctx.notify(),
            EditorEvent::Escape => ctx.focus_self(),
            _ => {}
        }
    }

    fn binding_row_clicked(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        // Unfocus the search bar.
        ctx.focus_self();

        // Only enable editing when none of the other keystrokes are being edited.
        if self.modifying_row.is_none() {
            let maybe_row = self
                .rows
                .as_mut()
                .into_iter()
                .flatten()
                .enumerate()
                .find(|(idx, _)| *idx == index);

            if let Some((_, row)) = maybe_row {
                ctx.disable_key_bindings_dispatching();
                self.modifying_row =
                    Some(KeyBindingModifyingState::new(row.binding.trigger.clone()));
                row.editor_open = true;

                // This is entering the edit mode, and we'll want to capture the keydown events.
                // For that all actions are being suppressed for the given window.
                ctx.notify();
            }
        }
    }

    fn remove_keystroke(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(row) = self.rows.as_mut().and_then(|rows| rows.get_mut(index)) {
            ctx.set_custom_trigger(row.binding.name.clone(), Trigger::Empty);

            trigger_keybinding_notifier(row.binding.name.clone(), None, ctx);

            self.conflict_map.update(&row.binding.trigger, None);

            // Persist the keybinding into the `.warp` directory so that it will last beyond
            // this session
            write_custom_keybinding(row.binding.name.clone(), UserDefinedKeybinding::Removed);
            update_binding_list(&row.binding.name, None, &mut self.bindings);
            row.binding.trigger = None;

            send_telemetry_from_ctx!(
                TelemetryEvent::KeybindingRemoved {
                    action: row.binding.name.clone(),
                },
                ctx
            );
            self.modifying_row = None;
            row.editor_open = false;
            ctx.enable_key_bindings_dispatching();
            ctx.notify();
        }
    }

    fn reset_to_default_keystroke(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(row) = self.rows.as_mut().and_then(|rows| rows.get_mut(index)) {
            let default_trigger = reset_keybinding_to_default(&row.binding.name, ctx);
            self.conflict_map
                .update(&row.binding.trigger, default_trigger.clone());
            update_binding_list(
                &row.binding.name,
                default_trigger.clone(),
                &mut self.bindings,
            );
            row.binding.trigger = default_trigger;

            send_telemetry_from_ctx!(
                TelemetryEvent::KeybindingResetToDefault {
                    action: row.binding.name.clone(),
                },
                ctx
            );

            self.modifying_row = None;
            row.editor_open = false;
            ctx.enable_key_bindings_dispatching();
            ctx.notify();
        }
    }

    fn cancel_keystroke_editing(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(row) = self.rows.as_mut().and_then(|rows| rows.get_mut(index)) {
            match self.modifying_row.take() {
                Some(keybinding_state) => {
                    self.conflict_map.update(
                        &row.binding.trigger,
                        keybinding_state.current_binding.clone(),
                    );
                    update_binding_list(
                        &row.binding.name,
                        keybinding_state.current_binding.clone(),
                        &mut self.bindings,
                    );
                    row.binding.trigger = keybinding_state.current_binding;

                    row.editor_open = false;
                    ctx.enable_key_bindings_dispatching();
                    ctx.notify();
                }
                None => {
                    log::error!("Modifying row should exist");
                }
            }
        }
    }

    fn confirm_keystroke_editing(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(row) = self.rows.as_mut().and_then(|rows| rows.get_mut(index)) {
            match self.modifying_row.take() {
                Some(keybinding_state) => {
                    if let Some(key) = keybinding_state.unsaved_binding {
                        set_custom_keybinding(&row.binding.name, &key, ctx);
                        update_binding_list(
                            &row.binding.name,
                            Some(key.clone()),
                            &mut self.bindings,
                        );
                        row.binding.trigger = Some(key.clone());
                        send_telemetry_from_ctx!(
                            TelemetryEvent::KeybindingChanged {
                                action: row.binding.name.clone(),
                                keystroke: key,
                            },
                            ctx
                        );
                    }

                    row.editor_open = false;
                    ctx.enable_key_bindings_dispatching();
                    ctx.notify();
                }
                None => {
                    log::error!("Modifying row should exist");
                }
            }
        }
    }

    fn set_temporary_keystroke_state(
        &mut self,
        index: usize,
        key: Keystroke,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(row) = self.rows.as_mut().and_then(|rows| rows.get_mut(index)) {
            match &mut self.modifying_row {
                Some(keybinding_state) => {
                    keybinding_state.unsaved_binding = Some(key.clone());
                }
                None => {
                    log::error!("Modifying row does not exist when it should");
                }
            }

            self.conflict_map
                .update(&row.binding.trigger, Some(key.clone()));
            row.binding.trigger = Some(key);
            ctx.notify();
        }
    }
}

impl Entity for KeybindingsView {
    type Event = ();
}

impl View for KeybindingsView {
    fn ui_name() -> &'static str {
        "KeybindingsView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for KeybindingsView {
    fn section() -> SettingsSection {
        SettingsSection::Keybindings
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn on_page_selected(&mut self, allow_steal_focus: bool, ctx: &mut ViewContext<Self>) {
        // Reset previous modifying_row state.
        self.modifying_row = None;
        // `from_editable_lens` materializes any dynamic description resolver
        // before caching, so the dedup below (which compares descriptions)
        // sees concrete strings.
        let lenses: Vec<_> = ctx.editable_bindings().collect();
        self.bindings = Some(
            lenses
                .into_iter()
                .map(|lens| CommandBinding::from_editable_lens(lens, ctx))
                .sorted_by(|a, b| {
                    // Sort by description then name so that we can deduplicate bindings by name.
                    a.description
                        .in_context(DescriptionContext::Default)
                        .cmp(b.description.in_context(DescriptionContext::Default))
                        .then(a.name.cmp(&b.name))
                })
                // Effectively, editable bindings can only be used by one view, because the
                // corresponding context predicate and typed action are view-specific.
                //
                // If multiple views need equivalent bindings, we handle this by declaring
                // duplicates with the same name and description, but different actions and
                // predicates. Because bindings are saved/loaded by name, changes to one binding
                // will affect the others. To reduce clutter, only show one binding for a given name
                // and description.
                //
                // There are some bindings with the same name, but different descriptions. Because
                // we sort by description first, those bindings won't be deduplicated. This is
                // alright for now, since those bindings have slightly different semantics despite
                // being linked (e.g. find in block vs. find in terminal).
                //
                // TODO: Long-term, we should instead refactor TypedActionView so that common
                // bindings can be declared once and handled by multiple views.
                .dedup_by(|a, b| a.name == b.name && a.description == b.description)
                .collect(),
        );
        self.rows = Some(
            self.bindings
                .iter()
                .flatten()
                .map(|b| (None, b))
                .map(KeybindingRow::from)
                .collect(),
        );

        // Populate the conflict map at startup.
        self.conflict_map = self
            .bindings
            .iter()
            .flatten()
            .map(|binding| binding.trigger.clone())
            .collect();

        self.search_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.set_placeholder_text(SEARCH_PLACEHOLDER, ctx);
        });

        if allow_steal_focus {
            ctx.focus(&self.search_editor);
        }
        ctx.notify();
    }

    fn on_tab_pressed(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.search_editor);
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<KeybindingsView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<KeybindingsView>) -> Self {
        SettingsPageViewHandle::Keybindings(view_handle)
    }
}

impl TypedActionView for KeybindingsView {
    type Action = KeybindingsViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        use KeybindingsViewAction::*;

        match action {
            RemoveKeyStroke(index) => self.remove_keystroke(*index, ctx),
            ResetToDefaultKeyStroke(index) => self.reset_to_default_keystroke(*index, ctx),
            CancelKeyStrokeEditing(index) => self.cancel_keystroke_editing(*index, ctx),
            ConfirmKeyStroke(index) => self.confirm_keystroke_editing(*index, ctx),
            KeybindingRowClicked(index) => self.binding_row_clicked(*index, ctx),
            KeystrokeDefined(index, key) => {
                self.set_temporary_keystroke_state(*index, key.clone(), ctx)
            }
        }
    }
}

// TODO maybe this should be turned into a table ui component?
fn render_columns(
    left: Box<dyn Element>,
    right: Box<dyn Element>,
    left_column_flex: f32,
    background: Option<Fill>,
    padding: Option<Coords>,
) -> Box<dyn Element> {
    let columns = Flex::row()
        .with_child(Shrinkable::new(left_column_flex, Align::new(left).left().finish()).finish())
        .with_child(
            Shrinkable::new(1. - left_column_flex, Align::new(right).left().finish()).finish(),
        )
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .finish();

    let mut container = Container::new(
        ConstrainedBox::new(columns)
            .with_min_height(ROW_HEIGHT)
            .finish(),
    );
    if let Some(padding) = padding {
        container = container
            .with_padding_top(padding.top)
            .with_padding_bottom(padding.bottom)
            .with_padding_right(padding.right)
            .with_padding_left(padding.left);
    } else {
        container = container
            .with_padding_top(10.)
            .with_padding_bottom(10.)
            .with_padding_right(20.)
            .with_padding_left(20.);
    };
    if let Some(background) = background {
        container.with_background(background).finish()
    } else {
        container.finish()
    }
}

fn render_button(
    text: &'static str,
    appearance: &Appearance,
    line_color: themes::theme::Fill,
) -> Box<dyn Element> {
    Container::new(
        Text::new_inline(text, appearance.ui_font_family(), appearance.ui_font_size())
            .with_color(line_color.into())
            .finish(),
    )
    .with_uniform_padding(4.0)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
        EDIT_BUTTONS_BORDER_RADIUS,
    )))
    .with_border(Border::all(1.).with_border_fill(line_color))
    .finish()
}

fn render_text(
    text: &str,
    styles: Option<UiComponentStyles>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let mut text = appearance
        .ui_builder()
        .wrappable_text(text.to_string(), true);

    if let Some(styles) = styles {
        text = text.with_style(styles);
    }

    text.build().finish()
}

/// Update the provided binding list by changing the binding with the given name to use a new
/// trigger.
fn update_binding_list(
    name: &str,
    trigger: Option<Keystroke>,
    list: &mut Option<Vec<CommandBinding>>,
) {
    let found_binding = list.as_mut().and_then(|vec| {
        vec.iter_mut()
            .find(|binding| !name.is_empty() && binding.name == name)
    });

    if let Some(binding) = found_binding {
        binding.trigger = trigger;
    }
}

fn trigger_keybinding_notifier(
    name: String,
    trigger: Option<Keystroke>,
    ctx: &mut ViewContext<KeybindingsView>,
) {
    KeybindingChangedNotifier::handle(ctx).update(ctx, move |_me, ctx| {
        ctx.emit(KeybindingChangedEvent::BindingChanged {
            binding_name: name,
            new_trigger: trigger,
        });
    })
}

#[derive(Default)]
struct KeybindingsWidget {
    local_only_icon_mouse_state: MouseStateHandle,
}

impl KeybindingsWidget {
    fn render_description(
        &self,
        bindings: Option<&Vec<CommandBinding>>,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let font_size = appearance.ui_font_size() + FONT_DELTA;
        let mut description = Flex::column().with_child(render_text(
            t!(app, "Add your own custom keybindings to existing actions below.", "在下方为现有操作添加自定义键盘绑定。"),
            Some(UiComponentStyles {
                font_size: Some(font_size),
                font_color: Some(
                    appearance
                        .theme()
                        .sub_text_color(appearance.theme().background())
                        .into_solid(),
                ),
                ..Default::default()
            }),
            appearance,
        ));

        if let Some(keystroke) = bindings
            .and_then(|bindings| {
                bindings
                    .iter()
                    .find(|&binding| binding.name == KEYBINDINGS_PAGE_SHORTCUT)
            })
            .and_then(|shortcut| shortcut.trigger.as_ref())
        {
            description = description.with_child(
                Wrap::row()
                    .with_child(
                        Container::new(render_text(
                            "Use",
                            Some(UiComponentStyles {
                                font_size: Some(font_size),
                                font_color: Some(
                                    appearance
                                        .theme()
                                        .sub_text_color(appearance.theme().background())
                                        .into_solid(),
                                ),
                                ..Default::default()
                            }),
                            appearance,
                        ))
                        .with_padding_right(10.)
                        .finish(),
                    )
                    .with_child(
                        appearance
                            .ui_builder()
                            .keyboard_shortcut(keystroke)
                            .with_style(UiComponentStyles {
                                margin: Some(Coords::default().right(5.)),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                    )
                    .with_child(
                        Container::new(render_text(
                            t!(app, "to reference these keybindings in a side pane at anytime.", "随时在侧边栏中查阅这些键盘绑定。"),
                            Some(UiComponentStyles {
                                font_size: Some(font_size),
                                font_color: Some(
                                    appearance
                                        .theme()
                                        .sub_text_color(appearance.theme().background())
                                        .into_solid(),
                                ),
                                ..Default::default()
                            }),
                            appearance,
                        ))
                        .with_padding_left(5.)
                        .finish(),
                    )
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .finish(),
            )
        }
        description.finish()
    }

    fn render_binding_list(
        &self,
        view: &KeybindingsView,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        if let Some(rows) = view.rows.as_ref() {
            let rows = Flex::column().with_children(
                rows.iter()
                    .enumerate()
                    .map(|(idx, row)| {
                        row.render(
                            idx,
                            view.modifying_row.is_some() && !row.editor_open,
                            view.conflict_map.has_conflict(&row.binding.trigger),
                            appearance,
                            app,
                        )
                    })
                    .collect::<Vec<_>>(),
            );

            return ClippedScrollable::vertical(
                view.clipped_scroll_state.clone(),
                rows.finish(),
                ScrollbarWidth::Auto,
                appearance
                    .theme()
                    .disabled_text_color(appearance.theme().background())
                    .into(),
                appearance
                    .theme()
                    .main_text_color(appearance.theme().background())
                    .into(),
                Fill::None,
            )
            .finish();
        }
        Empty::new().finish()
    }
}

impl SettingsWidget for KeybindingsWidget {
    type View = KeybindingsView;

    fn search_terms(&self) -> &str {
        "keybindings keyboard shortcuts hotkeys"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let local_only_icon_state = if *CloudPreferencesSettings::as_ref(app).settings_sync_enabled
        {
            Some(LocalOnlyIconState::Visible {
                mouse_state: self.local_only_icon_mouse_state.clone(),
                custom_tooltip: Some("Keyboard shortcuts are not synced to the cloud".to_string()),
            })
        } else {
            None
        };

        let subheader = render_sub_header(
            appearance,
            t!(app, "Configure keyboard shortcuts", "配置键盘快捷键"),
            local_only_icon_state,
        );
        let description = self.render_description(view.bindings.as_ref(), appearance, app);

        Flex::column()
            .with_child(subheader)
            .with_child(description)
            .with_child(render_columns(
                Container::new(render_text(
                    t!(app, "Command", "命令"),
                    Some(UiComponentStyles {
                        font_size: Some(appearance.ui_font_size() + FONT_DELTA),
                        ..Default::default()
                    }),
                    appearance,
                ))
                .with_uniform_margin(20.)
                .finish(),
                Container::new(ChildView::new(&view.search_bar).finish())
                    .with_margin_right(10.)
                    .finish(),
                0.62,
                None,
                Some(Coords {
                    top: 10.,
                    bottom: 0.,
                    right: 0.,
                    left: 0.,
                }),
            ))
            .with_child(Shrinkable::new(1., self.render_binding_list(view, appearance, app)).finish())
            .finish()
    }
}
