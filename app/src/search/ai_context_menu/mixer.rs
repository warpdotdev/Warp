use crate::cloud_object::ObjectType;
use crate::code_review::diff_state::DiffMode;
use crate::search::mixer::SearchMixer;

pub type AIContextMenuMixer = SearchMixer<AIContextMenuSearchableAction>;

#[derive(Debug, Clone, PartialEq)]
pub enum AIContextMenuSearchableAction {
    InsertFilePath {
        /// This is the file path relative to the root of the current git
        /// repository. If this changes, this could break how we resolve
        /// the file path outside of AI mode, so just note the downstream
        /// dependencies.
        file_path: String,
    },
    InsertText {
        /// Text to insert into the input buffer.
        text: String,
    },
    InsertDriveObject {
        /// Drive 对象类型(Workflow、Notebook 等)。
        object_type: ObjectType,
        /// 要附加的 Drive 对象 UID。
        object_uid: String,
        /// Agent Mode 输入框中展示的 @名称。
        display_name: String,
    },
    InsertPlan {
        /// 要附加的 AI 文档 UID。
        ai_document_uid: String,
        /// Agent Mode 输入框中展示的 @名称。
        display_name: String,
    },
    InsertDiffSet {
        /// The diff mode indicating what base to compare against
        diff_mode: DiffMode,
    },
    InsertConversation {
        /// 要附加的 conversation 标识。
        conversation_id: String,
        /// Agent Mode 输入框中展示的 @标题。
        title: String,
    },
    InsertSkill {
        /// The skill name to insert as /{name} into the buffer.
        name: String,
    },
}
