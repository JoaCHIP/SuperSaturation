//! The SuperSaturation colour transform, ported from `PrecalcParameters()` and `ProcessPixel()` in
//! the original Photoshop filter (`common/Dissolve.cpp` in github.com/JoaCHIP/SuperSaturation).
//!
//! The port keeps the original's single-precision maths and constants, so its output matches the
//! Photoshop version to within float rounding (`tests/reference.rs` checks it against the original
//! C++: the largest difference is far below one 16-bit level). The deliberate differences, none of
//! which changes a visible result:
//!
//! * Speed (the host interprets the plug-in, so every instruction counts):
//!   - A power whose exponent is exactly 1 (a slider at its neutral position) is skipped:
//!     `x^1 == x` exactly.
//!   - The two powers applied to saturation are merged: `(s^a)^b == s^(a·b)`.
//!   - The hue is not turned into an angle with `atan2` and back with three `cos`: the same three
//!     values come from the colour's direction vector, rotated by the hue shift. `cos(h ± 120°)`
//!     expand to sums of products with `cos h` and `sin h`, which are the normalised vector.
//!   - `powf` and `sqrtf` are the interpreter-friendly ones in [`crate::mathf`].
//! * Red and blue are clamped at 0 before the Warmth power. The original raised a negative value to
//!   a fractional power there, which gives NaN; Photoshop then stored NaN as 0. Clamping first gives
//!   that same 0 (PhotoCraft would otherwise keep the pixel's original value for a NaN sample).

use crate::mathf::{atan2f, cosf, powf, sinf, sqrtf};

/// The original's value of π (`3.141593f`), kept rather than `f32::consts::PI` so the results match.
#[allow(clippy::approx_constant)]
const PI: f32 = 3.141593;
/// The original's value of 120° in radians (`2.094395f`).
const THIRD_TURN: f32 = 2.094395;

/// The six slider values, in the original filter's units ([`crate::params`] maps the dialog's
/// onto these).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// 0 to 200; neutral 100. The dialog shows it as -100 to 100, neutral 0.
    pub saturation: f32,
    /// -100 to 100; neutral 0.
    pub liveliness: f32,
    /// -100 to 100; neutral 0.
    pub gamma: f32,
    /// -100 to 100 in the original; neutral 0. The dialog's -100 to 100 maps to about -175 to 200.
    pub contrast: f32,
    /// -100 to 100; neutral 0.
    pub warmth: f32,
    /// -180 to 180 degrees; neutral 0.
    pub hue_shift: f32,
}

impl Default for Settings {
    /// The neutral position of every slider (the original's `InitParameters()`).
    fn default() -> Self {
        Self { saturation: 100.0, liveliness: 0.0, gamma: 0.0, contrast: 0.0, warmth: 0.0, hue_shift: 0.0 }
    }
}

/// Per-run constants derived from [`Settings`]: the original's `TempParameters`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Precalc {
    pub saturation: f32,
    pub liveliness: f32,
    pub gamma1: f32,
    pub gamma2: f32,
    pub contrast: f32,
    pub warmth: f32,
    pub hue_shift: f32,
    /// `1 / warmth`, the exponent applied to red.
    pub(crate) warmth_inv: f32,
    /// `liveliness · gamma2`: both powers applied to saturation at once.
    pub(crate) sat_exp: f32,
    /// `cos`/`sin` of the hue shift.
    pub(crate) cos_shift: f32,
    pub(crate) sin_shift: f32,
    /// `cos`/`sin` of ±120°, which the original evaluated for every pixel.
    cos_pos: f32,
    sin_pos: f32,
    cos_neg: f32,
    sin_neg: f32,
}

impl Precalc {
    /// The original's `PrecalcParameters()`, plus constants that save work per pixel.
    pub fn new(s: &Settings) -> Self {
        let gamma1 = (200.0 - s.gamma) / 400.0;
        let gamma2 = (400.0 - s.gamma) / 800.0;
        let warmth = (s.warmth + 100.0) / 200.0;
        let warmth = ((warmth * warmth) + 0.25) * 2.0;
        let liveliness = powf((200.0 - s.liveliness) / 200.0, 2.0);
        let gamma2 = gamma2 * gamma2 * 4.0;
        let hue_shift = s.hue_shift * PI / 180.0;
        Self {
            saturation: s.saturation * s.saturation / 30000.0,
            liveliness,
            gamma1: gamma1 * gamma1 * 4.0,
            gamma2,
            contrast: s.contrast / 100.0,
            warmth,
            hue_shift,
            warmth_inv: 1.0 / warmth,
            sat_exp: liveliness * gamma2,
            cos_shift: cosf(hue_shift),
            sin_shift: sinf(hue_shift),
            cos_pos: cosf(THIRD_TURN),
            sin_pos: sinf(THIRD_TURN),
            cos_neg: cosf(-THIRD_TURN),
            sin_neg: sinf(-THIRD_TURN),
        }
    }
}

impl Precalc {
    /// Whether Gamma and Contrast leave the brightness alone ([`tone`] is the identity).
    pub fn tone_is_identity(&self) -> bool {
        self.gamma1 == 1.0 && !self.applies_contrast()
    }

    /// The original skips contrast up to 1 %.
    #[inline(always)]
    fn applies_contrast(&self) -> bool {
        self.contrast < -0.01 || self.contrast > 0.01
    }
}

/// `base^exp`, skipping the call when `exp` is exactly 1 (where it returns `base` unchanged).
#[inline(always)]
pub(crate) fn pow_unless_one(base: f32, exp: f32) -> f32 {
    if exp == 1.0 { base } else { powf(base, exp) }
}

/// Gamma, then Contrast, on the brightness (`val`) alone: the part of [`process_pixel`] that
/// Grayscale and Lab documents share with RGB ones.
#[inline(always)]
pub fn tone(val: f32, t: &Precalc) -> f32 {
    let mut val = pow_unless_one(val, t.gamma1);
    if t.applies_contrast() {
        let contrast = -cosf(val * PI) * 0.5 + 0.5;
        val = contrast * t.contrast + (1.0 - t.contrast) * val;
    }
    val
}

/// The original's `ProcessPixel()`: one RGB colour (0–1 per channel) in, one out. The result is not
/// clamped: the host clamps integer-depth documents to 0–1 when it stores them.
#[inline]
pub fn process_pixel(rgb: [f32; 3], t: &Precalc) -> [f32; 3] {
    let [mut r, mut g, mut b] = rgb;

    // To "superHSV": brightness is the plain mean, saturation the summed pairwise chroma lengths.
    let val = (r + g + b) / 3.0;
    r -= val;
    g -= val;
    b -= val;
    let difrg = sqrtf(r * r + g * g);
    let difrb = sqrtf(r * r + b * b);
    let difgb = sqrtf(g * g + b * b);
    let mut sat = (difrg + difrb + difgb) * t.saturation;

    // The original: sat = sat^liveliness; val = val^gamma1; sat = sat^gamma2; then contrast on val.
    sat = pow_unless_one(sat, t.sat_exp);
    let val = tone(val, t);

    let mut mx = r;
    let mut my = 0.0;
    mx += g * t.cos_pos;
    my += g * t.sin_pos;
    mx += b * t.cos_neg;
    my += b * t.sin_neg;

    // Back from superHSV. The original: hue = atan2(-my, -mx) + hue_shift, then
    // nr = -cos(hue), ng = -cos(hue - 120°), nb = -cos(hue + 120°).
    let len = sqrtf(mx * mx + my * my);
    let (mut nr, mut ng, mut nb);
    if len > 0.0 {
        // cos/sin of atan2(-my, -mx) are the normalised (-mx, -my); rotate by the hue shift.
        let (c, s) = (-mx / len, -my / len);
        let ch = c * t.cos_shift - s * t.sin_shift;
        let sh = s * t.cos_shift + c * t.sin_shift;
        nr = -ch;
        ng = -(ch * t.cos_pos + sh * t.sin_pos);
        nb = -(ch * t.cos_pos - sh * t.sin_pos);
    } else {
        // No direction (an exact grey, or a chroma too small to square): the original's way.
        let hue = atan2f(-my, -mx) + t.hue_shift;
        nr = -cosf(hue);
        ng = -cosf(hue - THIRD_TURN);
        nb = -cosf(hue + THIRD_TURN);
    }

    let gr = (nr + ng + nb) / 3.0;
    nr = gr * (1.0 - sat) + nr * sat;
    ng = gr * (1.0 - sat) + ng * sat;
    nb = gr * (1.0 - sat) + nb * sat;

    nr += val;
    ng += val;
    nb += val;

    // Warmth. See the module docs for the clamp.
    if t.warmth != 1.0 {
        nr = powf(nr.max(0.0), t.warmth_inv);
        nb = powf(nb.max(0.0), t.warmth);
    }

    [nr, ng, nb]
}

/// CMYK's C, M and Y inks (0 no ink, 1 full ink) through [`process_pixel`], as the RGB colour they
/// leave on white paper (`1 − ink`), and back to inks. K is left to [`tone_ink`], so the black
/// separation keeps its own shape.
#[inline(always)]
pub fn process_cmy([c, m, y]: [f32; 3], t: &Precalc) -> [f32; 3] {
    let [r, g, b] = process_pixel([1.0 - c, 1.0 - m, 1.0 - y], t);
    [1.0 - r, 1.0 - g, 1.0 - b]
}

/// One ink channel (CMYK's K) as Grayscale treats a grey: Gamma and Contrast on its brightness
/// (`1 − ink`), so Gamma's lightening takes ink away.
#[inline(always)]
pub fn tone_ink(ink: f32, t: &Precalc) -> f32 {
    1.0 - tone(1.0 - ink, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_settings_make_every_exponent_exactly_one() {
        let t = Precalc::new(&Settings::default());
        assert_eq!((t.liveliness, t.gamma1, t.gamma2, t.warmth, t.sat_exp), (1.0, 1.0, 1.0, 1.0, 1.0));
        assert_eq!(t.contrast, 0.0);
        assert_eq!(t.hue_shift, 0.0);
    }

    #[test]
    fn neutral_settings_keep_greys() {
        // Colours move slightly even at the neutral settings (the original's colour model is not an
        // exact round trip; tests/reference.rs checks that against the original), but greys stay.
        let t = Precalc::new(&Settings::default());
        for v in [0.0, 0.1, 0.5, 0.731, 1.0] {
            for out in process_pixel([v, v, v], &t) {
                assert!((out - v).abs() < 1e-6, "{v} -> {out}");
            }
        }
    }

    #[test]
    fn zero_saturation_gives_grey_of_the_mean() {
        let t = Precalc::new(&Settings { saturation: 0.0, ..Settings::default() });
        let out = process_pixel([0.9, 0.3, 0.0], &t);
        for v in out {
            assert!((v - 0.4).abs() < 1e-6, "{out:?}");
        }
    }

    #[test]
    fn warmth_never_produces_nan() {
        // Dark, saturated colours push red/blue below 0 before the Warmth power.
        for warmth in [-100.0, -37.0, 42.0, 100.0] {
            let t = Precalc::new(&Settings { saturation: 200.0, warmth, ..Settings::default() });
            for c in [[0.0, 0.0, 0.3], [0.3, 0.0, 0.0], [0.05, 0.2, 0.0]] {
                let out = process_pixel(c, &t);
                assert!(out.iter().all(|v| v.is_finite()), "{warmth} {c:?} -> {out:?}");
            }
        }
    }

    #[test]
    fn warmth_leans_orange_or_blue() {
        let grey = [0.5, 0.5, 0.5];
        let warm = process_pixel(grey, &Precalc::new(&Settings { warmth: 60.0, ..Settings::default() }));
        let cool = process_pixel(grey, &Precalc::new(&Settings { warmth: -60.0, ..Settings::default() }));
        assert!(warm[0] > warm[1] && warm[1] > warm[2], "{warm:?}");
        assert!(cool[2] > cool[1] && cool[1] > cool[0], "{cool:?}");
    }

    #[test]
    fn gamma_brightens_and_darkens() {
        let mid = [0.5, 0.4, 0.3];
        let mean = |c: [f32; 3]| (c[0] + c[1] + c[2]) / 3.0;
        let bright = process_pixel(mid, &Precalc::new(&Settings { gamma: 50.0, ..Settings::default() }));
        let dark = process_pixel(mid, &Precalc::new(&Settings { gamma: -50.0, ..Settings::default() }));
        assert!(mean(bright) > mean(mid) && mean(dark) < mean(mid));
    }

    #[test]
    fn tone_is_what_the_rgb_transform_does_to_greys() {
        // Grayscale documents use tone() alone: the same as running a grey through process_pixel
        // with Warmth neutral (the other sliders do nothing to greys), then taking (R + 2G + B) / 4.
        for (gamma, contrast) in [(0.0, 0.0), (40.0, 0.0), (-70.0, 25.0), (100.0, -100.0), (-100.0, 100.0), (0.0, 1.0)] {
            let s = Settings { saturation: 170.0, liveliness: 60.0, gamma, contrast, hue_shift: 77.0, ..Settings::default() };
            let t = Precalc::new(&s);
            for i in 0..=64 {
                let v = i as f32 / 64.0;
                let [r, g, b] = process_pixel([v; 3], &t);
                let via_rgb = (r + g + g + b) * 0.25;
                assert!((tone(v, &t) - via_rgb).abs() < 1e-6, "{s:?} {v}: {} vs {via_rgb}", tone(v, &t));
            }
            assert_eq!(t.tone_is_identity(), gamma == 0.0 && contrast.abs() <= 1.0);
        }
    }

    #[test]
    fn full_hue_turn_is_identity() {
        let c = [0.8, 0.3, 0.1];
        let a = process_pixel(c, &Precalc::new(&Settings { hue_shift: 180.0, ..Settings::default() }));
        let b = process_pixel(c, &Precalc::new(&Settings { hue_shift: -180.0, ..Settings::default() }));
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} {b:?}");
        }
    }
}
