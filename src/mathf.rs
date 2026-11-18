//! Float maths tuned for the host's WebAssembly interpreter, where every instruction counts.
//!
//! `libm` (pure Rust, so the plug-in needs no imports) is used for the trigonometry. Two of its
//! functions are slow in an interpreter, so they are replaced:
//!
//! * [`sqrtf`]: `libm::sqrtf` is a software routine on stable Rust (it uses the `f32.sqrt`
//!   instruction only with a nightly feature). `std`'s `f32::sqrt` is the instruction. Both are
//!   correctly rounded, so the results are identical.
//! * [`powf`]: `libm::powf` is FreeBSD's, ~140 float operations of extended-precision tricks. The
//!   plug-in only raises positive finite numbers to positive powers, which `exp2(y · log2 x)` in
//!   `f64` with short polynomials handles in ~40 operations, accurate to ~1e-11 before rounding to
//!   `f32`, i.e. within one `f32` ulp of the correctly rounded result. Other inputs go to `libm`.
//!
//! The native tests use these same functions, so they test the plug-in's exact maths.

pub use libm::{atan2f, cosf, sinf};

/// Square root (the `f32.sqrt` instruction on wasm).
#[inline(always)]
pub fn sqrtf(x: f32) -> f32 {
    x.sqrt()
}

/// `x^y`. Fast for a positive finite `x` and finite `y` (every call in the plug-in); anything else,
/// including 0, negatives (NaN for a fractional `y`), NaN and infinity, is `libm::powf`.
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    if !(x > 0.0 && x < f32::INFINITY && y.is_finite()) {
        return libm::powf(x, y);
    }
    let z = f64::from(y) * log2(f64::from(x));
    if z > 128.0 {
        return f32::INFINITY;
    }
    if z < -150.0 {
        return 0.0;
    }
    exp2(z) as f32
}

const LN2: f64 = core::f64::consts::LN_2;
const SQRT2: f64 = core::f64::consts::SQRT_2;

/// `log2(x)` for a positive normal `f64` (every positive finite `f32` is one).
#[inline(always)]
fn log2(x: f64) -> f64 {
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    // The mantissa in [1, 2), then moved to [√½, √2) so the series below converges fast.
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > SQRT2 {
        m *= 0.5;
        e += 1;
    }
    // ln(m) = 2·atanh(s), s = (m − 1)/(m + 1), |s| ≤ 0.1716: the series to s^13 is good to ~1e-12.
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let p = 1.0 / 13.0;
    let p = p * s2 + 1.0 / 11.0;
    let p = p * s2 + 1.0 / 9.0;
    let p = p * s2 + 1.0 / 7.0;
    let p = p * s2 + 1.0 / 5.0;
    let p = p * s2 + 1.0 / 3.0;
    let p = p * s2 + 1.0;
    e as f64 + 2.0 * s * p / LN2
}

/// `2^z` for `z` in about [-150, 128] (the `f32` range plus subnormals).
#[inline(always)]
fn exp2(z: f64) -> f64 {
    let n = (z + 0.5).floor_to_i64();
    // 2^f = e^(f·ln2), |f| ≤ ½ so |w| ≤ 0.347: Taylor to w^10 is good to ~1e-13.
    let w = (z - n as f64) * LN2;
    let mut p = 1.0 / 3628800.0;
    p = p * w + 1.0 / 362880.0;
    p = p * w + 1.0 / 40320.0;
    p = p * w + 1.0 / 5040.0;
    p = p * w + 1.0 / 720.0;
    p = p * w + 1.0 / 120.0;
    p = p * w + 1.0 / 24.0;
    p = p * w + 1.0 / 6.0;
    p = p * w + 0.5;
    p = p * w + 1.0;
    p = p * w + 1.0;
    // × 2^n, built directly as an f64 (n is in [-150, 128], well inside f64's normal range).
    p * f64::from_bits(((n + 1023) as u64) << 52)
}

trait FloorToI64 {
    fn floor_to_i64(self) -> i64;
}

impl FloorToI64 for f64 {
    /// `floor(self)` as an integer, for |self| < 2^62 (no `libm::floor` call).
    #[inline(always)]
    fn floor_to_i64(self) -> i64 {
        let t = self as i64; // truncates toward zero
        if (t as f64) > self { t - 1 } else { t }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Distance in units in the last place between two positive floats.
    fn ulps(a: f32, b: f32) -> u32 {
        (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs() as u32
    }

    #[test]
    fn powf_is_within_one_ulp_of_a_correctly_rounded_pow() {
        // f64 pow, rounded to f32, is the correctly rounded result in all but vanishingly rare cases.
        let exps = [0.25f32, 0.390625, 0.5625, 0.8, 0.9506, 1.0, 1.0526, 1.3225, 1.5625, 2.25, 2.5, 5.0625, 1.0 / 2.5, 1.0 / 0.5];
        let mut worst = 0;
        let mut x = 1e-30f32;
        while x < 8.0 {
            for &y in &exps {
                let want = (f64::from(x)).powf(f64::from(y)) as f32;
                let got = powf(x, y);
                let d = ulps(got, want);
                worst = worst.max(d);
                assert!(d <= 1, "powf({x:e}, {y}) = {got:e}, want {want:e}");
            }
            x *= 1.000_37;
        }
        println!("worst {worst} ulp");
    }

    #[test]
    fn powf_edge_cases_match_libm() {
        for (x, y) in [(0.0f32, 2.0f32), (0.0, 0.25), (-0.0, 1.5), (-0.5, 1.5), (-0.5, 2.0), (f32::NAN, 2.0), (f32::INFINITY, 0.5), (1e-45, 2.25), (3.0, 0.0)] {
            let (got, want) = (powf(x, y), libm::powf(x, y));
            assert!(got.to_bits() == want.to_bits() || (got.is_nan() && want.is_nan()), "powf({x}, {y}) = {got}, libm {want}");
        }
        // Underflow to zero and overflow to infinity in the fast path.
        assert_eq!(powf(1e-30, 5.0625), 0.0);
        assert_eq!(powf(1e30, 5.0625), f32::INFINITY);
    }

    #[test]
    fn floor_to_i64() {
        for (v, f) in [(0.0, 0), (0.5, 0), (-0.5, -1), (-1.0, -1), (2.999, 2), (-150.2, -151), (128.0, 128)] {
            assert_eq!(v.floor_to_i64(), f, "{v}");
        }
    }
}
