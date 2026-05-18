use super::*;

#[test]
fn absolute_line_numbers_default_to_one_based_values() {
    assert_eq!(absolute_line_number(LineCount::from(0), None), 1);
    assert_eq!(absolute_line_number(LineCount::from(4), None), 5);
}

#[test]
fn absolute_line_numbers_honor_starting_line_number() {
    assert_eq!(absolute_line_number(LineCount::from(0), Some(10)), 10);
    assert_eq!(absolute_line_number(LineCount::from(4), Some(10)), 14);
}

#[test]
fn relative_line_numbers_show_absolute_value_on_active_line() {
    assert_eq!(
        display_line_number(
            LineCount::from(4),
            CodeEditorLineNumberMode::Relative,
            None,
            Some(LineCount::from(4)),
        ),
        5
    );
}

#[test]
fn relative_line_numbers_show_distance_above_and_below_active_line() {
    assert_eq!(
        display_line_number(
            LineCount::from(2),
            CodeEditorLineNumberMode::Relative,
            None,
            Some(LineCount::from(5)),
        ),
        3
    );
    assert_eq!(
        display_line_number(
            LineCount::from(8),
            CodeEditorLineNumberMode::Relative,
            None,
            Some(LineCount::from(5)),
        ),
        3
    );
}

#[test]
fn relative_line_numbers_fall_back_to_absolute_without_active_line() {
    assert_eq!(
        display_line_number(
            LineCount::from(4),
            CodeEditorLineNumberMode::Relative,
            None,
            None,
        ),
        5
    );
}

#[test]
fn relative_line_numbers_use_starting_line_number_for_active_line_only() {
    assert_eq!(
        display_line_number(
            LineCount::from(4),
            CodeEditorLineNumberMode::Relative,
            Some(10),
            Some(LineCount::from(4)),
        ),
        14
    );
    assert_eq!(
        display_line_number(
            LineCount::from(1),
            CodeEditorLineNumberMode::Relative,
            Some(10),
            Some(LineCount::from(4)),
        ),
        3
    );
}
