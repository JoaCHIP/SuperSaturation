//! Checks the port against the original Photoshop filter's C++ (`reference/reference.cpp`, whose
//! output for 49 parameter sets × 735 colours is in `tests/data/reference.bin`).
//!
//! Both sides are compared the way the result is stored in an 8- or 16-bit document: clamped to
//! 0–1. A NaN from the original counts as 0, which is what Photoshop stored for it (see
//! `src/algorithm.rs`).

use supersaturation::algorithm::{Precalc, Settings, process_pixel};

const DATA: &[u8] = include_bytes!("data/reference.bin");

struct Record {
    settings: Settings,
    input: [f32; 3],
    expected: [f32; 3],
}

fn records() -> Vec<Record> {
    assert_eq!(DATA.len() % 48, 0);
    DATA.as_chunks::<48>()
        .0
        .iter()
        .map(|rec| {
            let f: Vec<f32> = rec.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
            Record {
                settings: Settings { saturation: f[0], liveliness: f[1], gamma: f[2], contrast: f[3], warmth: f[4], hue_shift: f[5] },
                input: [f[6], f[7], f[8]],
                expected: [f[9], f[10], f[11]],
            }
        })
        .collect()
}

/// How an 8/16-bit document stores a sample: clamped to 0–1, NaN as 0.
fn stored(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

#[test]
fn matches_the_original_photoshop_filter() {
    let records = records();
    assert_eq!(records.len(), 49 * 735);
    let (mut worst, mut worst_at) = (0.0f32, String::new());
    let mut over_16bit_step = 0;
    let mut nan_in_original = 0;
    for r in &records {
        let t = Precalc::new(&r.settings);
        let got = process_pixel(r.input, &t);
        for c in 0..3 {
            if r.expected[c].is_nan() {
                nan_in_original += 1;
            }
            let d = (stored(got[c]) - stored(r.expected[c])).abs();
            if d > 1.0 / 65535.0 {
                over_16bit_step += 1;
            }
            if d > worst {
                worst = d;
                worst_at = format!("{:?} {:?}: expected {:?}, got {:?}", r.settings, r.input, r.expected, got);
            }
        }
    }
    println!("worst difference {worst:e} ({} 16-bit steps) at {worst_at}", worst * 65535.0);
    println!("{over_16bit_step} samples differ by more than one 16-bit step; {nan_in_original} NaN samples in the original");
    // Within one 16-bit step everywhere (in practice the differences are float rounding, ~1e-6).
    assert_eq!(over_16bit_step, 0, "worst difference {worst} at {worst_at}");
}

#[test]
fn neutral_settings_match_the_original_too() {
    // The original's colour model is not an exact round trip, so even the neutral settings move
    // colours slightly. That is part of its character: the port must do the same, not "fix" it.
    let records = records();
    let neutral: Vec<&Record> = records.iter().filter(|r| r.settings == Settings::default()).collect();
    assert_eq!(neutral.len(), 735);
    let t = Precalc::new(&Settings::default());
    let mut moved = 0.0f32;
    for r in neutral {
        let got = process_pixel(r.input, &t);
        for c in 0..3 {
            assert!((stored(got[c]) - stored(r.expected[c])).abs() <= 1.0 / 65535.0, "{:?}", r.input);
            moved = moved.max((stored(r.expected[c]) - r.input[c]).abs());
        }
    }
    println!("at the neutral settings the original moves a sample by up to {:.2} 8-bit levels", moved * 255.0);
}
