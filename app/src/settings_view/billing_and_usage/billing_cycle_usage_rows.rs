use std::cell::RefCell;
use std::collections::HashMap;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use thousands::Separable;
use warp_core::ui::appearance::Appearance;
use warpui::{
    elements::{
        Align, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
        DropShadow, Empty, Expanded, Flex, Hoverable, MainAxisAlignment, MainAxisSize,
        MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
        Radius, Shrinkable, Stack, Text,
    },
    fonts::{Properties, Weight},
    platform::Cursor,
    Element, EventContext,
};

use crate::{
    settings_view::billing_and_usage_page_v2::{
        AMBIENT_CREDITS_DOT_COLOR, BASE_CREDITS_DOT_COLOR, BONUS_CREDITS_DOT_COLOR,
        PAYG_CREDITS_DOT_COLOR,
    },
    ui_components::{blended_colors, icons::Icon},
    workspaces::workspace::{
        AiCreditsUsageAndCostSubjectType, AiCreditsUsageAndCostType, AiCreditsUsageBucket,
        AiCreditsUsageSource, BillingCycleUsageEntry, UsageVisibilityGranularity, Workspace,
        WorkspaceMember,
    },
};

// for a bunch of this (min fill ratio, cost type order, ... )
// you will find analogous ts code in warp-server
const BAR_HEIGHT: f32 = 8.;
const MIN_FILL_RATIO: f32 = 0.05;
const ROW_BORDER_RADIUS: f32 = 8.;
const ROW_BORDER_WIDTH: f32 = 1.;
/// Size of the leading icons in the row credit cluster (coin + credit-card).
const ROW_ICON_SIZE: f32 = 12.;
/// Gap between the coin/credit cluster and the cost cluster on a row. Two
/// pixels larger than the within-cluster icon-to-text gap (4px) so the
/// credits and cost read as distinct columns.
const ROW_CLUSTER_GAP: f32 = 6.;
/// Right-aligned fixed widths for the tooltip's credit and cost columns,
/// so the numbers line up vertically across rows even though each row is
/// laid out independently.
const TOOLTIP_CREDITS_COL_WIDTH: f32 = 60.;
const TOOLTIP_COST_COL_WIDTH: f32 = 64.;
/// Corner radius for elements painted directly against the card's inner
/// edge (e.g. the stacked bar's leftmost/rightmost ends). `Container`
/// paints its child *inside* the border, so the bar starts
/// `ROW_BORDER_WIDTH` pixels in from the card's outer edge. Using the
/// card's full outer radius on the bar produces a visible 1px step
/// between the bar's rounded corner and the card's rounded corner. The
/// inner radius compensates so the two curves are flush.
const BAR_CORNER_RADIUS: f32 = ROW_BORDER_RADIUS - ROW_BORDER_WIDTH;
const ROW_PADDING: f32 = 12.;
const TOOLTIP_GAP: f32 = 6.;
const COST_TYPE_ORDER: &[AiCreditsUsageAndCostType] = &[
    AiCreditsUsageAndCostType::BaseLimit,
    AiCreditsUsageAndCostType::BonusGrant,
    AiCreditsUsageAndCostType::Payg,
    AiCreditsUsageAndCostType::AmbientBonusGrant,
];
const BUCKET_ORDER: &[AiCreditsUsageBucket] = &[
    AiCreditsUsageBucket::Ai,
    AiCreditsUsageBucket::Compute,
    AiCreditsUsageBucket::Platform,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceFilter {
    #[default]
    All,
    Local,
    Cloud,
}

impl SourceFilter {
    pub fn label(self) -> &'static str {
        match self {
            SourceFilter::All => "All",
            SourceFilter::Local => "Local",
            SourceFilter::Cloud => "Cloud",
        }
    }

    fn matches(self, source: &AiCreditsUsageSource) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Local => *source == AiCreditsUsageSource::Local,
            SourceFilter::Cloud => *source == AiCreditsUsageSource::Cloud,
        }
    }
}

pub struct RowMouseStates {
    pub filter_all: MouseStateHandle,
    pub filter_local: MouseStateHandle,
    pub filter_cloud: MouseStateHandle,
    tooltip_by_subject: RefCell<HashMap<String, MouseStateHandle>>,
}

const TEAM_AGGREGATE_KEY: &str = "__team_aggregate__";
const SELF_OWN_KEY: &str = "__self_own__";

impl Default for RowMouseStates {
    fn default() -> Self {
        Self {
            filter_all: MouseStateHandle::default(),
            filter_local: MouseStateHandle::default(),
            filter_cloud: MouseStateHandle::default(),
            tooltip_by_subject: RefCell::new(HashMap::new()),
        }
    }
}

impl RowMouseStates {
    fn tooltip_mouse_state(&self, key: &str) -> MouseStateHandle {
        let mut map = self.tooltip_by_subject.borrow_mut();
        map.entry(key.to_string()).or_default().clone()
    }
}

/// One colored slice of the stacked bar. `cost_type` drives color and
/// `usage_bucket` is preserved for tooltip breakdown (no overlay striping
/// for now — that can be ported later if we want admin-panel parity).
#[derive(Clone, Debug)]
pub struct BarSegment {
    pub cost_type: AiCreditsUsageAndCostType,
    pub usage_bucket: AiCreditsUsageBucket,
    pub credits: i64,
    pub cost_cents: i64,
}

/// Aggregated usage for a single subject (or the synthetic team), ready to
/// be rendered. `subject_key` is the stable key used to look up the row's
/// tooltip mouse state on subsequent renders.
#[derive(Debug)]
pub struct MemberUsageRow {
    pub subject_type: AiCreditsUsageAndCostSubjectType,
    pub subject_key: String,
    pub display_name: String,
    pub total_credits: i64,
    pub total_cost_cents: i64,
    /// Per-user base credit limit, if the row is a workspace member with a
    /// finite base limit. `None` for service accounts, team-aggregate
    /// rows, and members marked `is_unlimited`. When `Some`, the row
    /// renders `total_credits / base_limit` instead of just
    /// `total_credits`.
    pub base_limit: Option<i64>,
    /// Segments sorted by [`COST_TYPE_ORDER`] then [`BUCKET_ORDER`], with
    /// any zero-credit entries filtered out. Used both for the stacked bar
    /// and as the source of the per-cost-type breakdown shown in the row's
    /// hover tooltip.
    pub segments: Vec<BarSegment>,
}

fn member_base_limit(member: &WorkspaceMember) -> Option<i64> {
    if member.usage_info.is_unlimited {
        None
    } else {
        Some(member.usage_info.request_limit as i64)
    }
}

/// Returns the next-cycle reset / period-end label color and label for one
/// cost-type bucket, mirroring the legend palette already used by the page.
pub fn cost_type_color(cost_type: &AiCreditsUsageAndCostType) -> ColorU {
    match cost_type {
        AiCreditsUsageAndCostType::BaseLimit => BASE_CREDITS_DOT_COLOR,
        AiCreditsUsageAndCostType::BonusGrant => BONUS_CREDITS_DOT_COLOR,
        AiCreditsUsageAndCostType::Payg => PAYG_CREDITS_DOT_COLOR,
        AiCreditsUsageAndCostType::AmbientBonusGrant => AMBIENT_CREDITS_DOT_COLOR,
        AiCreditsUsageAndCostType::Aggregate | AiCreditsUsageAndCostType::Other(_) => {
            BASE_CREDITS_DOT_COLOR
        }
    }
}

pub fn cost_type_label(cost_type: &AiCreditsUsageAndCostType) -> &'static str {
    match cost_type {
        AiCreditsUsageAndCostType::BaseLimit => "Base",
        AiCreditsUsageAndCostType::BonusGrant => "Add-ons",
        AiCreditsUsageAndCostType::Payg => "Pay-as-you-go",
        AiCreditsUsageAndCostType::AmbientBonusGrant => "Ambient-only",
        AiCreditsUsageAndCostType::Aggregate => "Total",
        AiCreditsUsageAndCostType::Other(_) => "Other",
    }
}

fn bucket_label(bucket: &AiCreditsUsageBucket) -> &'static str {
    match bucket {
        AiCreditsUsageBucket::Ai => "AI",
        AiCreditsUsageBucket::Compute => "Compute",
        AiCreditsUsageBucket::Platform => "Platform",
        AiCreditsUsageBucket::SuggestedCodeDiffs => "Suggested code diffs",
        AiCreditsUsageBucket::Voice => "Voice",
        AiCreditsUsageBucket::Aggregate => "Total",
        AiCreditsUsageBucket::Other(_) => "Other",
    }
}

fn cost_type_rank(cost_type: &AiCreditsUsageAndCostType) -> usize {
    COST_TYPE_ORDER
        .iter()
        .position(|c| c == cost_type)
        .unwrap_or(COST_TYPE_ORDER.len())
}

fn bucket_rank(bucket: &AiCreditsUsageBucket) -> usize {
    BUCKET_ORDER
        .iter()
        .position(|b| b == bucket)
        .unwrap_or(BUCKET_ORDER.len())
}

fn segment_sort_key(segment: &BarSegment) -> (usize, usize) {
    (
        cost_type_rank(&segment.cost_type),
        bucket_rank(&segment.usage_bucket),
    )
}

/// Entries we never want to surface in the rows — voice and suggested code
/// diffs are billed separately and shouldn't pollute the bars/tooltips, and
/// any pre-synthesized `Team` subject is handled by the `TeamAggregate`
/// branch instead.
fn entry_is_renderable(entry: &BillingCycleUsageEntry) -> bool {
    entry.usage_bucket != AiCreditsUsageBucket::Voice
        && entry.usage_bucket != AiCreditsUsageBucket::SuggestedCodeDiffs
        && entry.subject_type != AiCreditsUsageAndCostSubjectType::Team
}

/// Group `entries` by `(cost_type, usage_bucket)` and turn each non-empty
/// group into a [`BarSegment`]. Returns the sorted segments plus the row
/// totals.
///
/// The cynic-generated enums don't implement `Hash`, so we accumulate into
/// a small `Vec` and do linear lookups — the per-row entry count is bounded
/// (at most ~12 cells: 4 cost-types × 3 buckets), making this faster than
/// allocating a HashMap with string-keyed indirection.
fn aggregate_segments<'a>(
    entries: impl IntoIterator<Item = &'a BillingCycleUsageEntry>,
) -> (Vec<BarSegment>, i64, i64) {
    let mut segments: Vec<BarSegment> = Vec::new();

    for entry in entries {
        if let Some(existing) = segments
            .iter_mut()
            .find(|s| s.cost_type == entry.cost_type && s.usage_bucket == entry.usage_bucket)
        {
            existing.credits += entry.credits_used as i64;
            existing.cost_cents += entry.cost_cents as i64;
        } else {
            segments.push(BarSegment {
                cost_type: entry.cost_type.clone(),
                usage_bucket: entry.usage_bucket.clone(),
                credits: entry.credits_used as i64,
                cost_cents: entry.cost_cents as i64,
            });
        }
    }

    segments.retain(|s| s.credits > 0);
    segments.sort_by_key(segment_sort_key);

    let total_credits = segments.iter().map(|s| s.credits).sum();
    let total_cost_cents = segments.iter().map(|s| s.cost_cents).sum();

    (segments, total_credits, total_cost_cents)
}

/// Build the single row shown to `OwnOnly` viewers — the viewer's own
/// aggregated usage. Returns a zero row when the viewer has no entries this
/// cycle.
pub fn build_own_usage_row(
    entries: &[BillingCycleUsageEntry],
    viewer_uid: Option<&str>,
    viewer_display_name: String,
    viewer_base_limit: Option<i64>,
    source_filter: SourceFilter,
) -> MemberUsageRow {
    let viewer_entries: Vec<&BillingCycleUsageEntry> = entries
        .iter()
        .filter(|e| entry_is_renderable(e))
        .filter(|e| source_filter.matches(&e.usage_source))
        // Defensive: even when the server has already redacted to the
        // viewer's own entries (OwnOnly), filter again so a stale server
        // can't leak peers into the OwnOnly row.
        .filter(|e| match (viewer_uid, e.subject_uid.as_deref()) {
            (Some(uid), Some(entry_uid)) => uid == entry_uid,
            // If the entry is missing a subject UID (shouldn't happen for
            // OwnOnly per the server contract), keep it conservatively.
            (_, None) => true,
            // If the viewer's UID is unknown, just trust the server's
            // redaction and render whatever it sent.
            (None, _) => true,
        })
        .collect();

    let (segments, total_credits, total_cost_cents) =
        aggregate_segments(viewer_entries.iter().copied());

    MemberUsageRow {
        subject_type: AiCreditsUsageAndCostSubjectType::User,
        subject_key: SELF_OWN_KEY.to_string(),
        display_name: viewer_display_name,
        total_credits,
        total_cost_cents,
        base_limit: viewer_base_limit,
        segments,
    }
}

/// Build the single synthetic team-aggregate row shown to `TeamAggregate`
/// viewers. Re-aggregates the entries client-side so the row is correct
/// even if the server hasn't collapsed them yet.
pub fn build_team_aggregate_row(
    entries: &[BillingCycleUsageEntry],
    source_filter: SourceFilter,
) -> MemberUsageRow {
    let filtered = entries
        .iter()
        .filter(|e| entry_is_renderable(e))
        .filter(|e| source_filter.matches(&e.usage_source));

    let (segments, total_credits, total_cost_cents) = aggregate_segments(filtered);

    MemberUsageRow {
        subject_type: AiCreditsUsageAndCostSubjectType::Team,
        subject_key: TEAM_AGGREGATE_KEY.to_string(),
        display_name: "Team total".to_string(),
        total_credits,
        total_cost_cents,
        // Team aggregate is not bound by any individual base limit.
        base_limit: None,
        segments,
    }
}

/// Build per-member rows shown to `PerUserTotals` viewers. Iterates the
/// workspace's member list as the source of truth so members with zero
/// usage still get a row. Service-account / unknown-subject entries that
/// don't correspond to any workspace member surface as additional rows at
/// the bottom (mirroring the admin panel's behavior).
pub fn build_member_usage_rows(
    entries: &[BillingCycleUsageEntry],
    members: &[WorkspaceMember],
    source_filter: SourceFilter,
) -> Vec<MemberUsageRow> {
    // First, group entries by subject so we can join against the member
    // list below.
    let mut grouped: HashMap<
        String,
        (
            AiCreditsUsageAndCostSubjectType,
            String,
            Vec<BillingCycleUsageEntry>,
        ),
    > = HashMap::new();
    let mut unknown_counter = 0usize;

    for entry in entries.iter().filter(|e| entry_is_renderable(e)) {
        if !source_filter.matches(&entry.usage_source) {
            continue;
        }

        let key = match entry.subject_uid.as_deref() {
            Some(uid) => format!("{:?}:{uid}", entry.subject_type),
            None => {
                unknown_counter += 1;
                format!("{:?}:unknown-{unknown_counter}", entry.subject_type)
            }
        };
        let group = grouped.entry(key).or_insert_with(|| {
            (
                entry.subject_type.clone(),
                entry
                    .subject_display_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                Vec::new(),
            )
        });
        group.2.push(entry.clone());
    }

    let mut rows: Vec<MemberUsageRow> = Vec::with_capacity(members.len());

    // Render one row per workspace member, including those without any
    // entries — they get a zero row. Members are keyed by their UID, which
    // matches the `subject_uid` carried on entries.
    let mut seen_keys: std::collections::HashSet<String> = Default::default();
    for member in members {
        let key = format!(
            "{:?}:{}",
            AiCreditsUsageAndCostSubjectType::User,
            member.uid.as_str()
        );
        seen_keys.insert(key.clone());

        let (segments, total_credits, total_cost_cents) = match grouped.remove(&key) {
            Some((_, _, entries)) => aggregate_segments(entries.iter()),
            None => (Vec::new(), 0, 0),
        };

        rows.push(MemberUsageRow {
            subject_type: AiCreditsUsageAndCostSubjectType::User,
            subject_key: key,
            display_name: member.email.clone(),
            total_credits,
            total_cost_cents,
            base_limit: member_base_limit(member),
            segments,
        });
    }

    // Anything left in `grouped` is a subject not present in the member
    // list (typically service accounts). Render those after the member
    // rows. Skip entries that were already accounted for above.
    for (key, (subject_type, display_name, entries)) in grouped {
        if seen_keys.contains(&key) {
            continue;
        }
        let (segments, total_credits, total_cost_cents) = aggregate_segments(entries.iter());
        rows.push(MemberUsageRow {
            subject_type,
            subject_key: key,
            display_name,
            total_credits,
            total_cost_cents,
            // Service accounts and other non-member subjects don't have a
            // per-user base limit.
            base_limit: None,
            segments,
        });
    }

    // Sort by total credits descending, matching the admin panel. Stable
    // by `subject_key` to keep zero rows in a deterministic order.
    rows.sort_by(|a, b| {
        b.total_credits
            .cmp(&a.total_credits)
            .then_with(|| a.subject_key.cmp(&b.subject_key))
    });

    rows
}

/// True when any entry carries cloud usage, used to decide whether to show
/// the Local/Cloud source filter toggle.
pub fn has_cloud_usage(entries: &[BillingCycleUsageEntry]) -> bool {
    entries
        .iter()
        .any(|e| e.usage_source == AiCreditsUsageSource::Cloud)
}

fn format_credits(credits: i64) -> String {
    credits.separate_with_commas()
}

fn format_cost_cents(cents: i64) -> String {
    let dollars = cents / 100;
    let remainder = (cents.abs() % 100) as u8;
    if dollars < 0 {
        format!(
            "-${}.{remainder:02}",
            dollars.unsigned_abs().separate_with_commas()
        )
    } else {
        format!("${}.{remainder:02}", dollars.separate_with_commas())
    }
}

fn render_stacked_bar(
    segments: &[BarSegment],
    total_credits: i64,
    team_max_credits: i64,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let track_bg = theme.surface_overlay_1();
    let corner = Radius::Pixels(BAR_CORNER_RADIUS);

    if team_max_credits == 0 || total_credits == 0 || segments.is_empty() {
        // Empty bar: a single muted track, top-rounded on both ends.
        return ConstrainedBox::new(
            Container::new(Empty::new().finish())
                .with_background(track_bg)
                .with_corner_radius(CornerRadius::with_top(corner))
                .finish(),
        )
        .with_height(BAR_HEIGHT)
        .finish();
    }

    let fill_ratio = (total_credits as f32 / team_max_credits as f32).clamp(MIN_FILL_RATIO, 1.0);
    let unfill_ratio = 1.0 - fill_ratio;
    let has_unfill = unfill_ratio > 0.0;
    let last_segment_idx = segments.len() - 1;

    // Build the filled portion: one Expanded child per segment, weighted
    // by the segment's share of `total_credits`. The first segment gets a
    // rounded top-left; the last segment gets a rounded top-right only if
    // there's no muted track tail after it.
    let mut filled = Flex::row();
    for (idx, seg) in segments.iter().enumerate() {
        let weight = seg.credits as f32 / total_credits as f32;
        if weight <= 0.0 {
            continue;
        }
        let is_first = idx == 0;
        let is_last_visible = idx == last_segment_idx && !has_unfill;
        let segment_corner = match (is_first, is_last_visible) {
            (true, true) => CornerRadius::with_top(corner),
            (true, false) => CornerRadius::with_top_left(corner),
            (false, true) => CornerRadius::with_top_right(corner),
            (false, false) => CornerRadius::default(),
        };
        filled.add_child(
            Expanded::new(
                weight,
                Container::new(Empty::new().finish())
                    .with_background_color(cost_type_color(&seg.cost_type))
                    .with_corner_radius(segment_corner)
                    .finish(),
            )
            .finish(),
        );
    }

    let mut bar = Flex::row();
    bar.add_child(Expanded::new(fill_ratio, filled.finish()).finish());
    if has_unfill {
        bar.add_child(
            Expanded::new(
                unfill_ratio,
                Container::new(Empty::new().finish())
                    .with_background(track_bg)
                    .with_corner_radius(CornerRadius::with_top_right(corner))
                    .finish(),
            )
            .finish(),
        );
    }

    ConstrainedBox::new(bar.finish())
        .with_height(BAR_HEIGHT)
        .finish()
}

/// The per-cost-type breakdown grid shown inside the row's hover tooltip,
/// plus a "Total usage" row at the bottom.
fn render_usage_tooltip_content(row: &MemberUsageRow, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    // Use the page background and let the outline border do the work of
    // separating the tooltip from the rows beneath it. This keeps the
    // tooltip visually quiet (no extra surface tier) and lets it read as a
    // clean card-on-page popover.
    let bg = theme.background().into_solid();
    let main = blended_colors::text_main(theme, bg);
    let sub = blended_colors::text_sub(theme, bg);

    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_spacing(6.);

    for line in &row.segments {
        let label = if matches!(line.usage_bucket, AiCreditsUsageBucket::Aggregate) {
            cost_type_label(&line.cost_type).to_string()
        } else {
            format!(
                "{} ({})",
                cost_type_label(&line.cost_type),
                bucket_label(&line.usage_bucket)
            )
        };

        column.add_child(render_tooltip_row(
            Some(cost_type_color(&line.cost_type)),
            label,
            line.credits,
            line.cost_cents,
            sub,
            main,
            font_family,
            /* bold */ false,
        ));
    }

    // Total row, separated by a thin divider line.
    column.add_child(
        Container::new(Empty::new().finish())
            .with_padding_top(1.)
            .with_background_color(theme.outline().into_solid())
            .finish(),
    );

    column.add_child(render_tooltip_row(
        /* no swatch on the total row */ None,
        "Total usage".to_string(),
        row.total_credits,
        row.total_cost_cents,
        main,
        main,
        font_family,
        /* bold */ true,
    ));

    ConstrainedBox::new(
        Container::new(column.finish())
            .with_background_color(bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            // Use the same outline color as the row cards so the tooltip
            // reads as part of the same surface family.
            .with_border(Border::all(1.).with_border_color(theme.outline().into_solid()))
            .with_uniform_padding(10.)
            // Soft drop shadow so the tooltip clearly floats above the rows.
            .with_drop_shadow(
                DropShadow::new_with_standard_offset_and_spread(ColorU::new(0, 0, 0, 48))
                    .with_offset(vec2f(0., 4.)),
            )
            .finish(),
    )
    .with_min_width(240.)
    .with_max_width(360.)
    .finish()
}

/// Single row inside the tooltip. Lays out as three columns to keep
/// numbers aligned vertically across rows:
/// `[swatch + label | flex spacer | credits/$cost cluster]`. The credits
/// and `$cost` text are each right-aligned inside fixed-width
/// `ConstrainedBox`es so columns line up even though rows are laid out
/// independently.
#[allow(clippy::too_many_arguments)]
fn render_tooltip_row(
    swatch_color: Option<ColorU>,
    label: String,
    credits: i64,
    cost_cents: i64,
    label_color: ColorU,
    value_color: ColorU,
    font_family: warpui::fonts::FamilyId,
    bold: bool,
) -> Box<dyn Element> {
    let style = if bold {
        Properties::default().weight(Weight::Semibold)
    } else {
        Properties::default()
    };

    let label_text = Text::new_inline(label, font_family, 12.)
        .with_color(label_color)
        .with_style(style)
        .finish();

    let mut left = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    if let Some(color) = swatch_color {
        left.add_child(
            ConstrainedBox::new(
                Container::new(Empty::new().finish())
                    .with_background_color(color)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.)))
                    .finish(),
            )
            .with_width(10.)
            .with_height(10.)
            .finish(),
        );
        left.add_child(Container::new(label_text).with_margin_left(8.).finish());
    } else {
        left.add_child(label_text);
    }

    let credits_text = Text::new_inline(format_credits(credits), font_family, 12.)
        .with_color(value_color)
        .with_style(style)
        .finish();
    let cost_text = Text::new_inline(format_cost_cents(cost_cents), font_family, 12.)
        .with_color(value_color)
        .with_style(style)
        .finish();
    let divider = Text::new_inline("/".to_string(), font_family, 12.)
        .with_color(label_color)
        .with_style(style)
        .finish();

    let credits_col = ConstrainedBox::new(Align::new(credits_text).right().finish())
        .with_width(TOOLTIP_CREDITS_COL_WIDTH)
        .finish();
    let cost_col = ConstrainedBox::new(Align::new(cost_text).right().finish())
        .with_width(TOOLTIP_COST_COL_WIDTH)
        .finish();

    let right = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(credits_col)
        .with_child(Container::new(divider).with_horizontal_margin(6.).finish())
        .with_child(cost_col)
        .finish();

    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(Shrinkable::new(1., left.finish()).finish())
        .with_child(right)
        .finish()
}

/// Build a single row card (stacked bar + name/total). Shared between the
/// non-hoverable fast path and the Hoverable closure body — the closure
/// can borrow `&Appearance` directly since `Hoverable::new`'s `FnOnce` has
/// no `'static` bound and is invoked immediately inside `new()`.
fn build_row_card(
    row: &MemberUsageRow,
    team_max_credits: i64,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    // Match the tooltip: sit on the page background with just the outline
    // border for definition, so the rows read as part of the page rather
    // than a separate surface tier.
    let card_bg = theme.background().into_solid();
    let main = blended_colors::text_main(theme, card_bg);
    let sub = blended_colors::text_sub(theme, card_bg);

    let bar = render_stacked_bar(
        &row.segments,
        row.total_credits,
        team_max_credits,
        appearance,
    );

    let display_name_text = Text::new_inline(
        row.display_name.clone(),
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main)
    .finish();

    let mut name_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(display_name_text);

    if matches!(
        row.subject_type,
        AiCreditsUsageAndCostSubjectType::ServiceAccount
    ) {
        name_row.add_child(
            Container::new(
                Text::new_inline(
                    "(agent)".to_string(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(sub)
                .finish(),
            )
            .with_margin_left(6.)
            .finish(),
        );
    }

    // Credit + cost cluster: `[coin] X[/limit]   [card] $cost`. Uses
    // `Icon::Credits` (the same coin icon as the "Buy credits"
    // denomination buttons) for the credits side and `Icon::CreditCard`
    // for the cost side.
    let credits_str = match row.base_limit {
        Some(limit) => format!(
            "{}/{}",
            format_credits(row.total_credits),
            format_credits(limit)
        ),
        None => format_credits(row.total_credits),
    };
    let credits_text = Text::new_inline(
        credits_str,
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main)
    .finish();
    let cost_text = Text::new_inline(
        format_cost_cents(row.total_cost_cents),
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main)
    .finish();
    // `to_warpui_icon` takes a `Fill`, so use the theme helper that already
    // returns one rather than wrapping a `ColorU` from `blended_colors`.
    let icon_color = theme.sub_text_color(theme.background());
    let coin_icon = ConstrainedBox::new(Icon::Credits.to_warpui_icon(icon_color).finish())
        .with_width(ROW_ICON_SIZE)
        .with_height(ROW_ICON_SIZE)
        .finish();
    let card_icon = ConstrainedBox::new(Icon::CreditCard.to_warpui_icon(icon_color).finish())
        .with_width(ROW_ICON_SIZE)
        .with_height(ROW_ICON_SIZE)
        .finish();
    let credits_cluster = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(coin_icon)
        .with_child(Container::new(credits_text).with_margin_left(4.).finish())
        .finish();
    let cost_cluster = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(card_icon)
        .with_child(Container::new(cost_text).with_margin_left(4.).finish())
        .finish();

    let credits_and_cost = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(credits_cluster)
        .with_child(
            Container::new(cost_cluster)
                .with_margin_left(ROW_CLUSTER_GAP)
                .finish(),
        )
        .finish();

    let body = Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Shrinkable::new(1., name_row.finish()).finish())
            .with_child(
                Container::new(credits_and_cost)
                    .with_margin_left(16.)
                    .finish(),
            )
            .finish(),
    )
    .with_uniform_padding(ROW_PADDING)
    .finish();

    Container::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(bar)
            .with_child(body)
            .finish(),
    )
    .with_background_color(card_bg)
    .with_border(Border::all(ROW_BORDER_WIDTH).with_border_color(theme.outline().into_solid()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(ROW_BORDER_RADIUS)))
    .finish()
}

/// Render a single row: stacked bar on top, name + total credits below,
/// wrapped in a Hoverable that surfaces the breakdown tooltip on hover.
fn render_member_row(
    row: &MemberUsageRow,
    team_max_credits: i64,
    tooltip_mouse_state: MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    // the viewer hasn't used any credits this cycle => don't bother wrapping in a Hoverable
    if row.segments.is_empty() {
        return build_row_card(row, team_max_credits, appearance);
    }

    Hoverable::new(tooltip_mouse_state, move |state| {
        let mut stack = Stack::new();
        stack.add_child(build_row_card(row, team_max_credits, appearance));

        if state.is_hovered() {
            stack.add_positioned_overlay_child(
                render_usage_tooltip_content(row, appearance),
                OffsetPositioning::offset_from_parent(
                    vec2f(0., -TOOLTIP_GAP),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopMiddle,
                    ChildAnchor::BottomMiddle,
                ),
            );
        }

        stack.finish()
    })
    .finish()
}

pub type FilterChangeFn = std::sync::Arc<dyn Fn(SourceFilter, &mut EventContext) + 'static>;

/// Inline pill toggle for filtering between All / Local / Cloud entries.
fn render_source_filter_toggle(
    current: SourceFilter,
    mouse_states: &RowMouseStates,
    appearance: &Appearance,
    on_change: FilterChangeFn,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.surface_1();
    let main = blended_colors::text_main(theme, bg);
    let sub = blended_colors::text_sub(theme, bg);

    let options: [(SourceFilter, MouseStateHandle); 3] = [
        (SourceFilter::All, mouse_states.filter_all.clone()),
        (SourceFilter::Local, mouse_states.filter_local.clone()),
        (SourceFilter::Cloud, mouse_states.filter_cloud.clone()),
    ];

    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);

    for (filter, mouse_state) in options {
        let label = filter.label();
        let is_selected = filter == current;
        let fg = if is_selected { main } else { sub };
        let font_family = appearance.ui_font_family();
        let on_change = on_change.clone();

        let cell = Hoverable::new(mouse_state, move |_state| {
            let mut cell = Container::new(
                Text::new_inline(label, font_family, 11.)
                    .with_color(fg)
                    .finish(),
            )
            .with_horizontal_padding(10.)
            .with_vertical_padding(4.);
            if is_selected {
                cell = cell.with_background(theme.surface_overlay_1());
            }
            cell.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            on_change(filter, ctx);
        })
        .finish();

        row.add_child(cell);
    }

    Container::new(row.finish())
        .with_border(Border::all(1.).with_border_color(theme.surface_3().into_solid()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
        .finish()
}

/// Top-level dispatcher invoked from the section view's `render_body`.
/// `on_filter_change` is only consumed in the `PerUserTotals` branch (the
/// only one that renders the source-filter toggle) — other branches drop
/// the closure on the floor.
#[allow(clippy::too_many_arguments)]
pub fn render_rows(
    workspace: &Workspace,
    entries: &[BillingCycleUsageEntry],
    viewer_uid: Option<&str>,
    viewer_display_name: Option<&str>,
    visibility: &crate::workspaces::workspace::UsageVisibility,
    source_filter: SourceFilter,
    mouse_states: &RowMouseStates,
    appearance: &Appearance,
    on_filter_change: FilterChangeFn,
) -> Box<dyn Element> {
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_spacing(8.);

    match visibility.granularity {
        UsageVisibilityGranularity::OwnOnly => {
            let display_name = viewer_display_name.unwrap_or("Your usage").to_string();
            // If the viewer is a workspace member with a finite request
            // limit, surface their base limit on the row so they see
            // `used / limit` like admins see for other members.
            let viewer_base_limit = viewer_uid.and_then(|uid| {
                workspace
                    .members
                    .iter()
                    .find(|m| m.uid.as_str() == uid)
                    .and_then(member_base_limit)
            });
            let row = build_own_usage_row(
                entries,
                viewer_uid,
                display_name,
                viewer_base_limit,
                source_filter,
            );
            let tooltip_state = mouse_states.tooltip_mouse_state(&row.subject_key);
            let max_credits = row.total_credits.max(1);
            column.add_child(render_member_row(
                &row,
                max_credits,
                tooltip_state,
                appearance,
            ));
        }
        UsageVisibilityGranularity::TeamAggregate => {
            let row = build_team_aggregate_row(entries, source_filter);
            let tooltip_state = mouse_states.tooltip_mouse_state(&row.subject_key);
            let max_credits = row.total_credits.max(1);
            column.add_child(render_member_row(
                &row,
                max_credits,
                tooltip_state,
                appearance,
            ));
        }
        UsageVisibilityGranularity::PerUserTotals => {
            let rows = build_member_usage_rows(entries, &workspace.members, source_filter);

            // Source filter is only useful when the workspace actually has
            // cloud usage — otherwise the toggle is a confusing no-op.
            if has_cloud_usage(entries) {
                column.add_child(
                    Container::new(
                        Flex::row()
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_main_axis_alignment(MainAxisAlignment::End)
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_child(render_source_filter_toggle(
                                source_filter,
                                mouse_states,
                                appearance,
                                on_filter_change,
                            ))
                            .finish(),
                    )
                    .with_margin_bottom(4.)
                    .finish(),
                );
            }

            if rows.is_empty() {
                column.add_child(render_empty_filter_state(source_filter, appearance));
            } else {
                let max_credits = rows
                    .iter()
                    .map(|r| r.total_credits)
                    .max()
                    .unwrap_or(0)
                    .max(1);
                for row in &rows {
                    let tooltip_state = mouse_states.tooltip_mouse_state(&row.subject_key);
                    column.add_child(render_member_row(
                        row,
                        max_credits,
                        tooltip_state,
                        appearance,
                    ));
                }
            }
        }
        UsageVisibilityGranularity::FullBreakdown => {
            // Enterprise admins short-circuit to the admin-panel CTA
            // before `render_body` runs, so this branch should be
            // unreachable in practice. Render an empty container so the
            // section gracefully degrades if the short-circuit ever
            // changes.
            column.add_child(Empty::new().finish());
        }
    }

    column.finish()
}

fn render_empty_filter_state(
    source_filter: SourceFilter,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.background().into_solid();
    let text = match source_filter {
        SourceFilter::All => "No usage this period",
        SourceFilter::Local => "No local usage this period",
        SourceFilter::Cloud => "No cloud usage this period",
    };
    Container::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_child(
                Text::new_inline(
                    text.to_string(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(blended_colors::text_sub(theme, bg))
                .finish(),
            )
            .finish(),
    )
    .with_background_color(bg)
    .with_border(Border::all(1.).with_border_color(theme.outline().into_solid()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(ROW_BORDER_RADIUS)))
    .with_vertical_padding(24.)
    .finish()
}
