use super::*;

use crate::server::ids::ClientId;

#[test]
fn compose_request_has_no_auto_submit_payload() {
    let request = CloudLaunchRequest::compose();

    assert_eq!(request.prompt(), None);
    assert_eq!(request.submit_mode, CloudLaunchSubmitMode::Compose);
    assert_eq!(request.explicit_environment_id, None);
    assert_eq!(request.attachments.request_attachments.len(), 0);
    assert_eq!(request.attachments.display_attachments.len(), 0);
}

#[test]
fn auto_submit_request_carries_prompt_attachments_and_environment() {
    let environment_id = SyncId::ClientId(ClientId::new());
    let attachments = CloudLaunchAttachments {
        request_attachments: vec![AttachmentInput {
            file_name: "context.txt".to_owned(),
            mime_type: "text/plain".to_owned(),
            data: "hello".to_owned(),
        }],
        display_attachments: vec![],
    };

    let request =
        CloudLaunchRequest::auto_submit("fix tests".to_owned(), attachments, Some(environment_id));

    assert_eq!(request.prompt(), Some("fix tests"));
    assert_eq!(request.submit_mode, CloudLaunchSubmitMode::AutoSubmit);
    assert_eq!(request.explicit_environment_id, Some(environment_id));
    assert_eq!(request.attachments.request_attachments.len(), 1);
    assert_eq!(
        request.attachments.request_attachments[0].file_name,
        "context.txt"
    );
}
