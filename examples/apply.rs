//! Applies SuperSaturation natively to raw pixels, to check what PhotoCraft produced with the plug-in.
//!
//! Usage: cargo run --release --example apply -- IN.f32 OUT.f32 WIDTH HEIGHT CHANNELS 'PARAMS_JSON'
//!
//! IN and OUT hold little-endian `f32` samples, interleaved, 0–1 (the plug-in ABI's own layout).
//! CHANNELS is 3 (RGB) or 4 (RGBA).

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        eprintln!("usage: apply IN.f32 OUT.f32 WIDTH HEIGHT CHANNELS PARAMS_JSON");
        return ExitCode::from(2);
    }
    let bytes = std::fs::read(&args[1]).expect("read input");
    let mut samples: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    let (width, height, channels): (u32, u32, u32) = (args[3].parse().unwrap(), args[4].parse().unwrap(), args[5].parse().unwrap());
    let format = 8 | u32::from(channels == 4) << 8 | 3 << 16;
    if let Err(e) = supersaturation::filter_buffer(&mut samples, width, height, channels, format, args[6].as_bytes()) {
        eprintln!("filter failed: {e:?}");
        return ExitCode::FAILURE;
    }
    let out: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&args[2], out).expect("write output");
    ExitCode::SUCCESS
}
