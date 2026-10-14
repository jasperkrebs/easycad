//! The extrusion bead model, identical to EasyGCode's (`gcode.js`, `beadArea`).

use std::f64::consts::PI;

/// Cross-section area (mm²) of one extruded bead, modelled as a stadium: a rectangle with a
/// half circle on each end, `width` wide and `height` tall.
///
/// `(w − h)·h + π·(h/2)²`, with the width clamped to at least the height (a bead can't be
/// narrower than it is tall). Multiply by a segment's length to get its volume in mm³.
pub fn bead_area(width: f64, height: f64) -> f64 {
    let w = width.max(height);
    (w - height) * height + PI * (height / 2.0).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_easygcode_formula() {
        // 0.6 × 0.3 bead: (0.6 − 0.3)·0.3 + π·0.15² = 0.09 + 0.070686
        assert!((bead_area(0.6, 0.3) - 0.160_685_834_7).abs() < 1e-9);
    }

    #[test]
    fn round_bead_when_width_equals_height() {
        assert!((bead_area(0.4, 0.4) - PI * 0.04).abs() < 1e-12);
    }

    #[test]
    fn width_is_clamped_to_height() {
        assert_eq!(bead_area(0.1, 0.4), bead_area(0.4, 0.4));
    }
}
