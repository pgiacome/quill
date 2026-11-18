//! The headroom handed to the HDR still tone map, which runs in the vendored
//! Metal and DirectX shaders (`third_party/gpui-pre-apple/src/shaders.metal`,
//! `third_party/gpui-pre-windows/src/shaders.hlsl`).

/// The headroom to hand the shader for content peaking at `peak` on a
/// display with `display` headroom. Content that fits under the display
/// draws untouched (the knee lands at the content's own peak); brighter
/// content rolls off into the display's range.
pub(crate) fn shader_headroom(display: f32, peak: f32) -> f32 {
    let display = display.max(1.0);
    if peak <= display {
        (peak / 0.8).max(display)
    } else {
        display
    }
}

#[cfg(test)]
mod tests {
    use super::shader_headroom;

    #[test]
    fn content_under_the_display_draws_untouched() {
        // The knee (80% of the headroom) lands at the content's own peak.
        assert!((shader_headroom(4.0, 3.9) - 4.875).abs() < 1e-4);
        assert_eq!(shader_headroom(16.0, 3.0), 16.0);
    }

    #[test]
    fn brighter_content_rolls_off_into_the_display() {
        assert_eq!(shader_headroom(2.0, 8.0), 2.0);
        assert_eq!(shader_headroom(0.5, 8.0), 1.0);
    }
}
