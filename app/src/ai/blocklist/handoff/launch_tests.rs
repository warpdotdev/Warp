use super::*;

use crate::server::ids::ClientId;

#[test]
fn auto_submit_request_carries_prompt_attachments_and_environment() {
    let environment_id = SyncId::ClientId(ClientId::new());
    let attachments = HandoffLaunchAttachments {
        request_attachments: vec![AttachmentInput {
            file_name: "context.txt".to_owned(),
            mime_type: "text/plain".to_owned(),
            data: "hello".to_owned(),
        }],
        display_attachments: vec![],
    };

    let request = HandoffLaunchRequest::auto_submit(
        "fix tests".to_owned(),
        attachments,
        Some(environment_id),
    );

    assert_eq!(request.initial_prompt.as_deref(), Some("fix tests"));
    assert_eq!(request.explicit_environment_id, Some(environment_id));
    assert_eq!(request.attachments.request_attachments.len(), 1);
    assert_eq!(
        request.attachments.request_attachments[0].file_name,
        "context.txt"
    );
}
