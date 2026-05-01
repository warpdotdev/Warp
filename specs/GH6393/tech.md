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

The implementation splits into five layers, listed bottom-up. Each layer can be merged independently behind the `OscHyperlinks` feature flag described in (6).

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

**Same-id grouping (product invariant 5).** Two `set_hyperlink` calls with the same non-empty `id` resolve to the same `HyperlinkId` via the registry's `intern`, so non-adjacent runs sharing an `id` answer "yes" to a `same-link?` query trivially.

### 4. Lookup helpers — model layer

Mirror the auto-detected URL flow:

- `GridHandler::hyperlink_at_point(&self, point: Point) -> Option<Link>` (`grid_handler.rs`, alongside the existing `url_at_point` at line 596). Returns the `Link` (range over cells) for the OSC 8 span at `point`, expanded by walking left/right while the cell's `HyperlinkId` matches. The returned `Link` is the existing struct; OSC 8 reuses it.
- `GridHandler::hyperlink_uri_at_point(&self, point: Point) -> Option<&str>` to read the URI without recovering it from cell text.
- Block / alt-screen / model-level wrappers paralleling `blocks.rs:2460`, `alt_screen.rs:285`, `terminal_model.rs:1844` (`hyperlink_at_point`, `hyperlink_uri_at_range`).

### 5. View layer — hover, click, copy, context menu

`app/src/terminal/view/link_detection.rs`:

- Extend `GridHighlightedLink` with a third variant: `Hyperlink(WithinModel<Link>, String /* uri */)`. The variant carries the URI directly because, unlike `Url`, it is not recoverable from the cell text.
- In the hover state machine (lines 299-339), call `model.hyperlink_at_point(position)` first; if it returns `Some`, set `GridHighlightedLink::Hyperlink(...)`. Only fall through to `url_at_point` (and the file-path scanner) when no OSC 8 span is found at that point — this implements product invariant 9 (OSC 8 wins over auto-detected URL on the same cell).
- In the click handler (lines 391-395), add the `Hyperlink` arm: `ctx.open_url(uri)`. Same telemetry path (`TelemetryEvent::OpenLink`), same scheme allow-list — implemented by routing `ctx.open_url` through the existing helper that auto-detected URLs use today, plus a new pre-check that drops disallowed schemes (product invariant 16).
- Tooltip text: `GridHighlightedLink::tooltip_text` (line 63) returns "Open link" for the new variant.

`app/src/terminal/view.rs`:

- The `OpenGridLink(link)` action (line 24786) gets a new arm for `Hyperlink` that calls `ctx.open_url(uri)`.
- The right-click context menu wiring around line 15040 gets a `GridHighlightedLink::Hyperlink` branch with "Open link" / "Copy link" items that operate on the URI.

### 6. Feature flag

Behind `FeatureFlag::OscHyperlinks` (added per `WARP.md`'s feature-flag guide, defaulted on for dogfood). All five layers gate on the flag at the dispatch boundary in (2): when off, `osc_dispatch`'s `b"8"` arm calls `unhandled(params)` and the rest of the pipeline never sees an OSC 8 hyperlink. This lets each layer land independently and be reverted in one place if a regression appears.

### 7. Sharing, copy-as-markdown, and AI context

- **Markdown sharing** (product invariant 13). The block→markdown serializer (search for `to_markdown` / shared-session export) emits `[visible text](URI)` for spans that carry a `HyperlinkId`. For "copy block" (raw bytes), the serializer round-trips the original OSC 8 sequences from the registry — bytes go in, bytes go out.
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
    View->>Browser: ctx.open_url("https://x")
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
- `intern` deduplicates: same `Hyperlink{id:"foo", uri:"x"}` returns the same `HyperlinkId`. → invariant 5.
- Different URIs with the same `id` produce different `HyperlinkId`s (`id` is a hint, not a key — `(id, uri)` is the key).
- `Cell::set_hyperlink_id`/`hyperlink_id` round-trip; `CellExtra` allocation only occurs when first set; cell reset clears the slot.

**Unit tests — `FlatStorage`** (`flat_storage/mod_tests.rs`).
- Writing 100 cells under one active id RLE-collapses to one `AttributeMap` entry.
- Removing a row that ended a span doesn't bleed the active id into later writes.

**Unit tests — model lookups.** `hyperlink_at_point` returns the full contiguous run of cells that share the same `HyperlinkId`, including across non-adjacent cells when their `id` matches → invariants 5, 10.

**Integration tests** (`crates/integration/`, following the patterns in the `warp-integration-test` skill).
- **`osc8_open_close.rs`** — pipe an OSC 8 open + visible text + close to a fake PTY, assert the cells carry a hyperlink, hover one, observe `PointingHand` cursor and tooltip showing the URI → invariants 1, 5, 17.
- **`osc8_cmd_click_opens_url.rs`** — same setup, simulate Cmd+click on a hyperlinked cell, assert `ctx.open_url` was called with the URI; simulate plain click and assert it was *not* called → invariants 6, 7.
- **`osc8_implicit_close_at_block_boundary.rs`** — open a hyperlink before a `precmd` / new prompt, assert the next block's cells do not carry the hyperlink → invariant 10.
- **`osc8_split_id_grouping.rs`** — open with `id=foo`, write text, close, open again with `id=foo` and the same URI, write more text, close. Hover the second run; assert highlight covers both runs → invariant 5.
- **`osc8_copy_text_vs_link.rs`** — select across a hyperlink and copy: clipboard contains visible text. Right-click → "Copy link": clipboard contains the URI → invariant 8.
- **`osc8_share_as_markdown.rs`** — share/copy-as-markdown produces `[visible](uri)` → invariant 13.
- **`osc8_disallowed_scheme_inert.rs`** — an OSC 8 span with `javascript:` URI does not navigate on click; tooltip shows literal URI → invariant 16.
- **`osc8_no_regression_on_url_autodetect.rs`** — output without OSC 8 still hyperlinks via auto-detection → invariant 18.

**Manual verification (recorded in PR description with a short clip).**
- Run `printf '\e]8;;https://warp.dev\e\\Open Warp\e]8;;\e\\\n'` in a Warp block; hover, observe pointer; Cmd+click, observe browser open.
- Run `wizcli` (or any CLI that emits OSC 8 — `gcc`, `make`, modern `git`) and exercise the live link.
- Run `cat` on a file containing a hyperlink across a wrapped line to confirm reflow on resize keeps the click intact.
- Run a TUI in alt-screen mode that emits OSC 8 (e.g. `lazygit`) to confirm parity with block-list behavior.

## Risks and mitigations

- **Memory overhead from the registry.** Bounded by deduplication and per-grid scoping; a registry entry is freed when its last referencing cell is overwritten or scrolled out (refcount on `HyperlinkRegistry::intern` / `forget`). Mitigation: spec a max registry size per block (e.g. 4096 unique URIs) and fall back to "don't track" past it.
- **Cell-size budget.** `cell.rs:122` is explicit that growing `Cell` past 24 bytes is a 33% memory hit. The `HyperlinkId` lives in `CellExtra` exactly to avoid this; the only `Cell`-shaped change is to `CellExtra`'s box, which is already optional and pays only when present.
- **Security: `javascript:` / `data:` / unexpected schemes.** Honored centrally via the existing scheme allow-list (invariant 16). The hyperlink layer does not introduce a new code path that bypasses it.
- **URIs containing `;`.** The vte parser splits OSC params on `;`; a URI with literal `;` arrives as multiple params. Mitigation: when more than two params follow `b"8"`, rejoin params from index 2 onward with `;` before parsing. Documented as part of (1).
- **Existing handler implementors not overriding `set_hyperlink`.** Default no-op means OSC 8 is silently dropped on those surfaces (e.g. `EarlyOutputHandler`). Acceptable: those surfaces don't render clickable output today either. They can be wired later without a behavior change for users.

## Parallelization

- Layer (1) — parser + types — is a single warp_terminal-crate change with no app dependencies.
- Layer (2) — Handler hook + dispatch — depends on (1) only.
- Layer (3a) and (3b) can run in parallel after (1). (3c) depends on (2) and the chosen (3a/3b) for the surface it covers.
- Layer (4) depends on (3).
- Layer (5) depends on (4).
- Layer (7) (sharing, AI context) depends on (3) but is otherwise independent of (4)/(5).

The natural agent split is one agent on (1)+(2)+parser tests, one on (3a) (FlatStorage), one on (3b) (Cell/CellExtra), then one each on (4), (5), and (7).

## Follow-ups

- Underline-on-hover styling for OSC 8 spans (today the spec defers to existing SGR styling). Once the hover state is wired, an additional `Flags::HYPERLINK_HOVER` is a small follow-on.
- Persisting OSC 8 state in serialized session snapshots (Warp Drive / shared sessions) requires extending the cell-serialization format. Doable in the same PR if the snapshot format already round-trips `CellExtra`; if not, deferred.
- Outgoing OSC 8: emitting hyperlinks from Warp's own UI when piping output through the terminal is out of scope for this issue.
