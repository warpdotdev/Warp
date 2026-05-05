# Test Fixtures

This directory contains test files used for manual testing and validation of the markdown editor.

## Images

The `images/` directory contains sample images and a test markdown file (`image_test.md`) that demonstrates various image rendering scenarios:

- Relative paths (`./sample1.jpg`)
- Parent directory references (`../parent_test.jpg`)
- Absolute paths
- Different image formats (JPG, PNG)
- Images in lists
- Empty alt text

To test image rendering, open `images/image_test.md` in Warp.

## ToC Anchors

`toc_anchor_test.md` covers manual validation for Markdown table-of-contents fragment links, including punctuation normalization, duplicate headings, natural suffix collisions, and long-document scrolling.

To test anchor navigation, open `toc_anchor_test.md` in Warp's Markdown viewer and click the table-of-contents links.
