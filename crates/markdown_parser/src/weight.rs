use enum_iterator::Sequence;

/// All [`Weight`]s that are not [`Weight::Normal`] are considered custom weights.
/// Avoid importing `CustomWeight`, and prefer using [`Weight`] throughout the codebase,
/// except in cases where you want to specifically track explicit weight overrides.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Sequence)]
pub enum CustomWeight {
    Thin,
    ExtraLight,
    Light,
    Medium,
    Semibold,
    Bold,
    ExtraBold,
    Black,
}

impl CustomWeight {
    /// Returns true if the weight is bold or heavier.
    pub fn is_at_least_bold(&self) -> bool {
        matches!(self, Self::Bold | Self::ExtraBold | Self::Black)
    }

    /// We do not support nested weights at this time! The outer weight will
    /// be the only respected weight.
    pub fn merge_weights(first: Option<Self>, second: Option<Self>) -> Option<Self> {
        // We don't currently support text containing text of varying weights.
        // We will just respect the outer weight if you specify a non-Normal weight.
        first.or(second)
    }
}
