use std::{borrow::Cow, str::FromStr};

use serde::{Deserialize, Serialize};
use warp_core::ui::appearance::Appearance;
use warpui::{
    color::ColorU,
    ui_components::components::{UiComponent, UiComponentStyles},
    AppContext, SingletonEntity,
};

use crate::{
    auth::UserUid,
    cloud_object::{model::persistence::ObjectStoreModel, Owner},
    server::ids::ServerId,
    ui_components::{
        avatar::{Avatar, AvatarContent},
        icons::Icon,
    },
    workspaces::{user_profiles::UserProfiles, user_workspaces::UserWorkspaces},
};

// OpenWarp Phase 2a: `dialog/` (cloud sharing modal UI) deleted along with
// all consumer triggers. `style.rs` is retained because the Subject /
// UserKind avatar helpers in this module still depend on it.
mod style;

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SharingAccessLevel {
    View,
    Edit,
    Full,
}

impl SharingAccessLevel {
    pub fn label(&self) -> &'static str {
        match self {
            SharingAccessLevel::View => "Can view",
            SharingAccessLevel::Edit => "Can edit",
            SharingAccessLevel::Full => "Full access",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SharingAccessLevel::View => "view",
            SharingAccessLevel::Edit => "edit",
            SharingAccessLevel::Full => "access",
        }
    }

    pub fn can_trash(self) -> bool {
        self >= SharingAccessLevel::Edit
    }

    pub fn can_delete(self) -> bool {
        self >= SharingAccessLevel::Full
    }

    pub fn can_move_drive(self) -> bool {
        self >= SharingAccessLevel::Full
    }

    pub fn can_edit_access(self) -> bool {
        self >= SharingAccessLevel::Full
    }

    pub fn to_serializable_value(self) -> &'static str {
        match self {
            SharingAccessLevel::View => "VIEW",
            SharingAccessLevel::Edit => "EDIT",
            SharingAccessLevel::Full => "FULL",
        }
    }
}

impl FromStr for SharingAccessLevel {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "VIEW" => Ok(Self::View),
            "EDIT" => Ok(Self::Edit),
            "FULL" => Ok(Self::Full),
            _ => Err(anyhow::anyhow!("unknown access level {value}")),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LinkSharingSubjectType {
    None,
    Anyone,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    User(UserKind),
    #[allow(dead_code)]
    PendingUser {
        email: Option<String>,
    },
    Team(TeamKind),
    AnyoneWithLink(LinkSharingSubjectType),
}

#[derive(Debug, Clone)]
pub enum UserKind {
    Account(UserUid),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TeamKind {
    Team { team_uid: ServerId },
}

impl TeamKind {
    pub fn team_uid(&self) -> ServerId {
        match self {
            TeamKind::Team { team_uid } => *team_uid,
        }
    }
}

impl Subject {
    pub fn from_owner(owner: Owner) -> Self {
        match owner {
            Owner::User { user_uid } => Subject::User(UserKind::Account(user_uid)),
            Owner::Team { team_uid } => Subject::Team(TeamKind::Team { team_uid }),
        }
    }

    pub fn user_uid(&self) -> Option<UserUid> {
        match self {
            Subject::User(user_kind) => match user_kind {
                UserKind::Account(user_uid) => Some(*user_uid),
            },
            Subject::PendingUser { .. } | Subject::Team(_) | Subject::AnyoneWithLink(_) => None,
        }
    }

    pub fn is_user(&self, other_uid: UserUid) -> bool {
        match self {
            Subject::User(UserKind::Account(user_uid)) => *user_uid == other_uid,
            Subject::PendingUser { .. } | Subject::Team(_) | Subject::AnyoneWithLink(_) => false,
        }
    }

    pub fn team_uid(&self) -> Option<ServerId> {
        match self {
            Subject::Team(team_kind) => Some(team_kind.team_uid()),
            Subject::User(_) | Subject::PendingUser { .. } | Subject::AnyoneWithLink(_) => None,
        }
    }
}

impl PartialEq for UserKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Account(self_uid), Self::Account(other_uid)) => self_uid == other_uid,
        }
    }
}

/// Identifier for an object that's shareable via the Warp Drive ACL model. Not all sharing in Warp
/// is _currently_ tied into this model (e.g. block sharing).
#[derive(Debug, Clone)]
pub enum ShareableObject {
    /// A shareable Warp Drive object.
    WarpDriveObject(ServerId),
}

impl ShareableObject {
    /// The canonical link to this object.
    pub fn link(&self, app: &AppContext) -> Option<String> {
        match self {
            ShareableObject::WarpDriveObject(id) => ObjectStoreModel::as_ref(app)
                .get_by_uid(&id.uid())
                .and_then(|object| object.object_link()),
        }
    }
}

/// Whether not a shared object's contents are editable by the current user.
///
/// This is not purely a function of their access level since anonymous users are not allowed to
/// edit (due to the lack of attribution).
#[derive(Debug, Clone, Copy)]
pub enum ContentEditability {
    ReadOnly,
    RequiresLogin,
    Editable,
}

impl ContentEditability {
    pub fn can_edit(self) -> bool {
        matches!(self, ContentEditability::Editable)
    }
}

/// Extension trait for Subject with methods that require AppContext.
pub trait SubjectExt {
    /// The name of this subject.
    fn name(&self, app: &AppContext) -> Option<Cow<'static, str>>;
    /// Detail text to display under this subject's name.
    fn detail(&self, app: &AppContext) -> Option<String>;
    /// Avatar component to show for this subject.
    fn avatar(&self, appearance: &Appearance, app: &AppContext) -> Avatar;
    /// Gets the email address for this subject, if it has one.
    fn email<'a>(&'a self, app: &'a AppContext) -> Option<&'a str>;
    /// Checks if this subject refers to the same user as an email address.
    fn matches_email(&self, email: &str, app: &AppContext) -> bool;
}

impl SubjectExt for Subject {
    fn name(&self, app: &AppContext) -> Option<Cow<'static, str>> {
        match self {
            Subject::User(kind) => kind.name(app),
            Subject::PendingUser { email } => email.clone().map(Cow::from),
            Subject::Team(kind) => kind.display_name(app).map(Cow::from),
            Subject::AnyoneWithLink(_) => Some(Cow::from("Anyone with the link")),
        }
    }

    fn detail(&self, app: &AppContext) -> Option<String> {
        if let Subject::User(kind) = self {
            kind.detail(app)
        } else {
            None
        }
    }

    fn avatar(&self, appearance: &Appearance, app: &AppContext) -> Avatar {
        match self {
            Subject::User(kind) => named_subject_avatar(kind.avatar_content(app), appearance),
            Subject::PendingUser { email } => named_subject_avatar(
                AvatarContent::DisplayName(email.clone().unwrap_or_default()),
                appearance,
            ),
            Subject::Team(_) => icon_avatar(Icon::Users, appearance),
            Subject::AnyoneWithLink(subject_type) => {
                let icon = match subject_type {
                    LinkSharingSubjectType::Anyone => Icon::Globe,
                    LinkSharingSubjectType::None => Icon::Lock,
                };
                icon_avatar(icon, appearance)
            }
        }
    }

    fn email<'a>(&'a self, app: &'a AppContext) -> Option<&'a str> {
        match self {
            Subject::User(user_kind) => match user_kind {
                UserKind::Account(user_uid) => UserProfiles::as_ref(app)
                    .profile_for_uid(*user_uid)
                    .map(|profile| profile.email.as_str()),
            },
            Subject::PendingUser { email } => email.as_deref(),
            Subject::Team(_) => None,
            Subject::AnyoneWithLink(_) => None,
        }
    }

    fn matches_email(&self, email: &str, app: &AppContext) -> bool {
        self.email(app)
            .is_some_and(|subject_email| subject_email == email)
    }
}

/// Extension trait for UserKind with methods that require AppContext.
pub trait UserKindExt {
    /// Gets the display name for this user kind.
    fn name(&self, app: &AppContext) -> Option<Cow<'static, str>>;
    /// Detail text to display under this user's name.
    fn detail(&self, app: &AppContext) -> Option<String>;
    /// Avatar content for this user kind.
    fn avatar_content(&self, app: &AppContext) -> AvatarContent;
}

impl UserKindExt for UserKind {
    fn name(&self, app: &AppContext) -> Option<Cow<'static, str>> {
        match self {
            UserKind::Account(id) => UserProfiles::as_ref(app)
                .displayable_identifier_for_uid(*id)
                .map(Cow::from),
        }
    }

    fn detail(&self, app: &AppContext) -> Option<String> {
        match self {
            UserKind::Account(uid) => {
                let profile = UserProfiles::as_ref(app).profile_for_uid(*uid)?;
                // Only show the user's email if we're already showing a display name.
                if profile.display_name.is_some() {
                    Some(profile.email.clone())
                } else {
                    None
                }
            }
        }
    }

    fn avatar_content(&self, app: &AppContext) -> AvatarContent {
        match self {
            UserKind::Account(uid) => match UserProfiles::as_ref(app).profile_for_uid(*uid) {
                Some(profile) => AvatarContent::Image {
                    url: profile.photo_url.clone(),
                    display_name: profile.displayable_identifier(),
                },
                None => AvatarContent::DisplayName(String::new()),
            },
        }
    }
}

/// Extension trait for TeamKind with methods that require AppContext.
pub trait TeamKindExt {
    /// Gets the display name for this team kind.
    fn display_name(&self, app: &AppContext) -> Option<String>;
}

impl TeamKindExt for TeamKind {
    fn display_name(&self, app: &AppContext) -> Option<String> {
        match self {
            TeamKind::Team { team_uid, .. } => UserWorkspaces::as_ref(app)
                .team_from_uid(*team_uid)
                .map(|team| team.name.clone()),
        }
    }
}

/// Helper to build an [Avatar] that shows a named subject.
fn named_subject_avatar(content: AvatarContent, appearance: &Appearance) -> Avatar {
    Avatar::new(
        content,
        UiComponentStyles {
            // TODO: Apply session-sharing color logic.
            background: Some(ColorU::new(93, 202, 60, 255).into()),
            font_color: Some(ColorU::black()),
            ..Default::default()
        },
    )
    .with_style(style::subject_avatar_styles(appearance))
}

/// Helper to build an [Avatar] that shows a subject icon.
fn icon_avatar(icon: Icon, appearance: &Appearance) -> Avatar {
    Avatar::new(
        AvatarContent::Icon(icon),
        UiComponentStyles {
            font_color: Some(style::acl_secondary_text_color(appearance)),
            ..Default::default()
        },
    )
    .with_style(style::subject_avatar_styles(appearance))
}
