# Markdown ToC Anchor Manual Test

Use this file in a Warp notebook/editor test flow to verify Markdown fragment links.

## Table of contents

- [Basic heading](#basic-heading)
- [Heading with punctuation](#heading-with-punctuation)
- [Duplicate heading](#duplicate-heading)
- [Duplicate heading again](#duplicate-heading-1)
- [Natural suffix heading](#duplicate-heading-2)
- [Mixed case and symbols](#mixed-case--symbols)
- [Bottom target](#bottom-target)

## Basic heading

Expected: clicking `Basic heading` in selectable/read-only mode scrolls here. In editable mode, a normal click should show the link tooltip/editor instead of immediately scrolling.

## Heading with punctuation!

Expected: punctuation is normalized out, so `#heading-with-punctuation` scrolls here.

## Duplicate heading

Expected: the first duplicate target resolves to `#duplicate-heading`.

## Duplicate heading

Expected: the second duplicate target resolves to `#duplicate-heading-1`.

## Duplicate heading-1

Expected: this natural suffix heading should not steal `#duplicate-heading-1`; it should resolve as `#duplicate-heading-2`.

## Mixed CASE & Symbols

Expected: mixed case and symbols normalize to `#mixed-case--symbols`.

## Scroll padding section 1

This filler makes scrolling visible.

## Scroll padding section 2

This filler makes scrolling visible.

## Scroll padding section 3

This filler makes scrolling visible.

## Scroll padding section 4

This filler makes scrolling visible.

## Scroll padding section 5

This filler makes scrolling visible.

## Bottom target

Expected: clicking `Bottom target` from the TOC scrolls near the bottom of the document.
