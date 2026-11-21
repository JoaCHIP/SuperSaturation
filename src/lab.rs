//! Lab documents: SuperSaturation worked out on the L*a*b* channels directly.
//!
//! The RGB transform ([`crate::algorithm::process_pixel`]) turns each colour into its own
//! "superHSV": a brightness (`val`, the mean of R, G and B), a hue (an angle on the colour circle)
//! and a saturation (how far the colour is from grey), adjusts those and turns them back into RGB.
//! Lab already has that shape, so this mode reads it off the channels instead of converting each
//! pixel to RGB and back:
//!
//! * L is `val`. Gamma and Contrast are the same curves as in RGB ([`tone`]).
//! * (a, b) is the colour's position on the colour circle, with (0, 0) grey: +a is a slightly
//!   magenta red, −a a slightly cyan green, +b yellow, −b blue. Its angle is the hue and its length
//!   (the chroma) the saturation. Hue Shift turns it by the slider's angle (in the same direction as
//!   in RGB: positive turns red towards yellow and green), and Saturation and Liveliness scale its
//!   length with the same formulas as in RGB, after converting chroma to the RGB transform's
//!   saturation units ([`CHROMA_TO_SAT`]).
//! * Warmth has no direct counterpart in Lab: in RGB it is a power curve on red and another on
//!   blue. Here every pixel gets the shift (in L, a and b) that the RGB Warmth gives a grey of the
//!   same lightness, so greys warm or cool exactly as in an RGB document.
//!
//! The results are close to running the filter on the same image in RGB mode, but not the same, on
//! purpose: a slider's overall strength matches, colour by colour it does not. Lab hues are spaced
//! differently from RGB ones (so a hue shift moves some colours further than others), and changing
//! the saturation here keeps L exactly, where in RGB it moves the lightness a little. Unlike the RGB
//! transform, the neutral settings leave every colour exactly as it is.

use crate::algorithm::{Precalc, Settings, tone};
use crate::mathf::{powf, sqrtf};

/// The host's a and b of a colourless pixel: it normalises a* and b* as `(v + 128) / 255`. The
/// host's own division gives this same `f32` for an 8- or 16-bit 128, so greys subtract to exactly 0.
pub const AB_ZERO: f32 = 128.0 / 255.0;

/// The RGB transform's saturation (at the neutral settings: the summed pairwise channel
/// differences over 3) per unit of the host's normalised chroma (`√(a² + b²)`, a* and b* over 255).
/// Fitted by least squares over the sRGB colour cube: Lab chroma ≈ 193 × the RGB saturation. Only
/// Liveliness and Gamma's effect on saturation depend on it (they raise the saturation to a power).
pub const CHROMA_TO_SAT: f32 = 1.32;

/// Steps in the Warmth table, over L from 0 to 1.
const WARMTH_STEPS: usize = 256;

/// Per-run constants for Lab, next to the shared [`Precalc`].
pub struct LabPrecalc {
    /// The RGB transform's saturation for a normalised chroma of 1: [`CHROMA_TO_SAT`] times
    /// Saturation's `(s / 100)²`.
    sat_in: f32,
    /// The exponent on saturation (Liveliness and Gamma), as in RGB.
    sat_exp: f32,
    /// How much the chroma is scaled when `sat_exp` is 1: then it is the same for every colour.
    gain: f32,
    cos_shift: f32,
    sin_shift: f32,
    /// `(ΔL, Δa, Δb)` that Warmth gives a grey of lightness `i / WARMTH_STEPS`, in the host's units;
    /// `None` when Warmth is neutral.
    warmth: Option<[[f32; 3]; WARMTH_STEPS + 1]>,
}

impl LabPrecalc {
    pub fn new(t: &Precalc) -> Self {
        // t.saturation is s² / 30000; the RGB transform's chroma sum is 3 × its saturation.
        let gain = 3.0 * t.saturation;
        Self {
            sat_in: CHROMA_TO_SAT * gain,
            sat_exp: t.sat_exp,
            gain,
            cos_shift: t.cos_shift,
            sin_shift: t.sin_shift,
            warmth: (t.warmth != 1.0).then(|| warmth_table(t)),
        }
    }

    /// For tests and tools: both precalcs from the slider values.
    pub fn from_settings(s: &Settings) -> (Precalc, Self) {
        let t = Precalc::new(s);
        let lab = Self::new(&t);
        (t, lab)
    }

    /// Warmth's shift for a pixel of lightness `l` (linear between the table's steps). Outside 0–1
    /// (32-bit documents) it is the shift at the nearer end, which is none.
    #[inline(always)]
    fn warmth_at(table: &[[f32; 3]; WARMTH_STEPS + 1], l: f32) -> [f32; 3] {
        let x = l.clamp(0.0, 1.0) * WARMTH_STEPS as f32;
        let i = (x as usize).min(WARMTH_STEPS - 1); // NaN casts to 0
        let f = x - i as f32;
        let (lo, hi) = (table[i], table[i + 1]);
        [lo[0] + (hi[0] - lo[0]) * f, lo[1] + (hi[1] - lo[1]) * f, lo[2] + (hi[2] - lo[2]) * f]
    }
}

/// What the RGB transform's Warmth does to greys, as a table over L. A grey of lightness L is the
/// sRGB grey `v` with that L*; Warmth turns it into `(v^(1/w), v, v^w)`, and the entry is that
/// colour's Lab minus the grey's.
fn warmth_table(t: &Precalc) -> [[f32; 3]; WARMTH_STEPS + 1] {
    let mut table = [[0.0; 3]; WARMTH_STEPS + 1];
    for (i, entry) in table.iter_mut().enumerate() {
        let v = convert::grey_of_lightness(100.0 * i as f32 / WARMTH_STEPS as f32);
        let grey = convert::srgb_to_lab([v, v, v]);
        let warm = convert::srgb_to_lab([powf(v, t.warmth_inv), v, powf(v, t.warmth)]);
        *entry = [(warm[0] - grey[0]) / 100.0, (warm[1] - grey[1]) / 255.0, (warm[2] - grey[2]) / 255.0];
    }
    table
}

/// SuperSaturation on one Lab colour in the host's units (L*/100, (a* + 128)/255, (b* + 128)/255).
/// The result is not clamped (the host clamps integer-depth documents when it stores them).
#[inline]
pub fn process_lab(lab: [f32; 3], t: &Precalc, p: &LabPrecalc) -> [f32; 3] {
    let [l, a, b] = lab;
    let mut val = tone(l, t);

    // Saturation and Liveliness: the chroma becomes the RGB transform's saturation, is raised to
    // the same power, and is turned back into chroma.
    let (a, b) = (a - AB_ZERO, b - AB_ZERO);
    let scale = if p.sat_exp == 1.0 {
        p.gain
    } else {
        let chroma = sqrtf(a * a + b * b);
        if chroma > 0.0 { powf(chroma * p.sat_in, p.sat_exp) / (chroma * CHROMA_TO_SAT) } else { 0.0 }
    };
    // Hue Shift: turn (a, b) about grey.
    let (c, s) = (scale * p.cos_shift, scale * p.sin_shift);
    let mut na = a * c - b * s;
    let mut nb = b * c + a * s;

    if let Some(table) = &p.warmth {
        let [dl, da, db] = LabPrecalc::warmth_at(table, val);
        val += dl;
        na += da;
        nb += db;
    }
    [val, na + AB_ZERO, nb + AB_ZERO]
}

/// sRGB ↔ CIE L*a*b* (D50), with the formulas and constants PhotoCraft's own Image › Mode › Lab
/// Color uses. Only the Warmth table and the tests need them; pixels are never converted.
pub mod convert {
    use crate::mathf::powf;

    const D50: [f32; 3] = [0.964_22, 1.0, 0.825_21];
    /// Linear sRGB → D50 XYZ (Bradford-adapted, as in the ICC sRGB profile).
    const SRGB_TO_XYZ_D50: [[f32; 3]; 3] = [[0.436_074, 0.385_064, 0.143_080], [0.222_504, 0.716_878, 0.060_618], [0.013_932, 0.097_104, 0.714_173]];
    const EPSILON: f32 = 6.0 / 29.0;

    pub fn srgb_to_linear(v: f32) -> f32 {
        if v <= 0.04045 { v / 12.92 } else { powf((v + 0.055) / 1.055, 2.4) }
    }

    pub fn linear_to_srgb(v: f32) -> f32 {
        if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * powf(v, 1.0 / 2.4) - 0.055 }
    }

    /// Encoded sRGB → (L*, a*, b*).
    pub fn srgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
        let lin = rgb.map(srgb_to_linear);
        let xyz = SRGB_TO_XYZ_D50.map(|r| r[0] * lin[0] + r[1] * lin[1] + r[2] * lin[2]);
        let f = |t: f32| if t > EPSILON * EPSILON * EPSILON { libm::cbrtf(t) } else { t / (3.0 * EPSILON * EPSILON) + 4.0 / 29.0 };
        let (fx, fy, fz) = (f(xyz[0] / D50[0]), f(xyz[1] / D50[1]), f(xyz[2] / D50[2]));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// The encoded sRGB grey (R = G = B) whose L* is `l`.
    pub fn grey_of_lightness(l: f32) -> f32 {
        let fy = (l + 16.0) / 116.0;
        let y = if fy > EPSILON { fy * fy * fy } else { 3.0 * EPSILON * EPSILON * (fy - 4.0 / 29.0) };
        linear_to_srgb(y.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::convert::srgb_to_lab;
    use super::*;
    use crate::algorithm::process_pixel;

    /// The host's units for a Lab colour.
    fn host(lab: [f32; 3]) -> [f32; 3] {
        [lab[0] / 100.0, (lab[1] + 128.0) / 255.0, (lab[2] + 128.0) / 255.0]
    }
    /// Back to L*, a*, b*.
    fn unhost(px: [f32; 3]) -> [f32; 3] {
        [px[0] * 100.0, px[1] * 255.0 - 128.0, px[2] * 255.0 - 128.0]
    }
    fn chroma(lab: [f32; 3]) -> f32 {
        lab[1].hypot(lab[2])
    }
    fn hue_deg(lab: [f32; 3]) -> f32 {
        lab[2].atan2(lab[1]).to_degrees()
    }
    fn run(s: Settings, lab: [f32; 3]) -> [f32; 3] {
        let (t, p) = LabPrecalc::from_settings(&s);
        unhost(process_lab(host(lab), &t, &p))
    }
    /// The sRGB cube sampled evenly: the centres of 12 × 12 × 12 cells.
    fn grid() -> impl Iterator<Item = [f32; 3]> {
        (0..12 * 12 * 12).map(|i| [i % 12, i / 12 % 12, i / 144].map(|c| (c as f32 + 0.5) / 12.0))
    }

    #[test]
    fn host_units_of_grey_subtract_to_zero() {
        // The host reads an 8-bit 128 as 128 / 255 and a 16-bit 32896 (= 128 × 257) as 32896 / 65535.
        assert_eq!(f32::from(128u8) / 255.0 - AB_ZERO, 0.0);
        assert_eq!(f32::from(32896u16) / 65535.0 - AB_ZERO, 0.0);
    }

    #[test]
    fn neutral_settings_change_nothing() {
        let (t, p) = LabPrecalc::from_settings(&Settings::default());
        assert!(p.warmth.is_none());
        for px in [[0.0, AB_ZERO, AB_ZERO], [0.53, 0.8, 0.2], [1.0, 0.0, 1.0], [0.25, 0.4, 0.61]] {
            let out = process_lab(px, &t, &p);
            for i in 0..3 {
                assert!((out[i] - px[i]).abs() < 1e-6, "{px:?} -> {out:?}");
            }
        }
    }

    #[test]
    fn greys_stay_grey_unless_warmth_moves() {
        let s = Settings { saturation: 200.0, liveliness: 100.0, gamma: 30.0, contrast: 40.0, hue_shift: 90.0, ..Settings::default() };
        let (t, p) = LabPrecalc::from_settings(&s);
        for l in [0.0, 0.2, 0.5, 0.9, 1.0] {
            let out = process_lab([l, AB_ZERO, AB_ZERO], &t, &p);
            assert_eq!([out[1], out[2]], [AB_ZERO, AB_ZERO]);
            assert!((out[0] - tone(l, &t)).abs() < 1e-7);
        }
    }

    #[test]
    fn saturation_scales_chroma_and_keeps_hue_and_lightness() {
        let c = [60.0, 40.0, -30.0];
        let more = run(Settings { saturation: 150.0, ..Settings::default() }, c);
        let none = run(Settings { saturation: 0.0, ..Settings::default() }, c);
        assert!((chroma(more) / chroma(c) - 2.25).abs() < 1e-3, "{more:?}");
        assert!((hue_deg(more) - hue_deg(c)).abs() < 1e-2 && (more[0] - c[0]).abs() < 1e-4);
        assert!(chroma(none) < 1e-4, "{none:?}");
    }

    #[test]
    fn liveliness_lifts_muted_colours_more() {
        let s = Settings { liveliness: 60.0, ..Settings::default() };
        let (muted, strong) = ([50.0, 8.0, 6.0], [50.0, 60.0, 45.0]);
        let (m, st) = (run(s, muted), run(s, strong));
        assert!(chroma(m) / chroma(muted) > chroma(st) / chroma(strong) && chroma(st) > chroma(strong));
    }

    #[test]
    fn hue_shift_turns_red_towards_yellow() {
        let red = srgb_to_lab([1.0, 0.0, 0.0]);
        let turned = run(Settings { hue_shift: 30.0, ..Settings::default() }, red);
        assert!((hue_deg(turned) - hue_deg(red) - 30.0).abs() < 1e-2, "{red:?} -> {turned:?}");
        let rgb = srgb_to_lab(process_pixel([1.0, 0.0, 0.0], &Precalc::new(&Settings { hue_shift: 30.0, ..Settings::default() })).map(|v| v.clamp(0.0, 1.0)));
        assert!(hue_deg(rgb) > hue_deg(red), "RGB mode turns the same way");
    }

    #[test]
    fn warmth_on_greys_matches_rgb_mode() {
        for warmth in [-100.0, -45.0, 30.0, 100.0] {
            let s = Settings { warmth, ..Settings::default() };
            let t = Precalc::new(&s);
            for i in 0..=40 {
                let v = i as f32 / 40.0;
                let grey = srgb_to_lab([v, v, v]);
                let want = srgb_to_lab(process_pixel([v; 3], &t).map(|c| c.clamp(0.0, 1.0)));
                let got = run(s, grey);
                let d = (0..3).map(|k| (got[k] - want[k]).powi(2)).sum::<f32>().sqrt();
                // Exact at the table's steps; between them the shift is linear, and near black (where
                // v^(1/w) is steep) that is off by up to a few tenths of a ΔE.
                assert!(d < if v < 0.1 { 0.6 } else { 0.1 }, "warmth {warmth}, grey {v}: {got:?} vs {want:?} (ΔE {d})");
            }
        }
    }

    #[test]
    fn warmth_leans_orange_or_blue() {
        let grey = [50.0, 0.0, 0.0];
        let warm = run(Settings { warmth: 60.0, ..Settings::default() }, grey);
        let cool = run(Settings { warmth: -60.0, ..Settings::default() }, grey);
        assert!(warm[1] > 0.0 && warm[2] > 0.0, "{warm:?}");
        assert!(cool[1] < 0.0 && cool[2] < 0.0, "{cool:?}");
    }

    #[test]
    fn chroma_to_sat_fits_the_rgb_transform() {
        // Least squares of the RGB transform's saturation (neutral settings) against the host's
        // normalised chroma, over the sRGB cube.
        let (mut sc, mut cc) = (0.0f64, 0.0f64);
        for rgb in grid() {
            let m = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
            let [r, g, b] = rgb.map(|c| c - m);
            let sat = (r.hypot(g) + r.hypot(b) + g.hypot(b)) / 3.0;
            let c = chroma(srgb_to_lab(rgb)) / 255.0;
            sc += f64::from(sat * c);
            cc += f64::from(c * c);
        }
        let fit = (sc / cc) as f32;
        assert!((fit / CHROMA_TO_SAT - 1.0).abs() < 0.01, "fit {fit}");
    }

    #[test]
    fn slider_strength_matches_rgb_mode_on_average() {
        // Colour by colour the two modes differ (on purpose); over many colours a slider should
        // move chroma and lightness about as much as it does in an RGB document. Measured: within
        // 11 % for the chroma, which only Liveliness and Gamma can make differ (Saturation is a
        // plain factor in both).
        let cases = [
            Settings { saturation: 140.0, ..Settings::default() },
            Settings { saturation: 50.0, ..Settings::default() },
            Settings { liveliness: 60.0, ..Settings::default() },
            Settings { liveliness: -60.0, ..Settings::default() },
            Settings { gamma: 50.0, ..Settings::default() },
            Settings { gamma: -50.0, ..Settings::default() },
        ];
        for s in cases {
            let t = Precalc::new(&s);
            let (mut rgb_c, mut lab_c, mut rgb_l, mut lab_l, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for rgb in grid() {
                let lab = srgb_to_lab(rgb);
                let raw = process_pixel(rgb, &t);
                if chroma(lab) < 10.0 || raw.iter().any(|v| !(0.0..=1.0).contains(v)) {
                    continue; // greys, and colours RGB mode pushes out of gamut (clipped there)
                }
                let (via_rgb, via_lab) = (srgb_to_lab(raw), run(s, lab));
                rgb_c += chroma(via_rgb) / chroma(lab);
                lab_c += chroma(via_lab) / chroma(lab);
                rgb_l += via_rgb[0] - lab[0];
                lab_l += via_lab[0] - lab[0];
                n += 1.0;
            }
            let (rgb_c, lab_c, rgb_l, lab_l) = (rgb_c / n, lab_c / n, rgb_l / n, lab_l / n);
            assert!((lab_c / rgb_c - 1.0).abs() < 0.15, "{s:?}: mean chroma ratio {lab_c} vs RGB {rgb_c}");
            assert!((lab_l - rgb_l).abs() < 6.0, "{s:?}: mean ΔL {lab_l} vs RGB {rgb_l}");
        }
    }
}
