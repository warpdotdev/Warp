use super::*;

#[test]
fn image_rect_returns_none_for_nan_origin() {
    assert!(image_rect(
        vec2f(164.0, 164.0),
        vec2f(f32::NAN, 874.725),
        vec2f(163.75, 163.75),
        false,
        false,
    )
    .is_none());
}
