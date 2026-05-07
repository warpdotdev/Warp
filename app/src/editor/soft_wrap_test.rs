use super::*;
use vec1::vec1;

fn line_with_final_two_char_ligature() -> text_layout::Line {
    text_layout::Line {
        width: 20.,
        trailing_whitespace_width: 0.,
        runs: vec![text_layout::Run {
            font_id: warpui::fonts::FontId(0),
            glyphs: vec![text_layout::Glyph {
                id: Default::default(),
                position_along_baseline: Default::default(),
                index: 0,
                width: 20.,
            }],
            styles: text_layout::TextStyle::new(),
            width: 20.,
        }],
        font_size: 10.,
        line_height_ratio: 1.,
        baseline_ratio: text_layout::DEFAULT_TOP_BOTTOM_RATIO,
        clip_config: None,
        ascent: 8.,
        descent: 2.,
        caret_positions: vec![text_layout::CaretPosition {
            position_in_line: 0.,
            start_offset: 0,
            last_offset: 1,
        }],
        chars_with_missing_glyphs: vec![],
    }
}

#[test]
fn test_singleton_frames_displayed_lines() {
    let frame_layouts = FrameLayouts {
        frames: vec![
            Arc::new(text_layout::TextFrame::empty(12., 1.)),
            Arc::new(text_layout::TextFrame::empty(13., 1.)),
            Arc::new(text_layout::TextFrame::empty(14., 1.)),
            Arc::new(text_layout::TextFrame::empty(15., 1.)),
            Arc::new(text_layout::TextFrame::empty(16., 1.)),
        ],
        start_line: 2,
        end_line: 3,
    };

    assert_eq!(frame_layouts.displayed_lines().count(), 1);

    let mut iter = frame_layouts.displayed_lines();
    assert_eq!(iter.next().expect("Should have line").font_size, 14.);
}

#[test]
fn test_displayed_lines_end_line_greater_than_iterator_size() {
    let frame_layouts = FrameLayouts {
        frames: vec![
            Arc::new(text_layout::TextFrame::empty(12., 1.)),
            Arc::new(text_layout::TextFrame::empty(13., 1.)),
            Arc::new(text_layout::TextFrame::empty(14., 1.)),
            Arc::new(text_layout::TextFrame::empty(15., 1.)),
            Arc::new(text_layout::TextFrame::empty(16., 1.)),
        ],
        start_line: 3,
        end_line: 6,
    };

    assert_eq!(frame_layouts.displayed_lines().count(), 2);

    let mut iter = frame_layouts.displayed_lines();
    assert_eq!(iter.next().expect("Should have line").font_size, 15.);
    assert_eq!(iter.next().expect("Should have line").font_size, 16.);
    assert!(iter.next().is_none());
}

#[test]
fn test_soft_wrapped_frame_displayed_lines() {
    let frame_layouts = FrameLayouts {
        frames: vec![
            Arc::new(text_layout::TextFrame::new(
                vec1![
                    text_layout::Line::empty(10., 1., 0),
                    text_layout::Line::empty(11., 1., 1),
                    text_layout::Line::empty(12., 1., 2),
                ],
                0.,
                Default::default(),
            )),
            Arc::new(text_layout::TextFrame::empty(13., 1.)),
            Arc::new(text_layout::TextFrame::empty(14., 1.)),
            Arc::new(text_layout::TextFrame::empty(15., 1.)),
            Arc::new(text_layout::TextFrame::empty(16., 1.)),
        ],
        start_line: 2,
        end_line: 5,
    };

    assert_eq!(frame_layouts.displayed_lines().count(), 3);

    let mut iter = frame_layouts.displayed_lines();
    assert_eq!(iter.next().expect("Should have line").font_size, 12.);
    assert_eq!(iter.next().expect("Should have line").font_size, 13.);
    assert_eq!(iter.next().expect("Should have line").font_size, 14.);
    assert!(iter.next().is_none());
}

#[test]
fn test_soft_wrap_point_uses_caret_end_for_ligature_line() {
    let frame_layouts = FrameLayouts {
        frames: vec![Arc::new(text_layout::TextFrame::new(
            vec1![
                line_with_final_two_char_ligature(),
                text_layout::Line::empty(10., 1., 2),
            ],
            20.,
            Default::default(),
        ))],
        start_line: 0,
        end_line: 2,
    };

    let soft_wrap_point = frame_layouts
        .to_soft_wrap_point(DisplayPoint::new(0, 2), ClampDirection::Up)
        .expect("ligature end should be within the first soft-wrapped line");

    assert_eq!(soft_wrap_point.row(), 0);
    assert_eq!(soft_wrap_point.column(), 2);
}

#[test]
fn test_display_point_marks_ligature_line_end_as_clamped_up() {
    let frame_layouts = FrameLayouts {
        frames: vec![Arc::new(text_layout::TextFrame::new(
            vec1![line_with_final_two_char_ligature()],
            20.,
            Default::default(),
        ))],
        start_line: 0,
        end_line: 1,
    };

    let display_point = frame_layouts.to_display_point(SoftWrapPoint::new(0, 2));

    assert_eq!(display_point.point, DisplayPoint::new(0, 2));
    assert_eq!(display_point.clamp_direction, ClampDirection::Up);
}
