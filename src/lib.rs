//! SuperSaturation by JoaCHIP, as a PhotoCraft filter plug-in (WebAssembly plug-in ABI v1).
//!
//! A port of the Photoshop filter at github.com/JoaCHIP/SuperSaturation. The colour transform
//! itself is in [`algorithm`] (RGB, and the brightness curve Grayscale shares) and [`lab`] (Lab);
//! this file is the plug-in plumbing: the ABI exports, the pixel buffer and the colour-mode dispatch.
//!
//! The host's sandbox allows no imports. `std` on `wasm32-unknown-unknown` has none (unlike WASI),
//! and the plug-in never allocates, so it uses `std`: that gives `f32::sqrt` as the `f32.sqrt`
//! instruction, which `no_std` code can't reach on stable Rust.

pub mod algorithm;
pub mod lab;
pub mod mathf;
pub mod params;

use algorithm::{Precalc, process_pixel, tone};
use lab::{LabPrecalc, process_lab};

/// The manifest the host reads through `pc_manifest`: id, menu name, author and the parameters the
/// host builds the dialog from.
pub const MANIFEST: &str = include_str!("manifest.json");

/// What `pc_filter` returns when it cannot filter; the host reports the code as an error and leaves
/// the document untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    /// The document's colour mode is not one the plug-in handles (yet): see [`ColorMode`].
    UnsupportedMode = 1,
    /// Too few channels for the colour mode.
    BadChannels = 2,
    /// The buffer is smaller than `width × height × channels` samples.
    BadBuffer = 3,
    /// The parameter JSON could not be read.
    BadParams = 4,
}

/// The `format` argument of `pc_filter`: `depth | alpha << 8 | mode << 16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// 8, 16 or 32. Informational only: samples are always `f32` normalised to 0–1.
    pub depth: u32,
    /// The last channel is (straight) alpha.
    pub alpha: bool,
    pub mode: ColorMode,
}

impl Format {
    pub fn decode(format: u32) -> Self {
        Self { depth: format & 0xff, alpha: format & (1 << 8) != 0, mode: ColorMode::from_psd((format >> 16) & 0xff) }
    }
}

/// The document colour modes, numbered as in the PSD format (and the plug-in ABI).
///
/// Handled: RGB (the original transform), Grayscale (its brightness curve: Gamma and Contrast) and
/// Lab (the transform worked out on L, a and b directly, see [`lab`]). Another mode gets its own
/// branch in [`filter`]; CMYK, for instance, could convert each pixel to RGB, call
/// [`process_pixel`] and convert back. Alpha channels and Quick Mask targets arrive as one Grayscale
/// channel, so they get the Grayscale treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Bitmap,
    Grayscale,
    /// Stored (and handed to plug-ins) as RGB.
    Indexed,
    Rgb,
    Cmyk,
    Multichannel,
    /// Stored as grey.
    Duotone,
    Lab,
    Other(u32),
}

impl ColorMode {
    pub fn from_psd(mode: u32) -> Self {
        match mode {
            0 => Self::Bitmap,
            1 => Self::Grayscale,
            2 => Self::Indexed,
            3 => Self::Rgb,
            4 => Self::Cmyk,
            7 => Self::Multichannel,
            8 => Self::Duotone,
            9 => Self::Lab,
            m => Self::Other(m),
        }
    }
}

/// Filters `samples` (`channels` interleaved `f32` per pixel) in place with the settings in the
/// host's parameter JSON. This is `pc_filter` minus the raw pointers.
pub fn filter(samples: &mut [f32], channels: usize, format: Format, params_json: &[u8]) -> Result<(), Error> {
    let settings = params::parse(params_json).map_err(|_| Error::BadParams)?;
    let t = Precalc::new(&settings);
    match format.mode {
        ColorMode::Rgb => map_pixels::<3, 4>(samples, channels, format.alpha, |c| process_pixel(c, &t)),
        // A grey has no hue or saturation, so only Gamma and Contrast act on it (the RGB transform
        // gives a grey exactly this; Warmth, which would tint it, is ignored in Grayscale).
        ColorMode::Grayscale if t.tone_is_identity() => map_pixels::<1, 2>(samples, channels, format.alpha, |c| c),
        ColorMode::Grayscale => map_pixels::<1, 2>(samples, channels, format.alpha, |[v]| [tone(v, &t)]),
        ColorMode::Lab => {
            let lab = LabPrecalc::new(&t);
            map_pixels::<3, 4>(samples, channels, format.alpha, |c| process_lab(c, &t, &lab))
        }
        _ => Err(Error::UnsupportedMode),
    }
}

/// Applies `f` to the first `C` (colour) channels of every pixel. `CA` is `C + 1`, the pixel size
/// with alpha. With `alpha`, the last channel is alpha: it is kept, and fully transparent pixels
/// are skipped (their colour is invisible, and skipping them saves time). Channels between the
/// colour and the alpha are left alone.
fn map_pixels<const C: usize, const CA: usize>(samples: &mut [f32], channels: usize, alpha: bool, f: impl Fn([f32; C]) -> [f32; C]) -> Result<(), Error> {
    const { assert!(CA == C + 1) };
    if channels < C + usize::from(alpha) {
        return Err(Error::BadChannels);
    }
    // Fast paths for the common layouts: the interpreter rewards fewer instructions.
    if channels == C {
        for p in samples.as_chunks_mut::<C>().0 {
            *p = f(*p);
        }
    } else if channels == CA && alpha {
        for p in samples.as_chunks_mut::<CA>().0 {
            if p[C] > 0.0 {
                let (colour, _) = p.split_first_chunk_mut::<C>().unwrap();
                *colour = f(*colour);
            }
        }
    } else {
        for p in samples.chunks_exact_mut(channels) {
            if alpha && p[channels - 1] <= 0.0 {
                continue;
            }
            let (colour, _) = p.split_first_chunk_mut::<C>().unwrap();
            *colour = f(*colour);
        }
    }
    Ok(())
}

/// Validates the geometry and runs [`filter`]: the body of `pc_filter`.
pub fn filter_buffer(samples: &mut [f32], width: u32, height: u32, channels: u32, format: u32, params_json: &[u8]) -> Result<(), Error> {
    let needed = (width as u64) * (height as u64) * (channels as u64);
    if channels == 0 || needed > samples.len() as u64 {
        return Err(if channels == 0 { Error::BadChannels } else { Error::BadBuffer });
    }
    filter(&mut samples[..needed as usize], channels as usize, Format::decode(format), params_json)
}

#[cfg(target_arch = "wasm32")]
mod abi {
    #[unsafe(no_mangle)]
    pub extern "C" fn pc_abi_version() -> u32 {
        1
    }

    /// `len << 32 | ptr` of the UTF-8 manifest JSON.
    #[unsafe(no_mangle)]
    pub extern "C" fn pc_manifest() -> u64 {
        ((super::MANIFEST.len() as u64) << 32) | super::MANIFEST.as_ptr() as u64
    }

    /// A block of `size` bytes: fresh pages from `memory.grow` (each band gets a fresh instance, so
    /// nothing is ever freed). Returns 0 when the host's memory cap is reached.
    #[unsafe(no_mangle)]
    pub extern "C" fn pc_alloc(size: u32) -> u32 {
        let pages = (size as usize).div_ceil(65536);
        let old = core::arch::wasm32::memory_grow(0, pages);
        if old == usize::MAX { 0 } else { (old * 65536) as u32 }
    }

    /// Filters `width × height` pixels of `channels` interleaved `f32` samples at `buf` in place.
    ///
    /// # Safety
    /// The host guarantees that `buf..buf + len` and `params..params + params_len` are blocks it
    /// got from `pc_alloc`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn pc_filter(
        buf: *mut f32,
        len: u32,
        width: u32,
        height: u32,
        channels: u32,
        format: u32,
        params: *const u8,
        params_len: u32,
    ) -> i32 {
        // SAFETY: see the function's contract.
        let samples = unsafe { core::slice::from_raw_parts_mut(buf, len as usize / 4) };
        let params = if params_len == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(params, params_len as usize) } };
        match super::filter_buffer(samples, width, height, channels, format, params) {
            Ok(()) => 0,
            Err(e) => e as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RGBA8: u32 = 8 | 1 << 8 | 3 << 16;
    const RGB16: u32 = 16 | 3 << 16;

    #[test]
    fn decodes_the_format_word() {
        assert_eq!(Format::decode(RGBA8), Format { depth: 8, alpha: true, mode: ColorMode::Rgb });
        assert_eq!(Format::decode(RGB16), Format { depth: 16, alpha: false, mode: ColorMode::Rgb });
        assert_eq!(Format::decode(32 | 9 << 16).mode, ColorMode::Lab);
    }

    #[test]
    fn manifest_is_valid_json_with_the_six_sliders_in_order() {
        let m: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        let keys: Vec<&str> = m["params"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["saturation", "liveliness", "gamma", "contrast", "warmth", "hueShift"]);
        assert_eq!(m["author"], "JoaCHIP");
        assert_eq!(m["name"], "SuperSaturation…");
        assert_eq!(m["params"]["saturation"]["default"], 0);
    }

    #[test]
    fn rgba_keeps_alpha_and_skips_transparent_pixels() {
        let mut px = [0.8, 0.3, 0.1, 0.5, 0.8, 0.3, 0.1, 0.0];
        filter_buffer(&mut px, 2, 1, 4, RGBA8, br#"{"saturation":-100}"#).unwrap();
        assert!((px[0] - 0.4).abs() < 1e-6 && (px[2] - 0.4).abs() < 1e-6);
        assert_eq!(px[3], 0.5);
        assert_eq!(&px[4..], &[0.8, 0.3, 0.1, 0.0]);
    }

    #[test]
    fn rgb_without_alpha() {
        let mut px = [0.8, 0.3, 0.1, 0.0, 0.0, 1.0];
        filter_buffer(&mut px, 1, 2, 3, RGB16, br#"{"saturation":-100}"#).unwrap();
        assert!((px[0] - 0.4).abs() < 1e-6 && (px[5] - 1.0 / 3.0).abs() < 1e-6, "{px:?}");
    }

    #[test]
    fn grayscale_applies_gamma_and_contrast_only() {
        let params = br#"{"saturation":80,"liveliness":50,"gamma":40,"contrast":30,"warmth":80,"hueShift":90}"#;
        let t = Precalc::new(&params::parse(br#"{"gamma":40,"contrast":30}"#).unwrap());
        let mut px = [0.2, 0.7, 1.0];
        filter_buffer(&mut px, 3, 1, 1, 8 | 1 << 16, params).unwrap();
        assert_eq!(px, [tone(0.2, &t), tone(0.7, &t), tone(1.0, &t)]);
        // With alpha: alpha kept, transparent pixels skipped.
        let mut px = [0.2, 0.5, 0.7, 0.0];
        filter_buffer(&mut px, 2, 1, 2, 16 | 1 << 8 | 1 << 16, params).unwrap();
        assert_eq!(px, [tone(0.2, &t), 0.5, 0.7, 0.0]);
        // Only the sliders that do nothing to greys moved: nothing changes.
        let mut px = [0.2, 0.7];
        filter_buffer(&mut px, 2, 1, 1, 8 | 1 << 16, br#"{"saturation":-100,"warmth":-50}"#).unwrap();
        assert_eq!(px, [0.2, 0.7]);
    }

    #[test]
    fn lab_with_and_without_alpha() {
        let params = br#"{"saturation":50,"hueShift":40}"#;
        let (t, p) = LabPrecalc::from_settings(&params::parse(params).unwrap());
        let colour = [0.5, 0.7, 0.3];
        let mut px = [colour, colour].concat();
        filter_buffer(&mut px, 2, 1, 3, 16 | 9 << 16, params).unwrap();
        assert_eq!(px[..3], process_lab(colour, &t, &p));
        assert_eq!(px[3..], px[..3]);
        let mut px = [0.5, 0.7, 0.3, 1.0, 0.5, 0.7, 0.3, 0.0];
        filter_buffer(&mut px, 2, 1, 4, 8 | 1 << 8 | 9 << 16, params).unwrap();
        assert_eq!(px[..4], [process_lab(colour, &t, &p).as_slice(), &[1.0]].concat());
        assert_eq!(px[4..], [0.5, 0.7, 0.3, 0.0]);
    }

    #[test]
    fn errors() {
        let mut px = [0.5f32; 8];
        assert_eq!(filter_buffer(&mut px, 2, 1, 4, 8 | 4 << 16, b"{}"), Err(Error::UnsupportedMode));
        assert_eq!(filter_buffer(&mut px, 2, 1, 1, 8 | 1 << 8 | 1 << 16, b"{}"), Err(Error::BadChannels));
        assert_eq!(filter_buffer(&mut px, 4, 1, 2, 8 | 3 << 16, b"{}"), Err(Error::BadChannels));
        assert_eq!(filter_buffer(&mut px, 3, 1, 4, RGBA8, b"{}"), Err(Error::BadBuffer));
        assert_eq!(filter_buffer(&mut px, 2, 1, 4, RGBA8, b"{"), Err(Error::BadParams));
    }
}
