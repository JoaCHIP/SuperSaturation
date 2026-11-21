//! Reads the slider values from the host's parameter JSON without an allocator.
//!
//! The host validates the values against the manifest before calling (missing ones take their
//! defaults, numbers are clamped into range), so this only has to find the six numbers among the
//! object's top-level keys and skip everything else, such as the nested `_image` object.
//!
//! Two sliders read differently in the dialog than in the original filter, and are mapped here:
//! Saturation runs -100 to 100 with 0 neutral (the original's 0 to 200 with 100 neutral, shifted
//! by 100), and Contrast acts more strongly than the original's: twice as strongly upwards, and
//! downwards as strongly as it can without reversing tones (see [`NEGATIVE_CONTRAST_GAIN`]).

use core::f32::consts::FRAC_PI_2;

use crate::algorithm::Settings;

/// How much stronger than the original positive Contrast acts.
const POSITIVE_CONTRAST_GAIN: f32 = 2.0;

/// How much stronger than the original negative Contrast acts: the most it can without reversing.
/// Contrast blends the brightness `v` with an S-curve whose slope peaks at π/2 at mid-grey, so the
/// curve's slope there is `1 + k·(π/2 − 1)` for a blend factor `k` (-1 at the original's -100).
/// This gain makes it exactly 0 at the dialog's -100: mid-tones go flat but never reverse.
const NEGATIVE_CONTRAST_GAIN: f32 = 1.0 / (FRAC_PI_2 - 1.0);

/// The JSON could not be read.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError;

type Result<T> = core::result::Result<T, ParseError>;

/// The settings in `json`, in the original filter's units; a key that is missing keeps its neutral
/// value.
pub fn parse(json: &[u8]) -> Result<Settings> {
    let dialog = parse_dialog(json)?;
    let gain = if dialog.contrast < 0.0 { NEGATIVE_CONTRAST_GAIN } else { POSITIVE_CONTRAST_GAIN };
    Ok(Settings { saturation: dialog.saturation + 100.0, contrast: dialog.contrast * gain, ..dialog })
}

/// The six values as the dialog shows them (Saturation neutral at 0); missing ones are neutral.
fn parse_dialog(json: &[u8]) -> Result<Settings> {
    let mut s = Settings { saturation: 0.0, ..Settings::default() };
    let mut p = Parser { s: json, i: 0 };
    p.ws();
    p.expect(b'{')?;
    p.ws();
    if p.peek() == Some(b'}') {
        return Ok(s);
    }
    loop {
        p.ws();
        let key = p.string()?;
        p.ws();
        p.expect(b':')?;
        p.ws();
        let slot = match key {
            b"saturation" => Some(&mut s.saturation),
            b"liveliness" => Some(&mut s.liveliness),
            b"gamma" => Some(&mut s.gamma),
            b"contrast" => Some(&mut s.contrast),
            b"warmth" => Some(&mut s.warmth),
            b"hueShift" => Some(&mut s.hue_shift),
            _ => None,
        };
        match slot {
            Some(slot) if p.peek().is_some_and(|c| c == b'-' || c.is_ascii_digit()) => *slot = p.number()? as f32,
            _ => p.skip_value(0)?,
        }
        p.ws();
        match p.next() {
            Some(b',') => continue,
            Some(b'}') => return Ok(s),
            _ => return Err(ParseError),
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }

    fn expect(&mut self, c: u8) -> Result<()> {
        if self.next() == Some(c) { Ok(()) } else { Err(ParseError) }
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    /// A string's raw bytes between the quotes (escapes are skipped over, not decoded).
    fn string(&mut self) -> Result<&'a [u8]> {
        self.expect(b'"')?;
        let start = self.i;
        loop {
            match self.next().ok_or(ParseError)? {
                b'"' => return Ok(&self.s[start..self.i - 1]),
                b'\\' => {
                    self.next().ok_or(ParseError)?;
                }
                _ => {}
            }
        }
    }

    fn digits(&mut self) -> (f64, i32) {
        let (mut v, mut n) = (0.0f64, 0);
        while let Some(c @ b'0'..=b'9') = self.peek() {
            v = v * 10.0 + f64::from(c - b'0');
            n += 1;
            self.i += 1;
        }
        (v, n)
    }

    fn number(&mut self) -> Result<f64> {
        let neg = self.peek() == Some(b'-');
        if neg {
            self.i += 1;
        }
        let (mut v, n) = self.digits();
        if n == 0 {
            return Err(ParseError);
        }
        let mut exp = 0i32;
        if self.peek() == Some(b'.') {
            self.i += 1;
            let (frac, k) = self.digits();
            if k == 0 {
                return Err(ParseError);
            }
            v = v * libm::pow(10.0, f64::from(k)) + frac;
            exp -= k;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            let eneg = self.peek() == Some(b'-');
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let (e, k) = self.digits();
            if k == 0 {
                return Err(ParseError);
            }
            let e = e.min(1000.0) as i32;
            exp += if eneg { -e } else { e };
        }
        let v = v * libm::pow(10.0, f64::from(exp));
        Ok(if neg { -v } else { v })
    }

    /// Skips one JSON value of any kind.
    fn skip_value(&mut self, depth: u32) -> Result<()> {
        if depth > 64 {
            return Err(ParseError);
        }
        self.ws();
        match self.peek().ok_or(ParseError)? {
            b'"' => self.string().map(|_| ()),
            open @ (b'{' | b'[') => {
                let close = if open == b'{' { b'}' } else { b']' };
                self.i += 1;
                self.ws();
                if self.peek() == Some(close) {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    if open == b'{' {
                        self.ws();
                        self.string()?;
                        self.ws();
                        self.expect(b':')?;
                    }
                    self.skip_value(depth + 1)?;
                    self.ws();
                    match self.next() {
                        Some(b',') => continue,
                        Some(c) if c == close => return Ok(()),
                        _ => return Err(ParseError),
                    }
                }
            }
            b'-' | b'0'..=b'9' => self.number().map(|_| ()),
            _ => {
                for word in [&b"true"[..], b"false", b"null"] {
                    if self.s[self.i..].starts_with(word) {
                        self.i += word.len();
                        return Ok(());
                    }
                }
                Err(ParseError)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_hosts_parameters() {
        let json = br#"{"saturation": 50, "liveliness": -20.5, "gamma": 1e1, "contrast": 40,
            "warmth": 0.25, "hueShift": -180,
            "_image": {"x": 0, "y": 256, "width": 1024, "height": 256, "overlap": 0,
                       "canvasWidth": 6000, "canvasHeight": 4000, "mode": "rgb", "depth": 8,
                       "alpha": true, "channels": 4, "nested": [1, {"a": null}, "x\"y"], "f": false}}"#;
        let s = parse(json).unwrap();
        assert_eq!(s, Settings { saturation: 150.0, liveliness: -20.5, gamma: 10.0, contrast: 80.0, warmth: 0.25, hue_shift: -180.0 });
    }

    #[test]
    fn missing_keys_stay_neutral() {
        assert_eq!(parse(b"{}").unwrap(), Settings::default());
        assert_eq!(parse(br#"{"gamma":-5}"#).unwrap(), Settings { gamma: -5.0, ..Settings::default() });
        // Saturation's dialog range is the original's shifted by 100, Contrast's is half as strong.
        assert_eq!(parse(br#"{"saturation":-100,"contrast":100}"#).unwrap(), Settings { saturation: 0.0, contrast: 200.0, ..Settings::default() });
        // `_image` nested keys must not be mistaken for parameters.
        assert_eq!(parse(br#"{"_image":{"gamma":7}}"#).unwrap(), Settings::default());
    }

    #[test]
    fn negative_contrast_flattens_mid_tones_without_reversing_them() {
        use crate::algorithm::{Precalc, tone};
        let curve = |json: &[u8]| {
            let t = Precalc::new(&parse(json).unwrap());
            (0..=1000).map(|i| tone(i as f32 / 1000.0, &t)).collect::<Vec<f32>>()
        };
        for gamma in [-100, -40, 0, 40, 100] {
            let c = curve(format!(r#"{{"contrast":-100,"gamma":{gamma}}}"#).as_bytes());
            let worst = c.windows(2).map(|w| w[1] - w[0]).fold(f32::INFINITY, f32::min);
            assert!(worst > -1e-6, "gamma {gamma}: a step goes down by {worst}");
        }
        // Still stronger than the original's -100: the mid-tones are flatter.
        let original = Precalc::new(&Settings { contrast: -100.0, ..Settings::default() });
        let c = curve(br#"{"contrast":-100}"#);
        assert!(c[600] - c[400] < tone(0.6, &original) - tone(0.4, &original));
    }

    #[test]
    fn rejects_malformed_json() {
        for bad in [&b""[..], b"[]", b"{", br#"{"gamma"}"#, br#"{"gamma":}"#, br#"{"gamma":1"#, br#"{"a":tru}"#] {
            assert_eq!(parse(bad), Err(ParseError), "{}", core::str::from_utf8(bad).unwrap());
        }
    }
}
