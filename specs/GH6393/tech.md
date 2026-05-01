# OSC 8 hyperlinks — implementation plan

Source issue: [warpdotdev/warp#6393](https://github.com/warpdotdev/warp/issues/6393).
Behavior: see [`product.md`](product.md).

## Context

OSC 8 is the cross-terminal escape sequence for attaching a URL to a span of cells (`ESC ] 8 ; params ; URI ST` … visible text … `ESC ] 8 ; ; ST`). Warp parses OSC sequences via its alacritty/vte fork but currently has no `b"8"` arm — the bytes are dropped on the `_ => unhandled(params)` floor and the visible text becomes plain, unclickable output.

**ANSI processor** — `app/src/terminal/model/ansi/mod.rs:811-1230` is the OSC dispatch (`fn osc_dispatch`). It is a long match on the first param: `b"0" | b"2"`, `b"4"`, `b"9"`, `b"10" | b"11" | b"12"`, `b"50"`, `b"52"`, `b"104"`, `b"110-112"`, `b"133"` (prompt markers), `b"777"`, `b"1337"` (iTerm images), and several Warp-specific OSC numbers. Each arm calls into the `Handler` trait. The dispatcher itself stays stateless; per-stream state lives on the implementer.

**Handler trait** — `app/src/terminal/model/ansi/handler.rs:27-…`. Long trait with required and default methods. Default methods are used for hooks that not every implementor cares about (`prompt_marker`, `pluggable_notification`, the Warp DCS hook callbacks). Today's implementors: `GridHandler` (`app/src/terminal/model/grid/ansi_handler.rs:159`), `BlockGrid` (`app/src/terminal/model/blockgrid.rs:704`), `Block` (`app/src/terminal/model/block.rs:2906`), `AltScreen` (`app/src/terminal/model/alt_screen.rs:371`), `EarlyOutputHandler` (`app/src/terminal/model/early_output.rs:298`), and the test `MockHandler` (`app/src/terminal/model/ansi/mod_test.rs:24`).

**Shared ANSI types** — `crates/warp_terminal/src/model/ansi/control_sequence_parameters.rs`. `Attr`, `Mode`, `CursorStyle`, `PromptMarker` etc. live here and are re-exported by `app/src/terminal/model/ansi/mod.rs:21` (`pub use warp_terminal::model::ansi::control_sequence_parameters::*;`). New shared types (`Hyperlink`) belong here so both the parser and the trait can name them.

**Cell storage — block list path (`FlatStorage`).** `crates/warp_terminal/src/model/grid/flat_storage/` uses a run-length-encoded `AttributeMap<T>` per attribute: `FgColorMap` and `BgAndStyleMap` (`flat_storage/style.rs:18-21`). This representation deduplicates long runs of cells that share the same value into a single map entry — a perfect fit for hyperlinks, where one OSC 8 span typically covers many adjacent cells with the same URI. Used by `GridHandler` (`grid_handler.rs:381`).

**Cell storage — alt-screen / row-based path (`Cell` + `Row`).** `crates/warp_terminal/src/model/grid/cell.rs:144` defines `Cell` (24 bytes, deliberately tightly packed; the doc comment at line 122 calls out that adding bytes to `Cell` itself bumps the struct to 32 bytes — a 33% memory hit). Optional rare attributes go in `CellExtra` (`cell.rs:113-120`), a boxed side allocation that already holds `cell_with_zero_width` and `end_of_prompt`. Hyperlinks belong in `CellExtra`, not in the base `Cell`.

**Auto-detected URL flow — the model that the OSC 8 plumbing should mirror:**
- Detection lives in `app/src/terminal/model/grid/grid_handler.rs` via `pub fn url_at_point(&self, displayed_point: Point) -> Option<Link>` at line 596 and `Link` at line 143 (`{ range: RangeInclusive<Point>, is_empty: bool }`). `Link: ContainsPoint` (line 169).
- Higher-level wrappers: `app/src/terminal/model/blocks.rs:2460` (`url_at_point` returning `WithinBlock<Link>`), `app/src/terminal/model/alt_screen.rs:285`, and `app/src/terminal/model/terminal_model.rs:1815` (`link_at_range`) / `1844` (`url_at_point` returning `WithinModel<Link>`).
- View layer: `app/src/terminal/view/link_detection.rs` defines `GridHighlightedLink::{Url(WithinModel<Link>), File(WithinModel<FileLink>)}` (line 48), the hover state machine that calls `model.url_at_point(...)` → `self.highlighted_link.set(GridHighlightedLink::Url(url), ...)` at lines 299-310, and the click handler at lines 391-395 (`GridHighlightedLink::Url(url) => { let model = self.model.lock(); ctx.open_url(&model.link_at_range(url, ...)); }`). Right-click context menu and `OpenGridLink` action wiring is in `app/src/terminal/view.rs:24786`.
- Critically: the URL the auto-detector hands to `ctx.open_url` is recovered from the cell text via `link_at_range` — there is no notion of a URL stored separately from the cells. OSC 8 inverts that: the URI is independent of the visible text and must be carried alongside the cells.

A WIP foundation (`Hyperlink` type + parser + Handler hook + OSC 8 dispatch arm + parser unit tests) was scaffolded earlier on this branch and stashed; it is referenced where useful but is not the spec.

## Proposed changes

The implementation is broken into the layers below, listed bottom-up. Each layer can be merged independently behind the `OscHyperlinks` feature flag described in (6); the ordering in **Parallelization** below maps each layer to its dependencies. Layer 5a (scheme allow-list) is independent of the rest and intentionally lands first as a hardening of the existing URL click path.

### 1. ANSI types — `crates/warp_terminal/src/model/ansi/control_sequence_parameters.rs`

Add a `Hyperlink { id: Option<String>, uri: String }` value type, a `HyperlinkParseError`, and `Hyperlink::parse_osc_params(params: &[&[u8]]) -> Result<Option<Self>, HyperlinkParseError>`. Layout per the OSC 8 spec:

- Two fields after the `b"8"` identifier: `params_field` (colon-separated `key=value` list, may be empty) and `uri_field`.
- Empty `uri_field` (or single empty field — what some emitters send for the close form) → `Ok(None)`, the close form.
- `params_field` is split on `:`; only `id=…` is recognized today, others ignored. A param without `=` is `Err(MalformedParam)`.
- Non-UTF-8 bytes in the URI return `Err(InvalidUtf8)`.

Parser unit tests live next to the type in a `#[cfg(test)] mod hyperlink_parse_tests`.

Tradeoffs considered:
- **Storing `id` and `uri` directly vs. interning.** Interning at parse time would couple the parser to an app-level registry. The parser stays pure and small; deduplication happens at the storage layer (3).
- **`Hyperlink` as an enum vs. struct.** A `Hyperlink::Open(_)`/`Hyperlink::Close` enum is more self-describing than `Option<Hyperlink>`, but the close form has no payload, so `Option` is the smaller surface and matches `set_title(Option<String>)` and similar Handler signatures.

### 2. Handler trait + OSC 8 dispatch — `app/src/terminal/model/ansi/handler.rs` and `mod.rs`

Add a default-impl method on `Handler`:

```rust
fn set_hyperlink(&mut self, _hyperlink: Option<Hyperlink>) {}
```

The default-impl pattern matches `prompt_marker`, `pluggable_notification`, and the Warp DCS hook callbacks; existing implementors continue to compile unchanged and only the implementors that need to actually attach hyperlinks to cells (3) override it.

In `osc_dispatch`, add a `b"8"` arm next to `b"9"`/`b"133"`:

```rust
b"8" => match Hyperlink::parse_osc_params(&params[1..]) {
    Ok(hyperlink) => self.handler.set_hyperlink(hyperlink),
    Err(_) => unhandled(params),
},
```

### 3. Cell storage — attaching the hyperlink to cells

The implementation needs a deduplicated registry keyed by an opaque small-integer `HyperlinkId` so cells store a 4-byte handle rather than a per-cell `Arc<Hyperlink>`. The registry is per-grid (block-scoped on `BlockList`, screen-scoped on `AltScreen`).

```rust
// New module: crates/warp_terminal/src/model/grid/hyperlink_registry.rs
pub struct HyperlinkId(NonZeroU32);
pub struct HyperlinkRegistry { /* HashMap<Hyperlink, HyperlinkId> + Vec<Hyperlink> */ }
impl HyperlinkRegistry {
    pub fn intern(&mut self, h: Hyperlink) -> HyperlinkId;
    pub fn get(&self, id: HyperlinkId) -> Option<&Hyperlink>;
}
```

Two storage variants need wiring:

**3a. `FlatStorage` (block list).** Add a third `AttributeMap<Option<HyperlinkId>>` parallel to `FgColorMap`/`BgAndStyleMap` in `crates/warp_terminal/src/model/grid/flat_storage/mod.rs`. RLE compression makes a 100-cell hyperlink cost one map entry. A new `flat_storage/hyperlink.rs` module mirroring `flat_storage/style.rs` is the natural shape.

**3b. `Cell` / `Row` (alt-screen and other row-based grids).** Extend `CellExtra` (`crates/warp_terminal/src/model/grid/cell.rs:113`) with `hyperlink_id: Option<HyperlinkId>` and add accessors `Cell::hyperlink_id() / Cell::set_hyperlink_id()`. The 24→24 byte budget for `Cell` itself is preserved because the new field lives in the boxed extra. Resetting a cell preserves `EndOfPromptMarker`; preserve `hyperlink_id` only while the cell still has content.

**3c. State on grid handlers.** `GridHandler`, `BlockGrid`, `Block`, and `AltScreen` each get an `active_hyperlink_id: Option<HyperlinkId>` field plus a per-grid `HyperlinkRegistry`. They override `set_hyperlink`:

```rust
fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
    self.active_hyperlink_id = hyperlink.map(|h| self.hyperlink_registry.intern(h));
}
```

The grid's `input(&mut self, c: char)` path stamps `self.active_hyperlink_id` into each newly-written cell, in the same place SGR styling is applied today.

**Block boundaries (product invariant 10).** `BlockList` clears `active_hyperlink_id` on prompt-marker block transitions (`prompt_marker` handler) and on `clear`/`reset_state`. `AltScreen` clears on its own reset.

**Bounded registry, no reclamation (security: DoS resistance).** Terminal output is untrusted and a hostile or buggy process can emit unlimited unique URIs. The registry is bounded along three axes, all enforced at `intern` time, and **uses a no-reclaim model**: entries are never freed while the registry is alive. The registry's lifetime is the grid's lifetime — when the grid (block, alt-screen, etc.) is dropped or replaced, the entire registry goes with it.

| Cap | Default | Behavior on hit |
| --- | --- | --- |
| Max URI byte length | 4096 | Sequence is dropped: `intern` returns `None`; `set_hyperlink` is treated as if the OSC 8 had been malformed. The visible text continues to render (per product invariant 15). |
| Max distinct entries per registry | 4096 | New entries are not inserted; `intern` returns `None`. Existing entries stay valid; old links remain clickable. A `log::warn!` fires once per cap hit per session. |
| Max referencing cells per entry | unbounded | Bounded indirectly by the grid's row cap. |

**Why no reclamation.** A reclamation model would need consistent reference-count accounting across cell overwrites (replace `hyperlink_id`, dec/inc), row eviction from scrollback, RLE run splits and merges in `FlatStorage`, reflow on resize (rows rebuilt from underlying spans), and deserialization (incoming cells must bump counts on the loaded registry). Getting any one of those wrong leaks entries (memory creep) or under-counts (use-after-free of an `id` that still appears in some cell). For a feature where the steady-state working set per block is small (single-digit URLs in real-world output) and the cap (4096 entries × ~1 KB average ≈ 4 MB) is already small, the simpler "registry grows monotonically until grid is dropped" model is the right tradeoff — and trivially correct under all of the above transitions because the only mutation is "append at intern time."

**Block-grid lifetime.** Per product invariant 10, a `BlockList` block is the natural unit of registry ownership: hyperlinks are reset on prompt-marker boundaries, and when a block is fully evicted from scrollback the whole block (including its registry) is dropped. So the working set is bounded by the *current* block's distinct URLs, not the session's. `AltScreen` registries are cleared on screen reset.

**Caps are `pub const`** in the registry module so tests can override them via `#[cfg(test)] const MAX_DISTINCT_ENTRIES: usize = 4;` in a test-only build.

Tests (in `hyperlink_registry_tests.rs`):
- `intern` returns `None` for a URI exceeding the byte cap.
- 4097 distinct interns: the first 4096 succeed, the 4097th returns `None`, the first 4096 stay valid.
- Cells overwritten with new content do **not** cause the registry to shrink — `len_for_test()` does not decrease, only `Drop` of the registry frees memory. Documented as the intended behavior, not a bug.
- A 1 MB OSC 8 sequence does not OOM the process; the parser drops it via the URI byte cap.
- Dropping a `BlockList` block drops its registry (asserted via a `Weak`/Drop counter in tests).

### 4. Lookup helpers — model layer

Mirror the auto-detected URL flow.

The existing `Link` (`grid_handler.rs:143`) is a single `RangeInclusive<Point>` and represents one contiguous run. Per product invariant 5 (narrowed in this revision), an OSC 8 span is also a single contiguous run — the cells written between one `set_hyperlink(Some(_))` call and its matching `set_hyperlink(None)`/implicit close. We therefore reuse `Link` as-is for the v1 lookup. Cross-run grouping by `id` (a non-contiguous "link group" of the same `id`) is **not** supported in v1 because `Link` cannot represent multiple disjoint ranges; see Follow-ups for the multi-range path.

- `GridHandler::hyperlink_at_point(&self, point: Point) -> Option<Link>` (`grid_handler.rs`, alongside the existing `url_at_point` at line 596). Returns the contiguous `Link` for the OSC 8 span at `point`, expanded by walking left/right while the cell's `HyperlinkId` matches and the cells remain adjacent.
- `GridHandler::hyperlink_uri_at_point(&self, point: Point) -> Option<&str>` to read the URI without recovering it from cell text.
- Block / alt-screen / model-level wrappers paralleling `blocks.rs:2460`, `alt_screen.rs:285`, `terminal_model.rs:1844` (`hyperlink_at_point`, `hyperlink_uri_at_range`).

### 5. View layer — hover, click, copy, context menu

`app/src/terminal/view/link_detection.rs`:

- Extend `GridHighlightedLink` with a third variant: `Hyperlink(WithinModel<Link>, String /* uri */)`. The variant carries the URI directly because, unlike `Url`, it is not recoverable from the cell text.
- In the hover state machine (lines 299-339), call `model.hyperlink_at_point(position)` first; if it returns `Some`, set `GridHighlightedLink::Hyperlink(...)`. Only fall through to `url_at_point` (and the file-path scanner) when no OSC 8 span is found at that point — this implements product invariant 9 (OSC 8 wins over auto-detected URL on the same cell).
- In the click handler (lines 391-395), add the `Hyperlink` arm: pass the URI through the centralized scheme validator (5a) before any open call; if it fails, the click is a no-op and the hover tooltip surfaces the rejection reason. Same telemetry path (`TelemetryEvent::OpenLink`).
- Tooltip text: `GridHighlightedLink::tooltip_text` (line 63) returns "Open link" for the new variant; for a hyperlink whose scheme fails the allow-list, the tooltip text is "Scheme not allowed: <scheme>" so the user understands why the click is inert.

`app/src/terminal/view.rs`:

- The `OpenGridLink(link)` action (line 24786) gets a new arm for `Hyperlink` that calls the validated open helper from (5a).
- The right-click context menu wiring around line 15040 gets a `GridHighlightedLink::Hyperlink` branch with "Open link" / "Copy link" items. "Open link" routes through (5a). "Copy link" copies the URI to the clipboard verbatim regardless of scheme — copying is not navigating, so the allow-list does not gate it.

### 5a. Centralized scheme allow-list (security)

Terminal output is untrusted, OSC 8 carries arbitrary URIs, and `ctx.open_url` is a thin wrapper that hands the string to the platform — it is **not** itself a validator. We add a single chokepoint that every path must call. The validator takes the URI as a `&str` (not a parsed `Url`) so it never forces the caller to throw away a URI that happens to be unparseable — that matters for hover/copy on malformed input (product invariant 15).

```rust
// New module: app/src/terminal/view/link_security.rs
pub enum SchemeCheck {
    Allowed,
    Rejected { reason: SchemeRejectReason },
}

pub enum SchemeRejectReason {
    /// URI failed to parse as a URL (e.g. spaces, no scheme, garbage).
    Unparseable,
    /// URI parsed but the scheme isn't on the allow-list.
    DisallowedScheme { scheme: String },
}

/// Returns Allowed iff the URI parses as a URL whose scheme is in the
/// allow-list for opening untrusted links. Called by every code path
/// that opens a URI coming from terminal output (OSC 8 hyperlinks and
/// auto-detected URLs).
pub fn check_open_scheme(uri: &str) -> SchemeCheck;

/// Convenience: validate then call ctx.open_url. Returns the
/// SchemeCheck so the click/menu path can pick a tooltip and decide
/// whether to fire OpenLink telemetry.
pub fn open_validated(ctx: &mut impl AppContextLike, uri: &str) -> SchemeCheck;
```

The allow-list is `http`, `https`, `mailto`, and `ftp`. Any other scheme — `javascript:`, `data:`, `file:`, `vbscript:`, `about:`, `chrome:`, custom protocol handlers, etc. — is rejected. The list is a `const &[&str]`, not a runtime config, to keep the boundary auditable.

**Hover, copy, and click semantics for malformed/disallowed URIs** (product invariant 15):
- The hyperlink span on the cells is unaffected by validity: hover always works, the tooltip always shows the literal URI, and right-click "Copy link" always copies the literal URI to the clipboard regardless of `SchemeCheck`. Validation only gates *opening* the URI.
- Click ("Open link") paths call `open_validated` and switch on the result:
  - `Allowed` → URI was opened; tooltip is "Open link".
  - `Rejected { Unparseable }` → click was a no-op; tooltip is "Cannot open: URI is malformed".
  - `Rejected { DisallowedScheme { scheme } }` → click was a no-op; tooltip is "Scheme not allowed: <scheme>".

Required call sites (each one becomes a "must call `open_validated` / `check_open_scheme`" bullet in code review):

- `app/src/terminal/view/link_detection.rs:391` — auto-detected URL click. **Today this calls `ctx.open_url` directly with no validation; this PR closes that gap as well**, so OSC 8 and auto-detected URLs share the same security boundary.
- New `Hyperlink` arm in the click handler.
- The `OpenGridLink` action arm at `app/src/terminal/view.rs:24786`.
- The right-click "Open link" menu arm.
- Any future code path that opens a URL that originated in terminal output. Lint rule (custom clippy or grep-based presubmit check) flags raw `ctx.open_url` calls inside `app/src/terminal/view/`; existing call sites that open *Warp-internal* URLs (settings deep links, docs URLs) are explicitly allow-listed in the lint.

Tests live in `link_security_tests.rs`:
- `http://x`, `https://x`, `mailto:a@b`, `ftp://x` → `Allowed`.
- `javascript:alert(1)`, `data:text/html,…`, `file:///etc/passwd`, `vbscript:`, `about:blank`, empty scheme → `Rejected { DisallowedScheme }`.
- Mixed-case (`HTTP://`, `JavaScript:`) is canonicalized — case-insensitive scheme match.
- Garbage strings (`"hello world"`, `""`, `"://"`) → `Rejected { Unparseable }`.
- A click test for OSC 8 with `javascript:alert(1)` confirms `ctx.open_url` is **never** called and the tooltip shows the rejection reason.
- A hover test for an unparseable URI confirms the tooltip still shows the literal URI and "Copy link" still copies it.

### 6. Feature flag

`FeatureFlag::OscHyperlinks` (added per `WARP.md`'s feature-flag guide, defaulted on for dogfood) gates **OSC 8 specific** behavior only: when off, `osc_dispatch`'s `b"8"` arm calls `unhandled(params)` and the rest of layers (3)–(5)/(6a)/(7) never sees an OSC 8 hyperlink.

Layer (5a) — the scheme allow-list — is **deliberately not** gated by this flag. Validation is a hardening change to the existing auto-detected URL click path that benefits users regardless of OSC 8. Disabling `OscHyperlinks` must not regress security on the URL flow. The flag's compile-time wiring is restricted to the `b"8"` arm and the layer (5) `Hyperlink` variant of `GridHighlightedLink`; layer (5a) lives outside the flag and is on for everyone the moment its layer ships.

This split lets each layer land independently and lets the team revert OSC 8 in one place if a regression appears, without losing the hardening of the auto-detected URL flow.

### 6a. Session persistence (Warp Drive, history, shared sessions)

`Cell` and `CellExtra` already derive `Serialize` / `Deserialize` (`crates/warp_terminal/src/model/grid/cell.rs:100,113,143`), so adding `hyperlink_id: Option<HyperlinkId>` to `CellExtra` automatically extends the wire format. The harder part is **resolving** the id back to a URI on the receiving side — the integer is meaningless without the registry. The plan:

- Tag `HyperlinkId` itself with `#[derive(Serialize, Deserialize)]` (a `NonZeroU32` is trivially serializable).
- Persist the `HyperlinkRegistry` alongside the grid wherever the grid itself is persisted: in the same blob/document for Warp Drive blocks, the same row for sqlite-backed history, and the same message for shared-session protocol frames. Concretely:
  - `BlockGrid` / `Block` gain `pub registry: HyperlinkRegistry` in their serialized shape.
  - `AltScreen` gains the same.
  - The session-sharing protocol (`session-sharing-protocol` workspace crate) adds a `hyperlink_registry: HyperlinkRegistry` field to the per-block payload. The new field is `#[serde(default)]` so older clients that don't send it deserialize cleanly (`HyperlinkId` becomes orphaned and the cell renders as a plain non-clickable cell — graceful degradation).
- Cross-block id stability: ids are local to each grid's registry. Restoring a session reconstructs the same id space because we serialize the registry *with* the grid.
- Forward compatibility: an older Warp build reading a block that contains a `hyperlink_id` field it doesn't know about must ignore it, not error. We confirm this is the case (or fix it) for each receiving site: the sqlite history reader, the Warp Drive deserializer, and the session-sharing client.

Tests:
- Round-trip serialize → deserialize a block with a hyperlinked span; assert the cells still resolve to the same URI through the restored registry.
- Round-trip a block produced by *this* client through a deserializer that lacks the hyperlink fields (simulating an older client); assert the deserialization succeeds and the visible cells render unchanged (clickability lost is acceptable).
- The session-sharing integration test exercises a producer that emits OSC 8 and a consumer that observes the link as clickable on the receiver side.

This subsumes the earlier "session persistence" follow-up; it is no longer deferred.

### 7. Sharing, copy-as-markdown, and AI context

- **Markdown sharing** (product invariant 13, first sentence). The block→markdown serializer (search for `to_markdown` / shared-session export) emits `[visible text](URI)` for spans that carry a `HyperlinkId`.
- **Copy block as terminal bytes** (product invariant 13, second sentence). The byte serializer emits a *semantically equivalent* OSC 8 sequence around each span: `ESC ] 8 ; ; <uri> ESC \` … visible bytes … `ESC ] 8 ; ; ESC \`. We **do not** preserve the original OSC bytes — only the URI is round-tripped, params are normalized to empty, and the terminator is normalized to `ESC \`. This is deliberate: the registry stores normalized `Hyperlink { id, uri }`, not raw bytes, so byte-exact round-tripping would require carrying the original OSC bytes per cell. The cost (per-cell byte arrays) outweighs the benefit (only programs that round-trip Warp output through another OSC-8-aware terminal would notice the difference, and the receiving terminal renders the same clickable link either way). Product invariant 13 is updated to reflect this.
- **AI context** (invariant 14). The block→agent context formatter inlines `visible text (URI)` for hyperlinked spans so an agent reading wizcli output sees the URI without losing the visible label.

## End-to-end flow

```mermaid
sequenceDiagram
    participant PTY
    participant Processor as ansi::Performer
    participant Handler as GridHandler/BlockGrid/AltScreen
    participant Registry as HyperlinkRegistry
    participant Storage as FlatStorage / Cell+Row
    participant View as TerminalView
    participant Validator as link_security
    participant Browser

    PTY->>Processor: ESC ] 8 ; id=foo ; https://x ESC \ "Click me" ESC ] 8 ; ; ESC \
    Processor->>Handler: set_hyperlink(Some(Hyperlink{id:"foo", uri:"https://x"}))
    Handler->>Registry: intern(hyperlink) -> HyperlinkId
    Handler->>Handler: active_hyperlink_id = Some(id)
    loop Each printable char "C","l","i","c","k"," ","m","e"
        Processor->>Handler: input(c)
        Handler->>Storage: write cell with hyperlink_id = active_hyperlink_id
    end
    Processor->>Handler: set_hyperlink(None)
    Handler->>Handler: active_hyperlink_id = None

    Note over View: User hovers a cell
    View->>Storage: hyperlink_at_point(p)
    Storage-->>View: Some(Link { range })
    View->>Registry: get(id) -> Hyperlink
    View->>View: cursor=PointingHand, tooltip="Open link" + uri

    Note over View: User Cmd-clicks
    View->>Validator: open_validated(ctx, "https://x")
    Validator->>Validator: parse + scheme allow-list check
    alt Allowed
        Validator->>Browser: ctx.open_url("https://x")
        Validator-->>View: SchemeCheck::Allowed
    else Rejected (Unparseable / DisallowedScheme)
        Validator-->>View: SchemeCheck::Rejected{...}
        View->>View: tooltip explains why click is inert
    end
```

## Testing and validation

Each numbered item below maps to a product invariant from `product.md`.

**Unit tests — parser** (`crates/warp_terminal/src/model/ansi/control_sequence_parameters.rs`, `#[cfg(test)] mod hyperlink_parse_tests`).
- Open / open-with-id / close / single-empty-field-close / unknown-keys-ignored / malformed-param / multi-`:` separators / non-UTF-8 URI → invariants 1, 2, 3, 15.

**Unit tests — dispatch** (`app/src/terminal/model/ansi/mod_test.rs`, alongside the existing `parse_osc9_*` tests).
- Open + close fires two `set_hyperlink` events on `MockHandler` (the mock gets a `hyperlink_events: Vec<Option<Hyperlink>>` field) → invariant 1.
- Bell-terminated and ESC-`\`-terminated forms both work → invariant 1.
- Malformed `OSC 8 ; foo` (no URI segment) does not panic, does not fire an event → invariant 15.
- Empty-URI close after open clears → invariant 2.

**Unit tests — registry + cell stamping** (new `hyperlink_registry_tests.rs`, plus extension of `cell_test.rs`).
- `intern` deduplicates: same `Hyperlink{id:"foo", uri:"x"}` returns the same `HyperlinkId`. (Used internally so adjacent runs with the same `(id, uri)` share a slot — *not* a behavior the lookup layer exposes; see Follow-ups for cross-run grouping.)
- Different URIs (regardless of `id`) produce different `HyperlinkId`s — `(id, uri)` is the registry key.
- `Cell::set_hyperlink_id`/`hyperlink_id` round-trip; `CellExtra` allocation only occurs when first set; cell reset clears the slot.

**Unit tests — `FlatStorage`** (`flat_storage/mod_tests.rs`).
- Writing 100 cells under one active id RLE-collapses to one `AttributeMap` entry.
- Removing a row that ended a span doesn't bleed the active id into later writes.

**Unit tests — model lookups.** `hyperlink_at_point` returns the contiguous run of cells around `point` that share the same `HyperlinkId`, expanding left and right while the next cell is adjacent and carries the same id → invariants 5, 10. The lookup explicitly does **not** jump across non-adjacent cells even when `HyperlinkId` matches; cross-run grouping is out of scope.

**Integration tests** (`crates/integration/`, following the patterns in the `warp-integration-test` skill).
- **`osc8_open_close.rs`** — pipe an OSC 8 open + visible text + close to a fake PTY, assert the cells carry a hyperlink, hover one, observe `PointingHand` cursor and tooltip showing the URI → invariants 1, 5, 17.
- **`osc8_cmd_click_opens_url.rs`** — same setup, simulate Cmd+click on a hyperlinked cell, assert `ctx.open_url` was called with the URI (via the validated path) and that telemetry fired; simulate plain click and assert it was *not* called → invariants 6, 7.
- **`osc8_implicit_close_at_block_boundary.rs`** — open a hyperlink before a `precmd` / new prompt, assert the next block's cells do not carry the hyperlink → invariant 10.
- **`osc8_soft_wrap_keeps_one_span.rs`** — open a hyperlink whose visible text crosses a soft wrap; assert hover on either wrapped row highlights the full contiguous span and a single Cmd+click anywhere on the span opens the URI → invariants 5, 10. (Replaces the dropped non-contiguous `id` grouping test, which is moved to Follow-ups.)
- **`osc8_copy_text_vs_link.rs`** — select across a hyperlink and copy: clipboard contains visible text. Right-click → "Copy link": clipboard contains the URI → invariant 8.
- **`osc8_share_as_markdown.rs`** — share/copy-as-markdown produces `[visible](uri)` → invariant 13.
- **`osc8_disallowed_scheme_inert.rs`** — an OSC 8 span with `javascript:` URI does not navigate on click; tooltip shows "Scheme not allowed: javascript". Right-click → "Copy link" still copies the literal URI → invariants 15, 16.
- **`osc8_unparseable_uri_inert.rs`** — an OSC 8 span with a URI that fails URL parsing (e.g. `not a url`) is hoverable, copyable, and inert on click; tooltip shows "Cannot open: URI is malformed" → invariant 15.
- **`osc8_no_regression_on_url_autodetect.rs`** — output without OSC 8 still hyperlinks via auto-detection, and the auto-detected click also goes through `open_validated` (regression test for layer 5a) → invariant 18.

**Manual verification (recorded in PR description with a short clip).**
- Run `printf '\e]8;;https://warp.dev\e\\Open Warp\e]8;;\e\\\n'` in a Warp block; hover, observe pointer; Cmd+click, observe browser open.
- Run `wizcli` (or any CLI that emits OSC 8 — `gcc`, `make`, modern `git`) and exercise the live link.
- Run `cat` on a file containing a hyperlink across a wrapped line to confirm reflow on resize keeps the click intact.
- Run a TUI in alt-screen mode that emits OSC 8 (e.g. `lazygit`) to confirm parity with block-list behavior.

## Risks and mitigations

- **Memory / DoS from the registry.** Designed in, not deferred — see "Bounded registry, no reclamation" in (3). The cap (4096 distinct entries × 4096-byte max URI ≈ 16 MB worst-case per block) plus the bounded URI byte length plus the registry's grid-scoped lifetime put a hard ceiling on the working set. Adopting no-reclaim avoids a class of refcount/use-after-free bugs across cell overwrite, RLE split/merge, scrollback eviction, reflow, and deserialization.
- **Cell-size budget.** `cell.rs:122` is explicit that growing `Cell` past 24 bytes is a 33% memory hit. The `HyperlinkId` lives in `CellExtra` exactly to avoid this; the only `Cell`-shaped change is to `CellExtra`'s box, which is already optional and pays only when present.
- **Security: `javascript:` / `data:` / unexpected schemes.** Centralized in (5a) — every code path that can be reached from terminal output goes through `check_open_scheme` / `open_validated` before any platform open call. The plan also closes the same gap for the existing auto-detected URL flow.
- **URIs containing `;`.** The vte parser splits OSC params on `;`; a URI with literal `;` arrives as multiple params. Mitigation: when more than two params follow `b"8"`, rejoin params from index 2 onward with `;` before parsing. Documented as part of (1).
- **Existing handler implementors not overriding `set_hyperlink`.** Default no-op means OSC 8 is silently dropped on those surfaces (e.g. `EarlyOutputHandler`). Acceptable: those surfaces don't render clickable output today either. They can be wired later without a behavior change for users.
- **Session-sharing protocol compatibility.** New `hyperlink_registry` field in the per-block payload uses `#[serde(default)]`, so older clients deserialize without error and just don't see clickable links. The CI matrix includes a "current client ↔ pinned-old client" round-trip to lock this in.

## Parallelization

- Layer (1) — parser + types — is a single warp_terminal-crate change with no app dependencies.
- Layer (2) — Handler hook + dispatch — depends on (1) only.
- Layer (3a) and (3b) can run in parallel after (1). (3c) depends on (2) and the chosen (3a/3b) for the surface it covers.
- Layer (4) depends on (3).
- Layer (5) depends on (4).
- Layer (5a) — scheme allow-list — depends on nothing else and can land first as a hardening change to the existing auto-detected URL click path. Doing it first means (5)'s `Hyperlink` arm has the validator already in place.
- Layer (6a) — session persistence — depends on (3) (the on-cell field shape) and on the session-sharing protocol crate. Independent of (4)/(5).
- Layer (7) (sharing, AI context) depends on (3) but is otherwise independent of (4)/(5)/(6a).

The natural agent split is: one agent on (5a) (lands independently, no dependency on the rest); one on (1)+(2)+parser tests; one on (3a); one on (3b); then one each on (4), (5), (6a), (7).

## Follow-ups

- **Cross-run id grouping.** Treating two non-contiguous OSC 8 emissions with the same `id` as one logical link span. Requires a multi-range link type (something like `HyperlinkSpan { id: HyperlinkId, ranges: Vec<RangeInclusive<Point>> }`) and a generalization of `GridHighlightedLink` and the hover/click code to operate on a set of ranges. Deferred because (a) the common case is a single contiguous span, (b) Warp's existing `Link` and the highlighted-link UI assume one contiguous range, and (c) the user-facing benefit is small relative to the breadth of code that would change.
- **Underline-on-hover styling** for OSC 8 spans. Today the spec defers to existing SGR styling. Once the hover state is wired, an additional `Flags::HYPERLINK_HOVER` flag plus a small render change is a clean follow-on.
- **Byte-exact OSC 8 round-trip on copy.** Today the byte-copy path emits a normalized OSC 8 wrapper (see (7)). If we later decide preserving original bytes (params, terminator choice, custom keys) matters, we'd add a per-span byte buffer to the registry. Out of scope for v1.
- **Outgoing OSC 8.** Emitting hyperlinks from Warp's own UI when piping output through the terminal is out of scope for this issue.
