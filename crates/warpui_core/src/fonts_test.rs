use super::*;

#[test]
fn test_subpixel_alignment_computation() {
    // STEPS = 4. Bucket offsets are 0.0, 0.25, 0.5, 0.75. The bucket index
    // is round(fract * 4) % 4.

    {
        // Default case: a non-fractional offset has alignment zero.
        let pos = vec2f(0., 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 0);
    }
    {
        // 0.1 * 4 = 0.4, rounds to 0.
        let pos = vec2f(0.1, 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 0);
    }
    {
        // y-position never participates: only the horizontal fractional
        // remainder is bucketed.
        let pos = vec2f(0.1, 0.33);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 0);
    }
    {
        // 0.2 * 4 = 0.8, rounds to 1 -> 0.25 offset.
        let pos = vec2f(0.2, 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 1);
    }
    {
        // 0.5 * 4 = 2.0, rounds to 2 -> 0.5 offset.
        let pos = vec2f(0.5, 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 2);
    }
    {
        // 0.66 * 4 = 2.64, rounds to 3 -> 0.75 offset.
        let pos = vec2f(0.66, 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 3);
    }
    {
        // 0.9 * 4 = 3.6, rounds to 4, then % 4 wraps back to 0.
        let pos = vec2f(0.9, 0.);
        let alignment = SubpixelAlignment::new(pos);
        assert_eq!(alignment.0, 0);
    }
}
