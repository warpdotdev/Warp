pub mod generate_metadata_for_command;
// OpenWarp Wave1-2:`give_up_notebook_edit_access` / `grab_notebook_edit_access` /
// `leave_object` / `record_object_action` / `remove_object_guest` 5 个 mutation
// 已随云对象 RPC client 下线,文件一并物理删除。
//
// OpenWarp Wave 2-1:再删 21 个 cloud-object mutation —
// `add_object_guests` / `bulk_create_objects` / `create_folder` / `create_generic_string_object`
// / `create_notebook` / `create_workflow` / `delete_object` / `empty_trash` / `move_object`
// / `remove_object_link_permissions` / `set_object_link_permissions`
// / `transfer_generic_string_object_owner` / `transfer_notebook_owner` / `transfer_workflow_owner`
// / `trash_object` / `untrash_object` / `update_folder` / `update_generic_string_object`
// / `update_notebook` / `update_object_guests` / `update_workflow` —
// 云对象 RPC client 已物理删除,不再调任何 GraphQL 路径。
//
// OpenWarp Wave 2-2:再删 5 个 AI mutation —
// `confirm_file_artifact_upload` / `create_file_artifact_upload_target`
// / `delete_ai_conversation` / `generate_dialogue` / `request_bonus`
// (`provideNegativeFeedbackResponseForAiConversation`) — 唯一消费方
// 旧云端 AI RPC 已下线。
// `generate_metadata_for_command` 有复用类型被
// `app/src/drive/workflows/ai_assist.rs` import,保留 operation 文件。
//
// OpenWarp Wave 3-1:再删 4 个 auth-only mutation —
// `create_anonymous_user` / `expire_api_key` / `generate_api_key` /
// `mint_custom_token` — 唯一消费方
// `AuthClient impl for ServerApi` 已随 server_api/auth.rs 整文件物理删,
// 上层 AuthManager 改为本地 stub,不再发起任何云端身份请求。
//
// OpenWarp Wave 4-1:再删 4 个 managed-secrets mutation —
// `create_managed_secret` / `delete_managed_secret` / `update_managed_secret`
// / `issue_task_identity_token` — 唯一消费方 `ManagedSecretsClient impl for ServerApi`
// 已 stub(Err 或 Ok 空集合)。`issue_task_identity_token` 虽然有 BYOP AWS/GCP
// 调用点(`ai/aws_credentials.rs` 等),但实际服务端为 warp.dev OIDC issuer,
// OpenWarp 无可达服务端,链路必失败,值得物理删 GraphQL 端入口,只在 client impl
// 处统一返回 disabled 错误,callers 走错误处理路径。
