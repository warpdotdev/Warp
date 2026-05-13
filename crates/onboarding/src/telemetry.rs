use serde::{Deserialize, Serialize};

/// Telemetry events for the onboarding flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OnboardingEvent {
    /// The onboarding flow was started.
    OnboardingStarted,
    /// A specific slide was viewed.
    SlideViewed { slide_name: String },
    /// A setting was changed during onboarding.
    SettingChanged { setting: String, value: String },
    /// The onboarding slides were completed.
    OnboardingSlidesCompleted {
        intention: String,
        model: Option<String>,
        autonomy: Option<String>,
        has_project_path: bool,
    },
    /// The user clicked the "Get Started" button.
    GetStartedClicked,
    /// The user started folder selection.
    FolderSelectionStarted,
    /// The user selected a folder.
    FolderSelected,
    /// A callout was displayed.
    CalloutDisplayed { callout: String },
    /// The user clicked next on a callout.
    CalloutNext,
    /// The user completed the callout flow.
    CalloutCompleted { completion_type: String },
    /// The user navigated to the next slide.
    SlideNavigatedNext,
    /// The user navigated to the previous slide.
    SlideNavigatedBack,
    /// The user clicked the "Log in" link on the welcome/intro slide.
    WelcomeLoginClicked,
}
