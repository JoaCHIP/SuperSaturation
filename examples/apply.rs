//! Applies SuperSaturation natively to raw pixels, to check what PhotoCraft produced with the plug-in.
//!
//! Usage: cargo run --release --example apply -- IN.f32 OUT.f32 WIDTH HEIGHT CHANNELS 'PARAMS_JSON' [MODE]
//!
//! IN and OUT hold little-endian `f32` samples, interleaved, in the plug-in ABI's own layout and
//! units (0–1; Lab as L*/100, (a* + 128)/255, (b* + 128)/255). MODE is `rgb` (the default), `gray`,
//! `lab` or `cmyk`; one channel more than the mode's colours (4 for RGB and Lab, 2 for gray, 5 for
//! CMYK) is alpha.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if !(7..=8).contains(&args.len()) {
        eprintln!("usage: apply IN.f32 OUT.f32 WIDTH HEIGHT CHANNELS PARAMS_JSON [rgb|gray|lab|cmyk]");
        return ExitCode::from(2);
    }
    let bytes = std::fs::read(&args[1]).expect("read input");
    let mut samples: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    let (width, height, channels): (u32, u32, u32) = (args[3].parse().unwrap(), args[4].parse().unwrap(), args[5].parse().unwrap());
    let (mode, colours) = match args.get(7).map_or("rgb", String::as_str) {
        "rgb" => (3, 3),
        "gray" => (1, 1),
        "lab" => (9, 3),
        "cmyk" => (4, 4),
        other => {
            eprintln!("unknown mode {other}");
            return ExitCode::from(2);
        }
    };
    let format = 8 | u32::from(channels > colours) << 8 | mode << 16;
    if let Err(e) = supersaturation::filter_buffer(&mut samples, width, height, channels, format, args[6].as_bytes()) {
        eprintln!("filter failed: {e:?}");
        return ExitCode::FAILURE;
    }
    let out: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&args[2], out).expect("write output");
    ExitCode::SUCCESS
}
