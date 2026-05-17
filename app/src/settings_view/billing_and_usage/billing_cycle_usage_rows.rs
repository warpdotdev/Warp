//! Per-row rendering for the cycle usage section, mirroring the admin-panel
//! visual model in `warp-server/client/src/components/admin/billing/`
//! (`memberUsageUtils.ts` + `MemberUsageTable.tsx`), adapted to the resolved
//! client-side [`UsageVisibility`] model:
//!
//! * `OwnOnly` — one row for the viewer (zero row if no entries).
//! * `TeamAggregate` — one synthetic "Team" row aggregated across all entries.
//! * `PerUserTotals` — one row per workspace member; members with no entries
//!   still get a zero row.
//! * `FullBreakdown` — never reaches here; enterprise admins short-circuit to
//!   the admin-panel CTA before [`render_rows`] runs.
//!
//! The categorical `Voice` / `SuggestedCodeDiffs` buckets and the `Team`
//! subject are filtered out before aggregation, matching the admin panel.

use std::cell::RefCell;
use std::collections::HashMap;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use thousands::Separable;
use warp_core::ui::appearance::Appearance;
use warpui::{
    elements::{
        Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
        Expanded, Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle,
        OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Shrinkable,
        Stack, Text,
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
    ui_components::blended_colors,
    workspaces::workspace::{
        AiCreditsUsageAndCostSubjectType, AiCreditsUsageAndCostType, AiCreditsUsageBucket,
        AiCreditsUsageSource, BillingCycleUsageEntry, UsageVisibilityGranularity, Workspace,
        WorkspaceMember,
    },
};

const BAR_HEIGHT: f32 = 8.;
/// Reserve at least 5% width for the filled portion so even tiny usages
/// remain visible, matching the admin panel's `Math.max(pct, 5)`.
const MIN_FILL_RATIO: f32 = 0.05;
const ROW_BORDER_RADIUS: f32 = 8.;
const ROW_PADDING: f32 = 12.;
const TOOLTIP_GAP: f32 = 4.;

/// All cost-type buckets the admin panel considers, in the order they are
/// rendered within a single bar (and listed in tooltips).
const COST_TYPE_ORDER: &[AiCreditsUsageAndCostType] = &[
    AiCreditsUsageAndCostType::BaseLimit,
    AiCreditsUsageAndCostType::BonusGrant,
    AiCreditsUsageAndCostType::Payg,
    AiCreditsUsageAndCostType::AmbientBonusGrant,
];

/// Usage-bucket ordering used as a secondary sort key inside a single cost
/// type. Mirrors `BUCKET_ORDER` in `memberUsageUtils.ts`.
const BUCKET_ORDER: &[AiCreditsUsageBucket] = &[
    AiCreditsUsageBucket::Ai,
    AiCreditsUsageBucket::Compute,
    AiCreditsUsageBucket::Platform,
];

/// Source filter for the "Usage" table when the workspace has any cloud
/// (ambient agent) usage. The toggle is hidden when no cloud usage exists.
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

/// Persistent mouse state owned by the section view. Pre-allocated for the
/// three filter toggles plus a dynamically grown set of per-row tooltip
/// states keyed by subject UID. Subject-less rows (e.g. the synthetic
/// `TeamAggregate` row) get a stable sentinel key.
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
    /// Segments sorted by [`COST_TYPE_ORDER`] then [`BUCKET_ORDER`], with
    /// any zero-credit entries filtered out.
    pub segments: Vec<BarSegment>,
    /// Same as [`MemberUsageRow::segments`] but retains zero-credit entries
    /// so tooltips can show the full breakdown. Currently identical to
    /// `segments` since we don't synthesize zero-credit lines, but kept as a
    /// distinct field to mirror the admin-panel structure.
    pub tooltip_lines: Vec<BarSegment>,
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
        tooltip_lines: segments.clone(),
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
        tooltip_lines: segments.clone(),
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
            tooltip_lines: segments.clone(),
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
            tooltip_lines: segments.clone(),
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

/// Stacked horizontal bar: filled portion sized to `total_credits /
/// team_max_credits` (clamped to a minimum of 5%), with each segment
/// proportional to its slice of `total_credits`. The unfilled portion uses
/// the muted track color.
fn render_stacked_bar(
    segments: &[BarSegment],
    total_credits: i64,
    team_max_credits: i64,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let track_bg = theme.surface_overlay_1();

    if team_max_credits == 0 || total_credits == 0 {
        return ConstrainedBox::new(
            Container::new(Empty::new().finish())
                .with_background(track_bg)
                .finish(),
        )
        .with_height(BAR_HEIGHT)
        .finish();
    }

    let fill_ratio = (total_credits as f32 / team_max_credits as f32).clamp(MIN_FILL_RATIO, 1.0);
    let unfill_ratio = 1.0 - fill_ratio;

    // Build the filled portion: one Expanded child per segment, weighted by
    // the segment's share of `total_credits`.
    let mut filled = Flex::row();
    for seg in segments {
        let weight = seg.credits as f32 / total_credits as f32;
        if weight <= 0.0 {
            continue;
        }
        filled.add_child(
            Expanded::new(
                weight,
                Container::new(Empty::new().finish())
                    .with_background_color(cost_type_color(&seg.cost_type))
                    .finish(),
            )
            .finish(),
        );
    }

    let mut bar = Flex::row();
    bar.add_child(Expanded::new(fill_ratio, filled.finish()).finish());
    if unfill_ratio > 0.0 {
        bar.add_child(
            Expanded::new(
                unfill_ratio,
                Container::new(Empty::new().finish())
                    .with_background(track_bg)
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
    let bg = theme.tooltip_background();
    let main = blended_colors::text_main(theme, bg);
    let sub = blended_colors::text_sub(theme, bg);

    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_spacing(6.);

    for line in &row.tooltip_lines {
        let label = if matches!(line.usage_bucket, AiCreditsUsageBucket::Aggregate) {
            cost_type_label(&line.cost_type).to_string()
        } else {
            format!(
                "{} ({})",
                cost_type_label(&line.cost_type),
                bucket_label(&line.usage_bucket)
            )
        };

        let swatch = ConstrainedBox::new(
            Container::new(Empty::new().finish())
                .with_background_color(cost_type_color(&line.cost_type))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.)))
                .finish(),
        )
        .with_width(10.)
        .with_height(10.)
        .finish();

        let label_text = Text::new_inline(label, font_family, 12.)
            .with_color(sub)
            .finish();
        let credits_text = Text::new_inline(
            format!("{} cr", format_credits(line.credits)),
            font_family,
            12.,
        )
        .with_color(sub)
        .finish();
        let cost_text = Text::new_inline(format_cost_cents(line.cost_cents), font_family, 12.)
            .with_color(main)
            .finish();

        column.add_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(swatch)
                        .with_child(Container::new(label_text).with_margin_left(8.).finish())
                        .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(credits_text)
                        .with_child(Container::new(cost_text).with_margin_left(12.).finish())
                        .finish(),
                )
                .finish(),
        );
    }

    // Total row, separated by a thin divider line.
    column.add_child(
        Container::new(Empty::new().finish())
            .with_padding_top(1.)
            .with_background_color(blended_colors::neutral_5(theme))
            .finish(),
    );

    let total_label = Text::new_inline("Total usage", font_family, 12.)
        .with_color(main)
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish();
    let total_credits_text = Text::new_inline(
        format!("{} cr", format_credits(row.total_credits)),
        font_family,
        12.,
    )
    .with_color(main)
    .with_style(Properties::default().weight(Weight::Semibold))
    .finish();
    let total_cost_text =
        Text::new_inline(format_cost_cents(row.total_cost_cents), font_family, 12.)
            .with_color(main)
            .with_style(Properties::default().weight(Weight::Semibold))
            .finish();

    column.add_child(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(total_label)
            .with_child(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(total_credits_text)
                    .with_child(
                        Container::new(total_cost_text)
                            .with_margin_left(12.)
                            .finish(),
                    )
                    .finish(),
            )
            .finish(),
    );

    Container::new(column.finish())
        .with_background_color(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
        .with_border(Border::all(1.).with_border_color(blended_colors::neutral_5(theme)))
        .with_uniform_padding(10.)
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
    let surface_bg = theme.surface_1();
    let main = blended_colors::text_main(theme, surface_bg);
    let sub = blended_colors::text_sub(theme, surface_bg);

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

    let credits_text = Text::new_inline(
        format!(
            "{} cr · {}",
            format_credits(row.total_credits),
            format_cost_cents(row.total_cost_cents)
        ),
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main)
    .finish();

    let body = Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Shrinkable::new(1., name_row.finish()).finish())
            .with_child(Container::new(credits_text).with_margin_left(16.).finish())
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
    .with_background_color(theme.surface_1().into_solid())
    .with_border(Border::all(1.).with_border_color(theme.surface_3().into_solid()))
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
    // Empty tooltip lines means the viewer hasn't used any credits this
    // cycle — nothing useful to show in a tooltip, so don't bother
    // wrapping in a Hoverable that opens an empty popover.
    if row.tooltip_lines.is_empty() {
        return build_row_card(row, team_max_credits, appearance);
    }

    // `Hoverable::new`'s `build_child` closure is `FnOnce` without a
    // `'static` bound and is invoked immediately, so we can capture `row`
    // and `appearance` by reference without any cloning.
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

/// Callback signature for source-filter clicks. Receives the newly-clicked
/// filter and the active [`EventContext`] so callers can dispatch their own
/// typed action without this module needing to know about the section's
/// action enum.
pub type FilterChangeFn = std::sync::Arc<dyn Fn(SourceFilter, &mut EventContext) + 'static>;

/// Inline pill toggle for filtering between All / Local / Cloud entries.
/// Invokes `on_change` when a button is clicked.
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
            let row = build_own_usage_row(entries, viewer_uid, display_name, source_filter);
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
    let bg = theme.surface_1();
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
    .with_background_color(theme.surface_1().into_solid())
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(ROW_BORDER_RADIUS)))
    .with_vertical_padding(24.)
    .finish()
}
