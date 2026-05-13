use crate::{
    auth::UserUid,
    cloud_object::{ServerObjectContainer, StoredObjectGuest},
    drive::sharing::{LinkSharingSubjectType, SharingAccessLevel, Subject, TeamKind, UserKind},
    server::ids::ServerId,
};

#[test]
fn test_roundtrip_guests() {
    let guests = vec![
        StoredObjectGuest {
            subject: Subject::User(UserKind::Account(UserUid::new("firebase_uid"))),
            access_level: SharingAccessLevel::Edit,
            source: None,
        },
        StoredObjectGuest {
            subject: Subject::PendingUser {
                email: Some("pending@warp.dev".to_string()),
            },
            access_level: SharingAccessLevel::View,
            source: Some(ServerObjectContainer::Folder {
                folder_uid: 123.into(),
            }),
        },
        StoredObjectGuest {
            subject: Subject::Team(TeamKind::Team {
                team_uid: ServerId::from(99),
            }),
            access_level: SharingAccessLevel::Edit,
            source: None,
        },
    ];

    let encoded = super::encode_guests(&guests).expect("encode should succeed");
    let decoded = super::decode_guests(&encoded).expect("decode should succeed");

    assert_eq!(guests, decoded);
}

#[test]
fn test_fail_unsupported_subjects() {
    let result = super::encode_guests(&[StoredObjectGuest {
        subject: Subject::AnyoneWithLink(LinkSharingSubjectType::Anyone),
        access_level: SharingAccessLevel::View,
        source: None,
    }]);
    assert!(result.is_err());
}
