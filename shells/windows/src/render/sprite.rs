//! Sprite sheets: a horizontal PNG strip plus a JSON sidecar (CONTRACTS.md
//! §8.1), sliced into frames and scaled by whole numbers only.
//!
//! Two decisions here are inherited from the macOS reference rather than
//! invented, because the same art has to look identical on both platforms:
//!
//! * A sheet loads whole or not at all. `SpriteSheetLoader.load` returns nil
//!   the moment one crop falls outside the strip, and a half-loaded buddy that
//!   animates through missing frames is worse than a buddy that never appears.
//! * Frames stay **straight** (non-premultiplied) RGBA8. Windows' layered
//!   window wants premultiplied, but that is the compositor's business — doing
//!   it here would bake alpha into the pixels the tray icon path also reads.

use png::{BitDepth, ColorType, Decoder, Transformations};
use serde::Deserialize;

/// One decoded frame, straight (non-premultiplied) RGBA8, row-major, top-down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The sidecar (CONTRACTS.md §8.1). All fields required, all > 0.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SpriteManifest {
    pub name: String,
    #[serde(rename = "frameWidth")]
    pub frame_width: u32,
    #[serde(rename = "frameHeight")]
    pub frame_height: u32,
    #[serde(rename = "frameCount")]
    pub frame_count: u32,
    pub fps: f64,
}

impl SpriteManifest {
    /// Decode and validate. The macOS side calls the validation `isValid` and
    /// throws `invalidManifest`; there is no lenient path on either platform,
    /// because a zero frame width is a build mistake in committed content, not
    /// a user setting to be clamped.
    pub fn parse(json: &[u8]) -> Result<SpriteManifest, String> {
        let manifest: SpriteManifest =
            serde_json::from_slice(json).map_err(|e| format!("sprite manifest: {e}"))?;
        // Deliberately NOT rejecting an empty `name`. The normative rule is
        // `SpriteSheetManifest.isValid` (PORTS.md §4), which constrains only
        // the four numeric fields. A stricter Windows loader would mean a
        // sidecar that renders on macOS silently renders nothing here — the
        // exact cross-platform divergence PORTS.md §3 exists to prevent.
        // Tightening this is a CONTRACTS.md §8.1 change, which is Brennen's
        // call, not the port's.
        for (field, value) in [
            ("frameWidth", manifest.frame_width),
            ("frameHeight", manifest.frame_height),
            ("frameCount", manifest.frame_count),
        ] {
            if value == 0 {
                return Err(format!("sprite manifest {}: {field} must be > 0", manifest.name));
            }
        }
        // `<= 0` rather than `!(> 0)` because serde_json cannot hand us a NaN
        // — standard JSON has no literal for one — so the two agree here.
        if manifest.fps <= 0.0 {
            return Err(format!(
                "sprite manifest {}: fps must be > 0, got {}",
                manifest.name, manifest.fps
            ));
        }
        Ok(manifest)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteSheet {
    pub manifest: SpriteManifest,
    pub frames: Vec<Frame>,
}

impl SpriteSheet {
    pub fn load(png_bytes: &[u8], manifest_json: &[u8]) -> Result<SpriteSheet, String> {
        let manifest = SpriteManifest::parse(manifest_json)?;
        let strip = decode_png_rgba(png_bytes)?;

        // Widened to u64 first: a hand-edited frameCount of 4 billion would
        // otherwise wrap and pass a check it should fail.
        let needed = manifest.frame_count as u64 * manifest.frame_width as u64;
        if needed > strip.width as u64 || manifest.frame_height > strip.height {
            return Err(format!(
                "sprite {}: strip is {}x{}, manifest needs {}x{}",
                manifest.name, strip.width, strip.height, needed, manifest.frame_height
            ));
        }

        let (fw, fh) = (manifest.frame_width, manifest.frame_height);
        let frames = (0..manifest.frame_count).map(|i| crop(&strip, i * fw, fw, fh)).collect();
        Ok(SpriteSheet { manifest, frames })
    }

    /// The species' resting pose. Mirrors `SpriteLibrary.idleFrame`: last frame
    /// for grounded bounce strips (lean/squash/lean/idle), first for floaters,
    /// separated by an fps threshold because floaters are authored slower.
    ///
    /// Load-bearing, not cosmetic — this picks the pose that becomes the tray
    /// icon and the Scootdex silhouette, so drifting from the macOS heuristic
    /// would give the same species a different face on Windows.
    pub fn idle_frame_index(&self) -> usize {
        if self.manifest.fps >= 8.0 {
            self.frames.len().saturating_sub(1)
        } else {
            0
        }
    }
}

/// Copies a `width`x`height` window out of `strip` starting at column `x`.
/// Callers have already proven the window fits.
fn crop(strip: &Frame, x: u32, width: u32, height: u32) -> Frame {
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for row in 0..height as usize {
        let start = (row * strip.width as usize + x as usize) * 4;
        rgba.extend_from_slice(&strip.rgba[start..start + width as usize * 4]);
    }
    Frame { width, height, rgba }
}

/// Decodes any PNG to 8-bit straight RGBA. The committed art is all colour
/// type 6 / 8-bit, but the pipeline in `tools/` is free to emit a palette PNG
/// one day and that must not silently render as garbage — so the normalizing
/// happens here rather than being assumed away.
pub fn decode_png_rgba(bytes: &[u8]) -> Result<Frame, String> {
    // png 0.18 wants BufRead + Seek; a Cursor over the borrowed bytes gives it
    // both without a copy, and the art is embedded or read whole either way.
    let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
    // EXPAND lifts sub-byte depths and palettes to 8-bit channels, ALPHA adds
    // the channel palette/RGB sources lack, STRIP_16 narrows 16-bit samples.
    // Everything the crate can hand back afterwards is 8-bit; only greyscale
    // still needs widening to three colour channels, which we do below.
    decoder.set_transformations(
        Transformations::EXPAND | Transformations::ALPHA | Transformations::STRIP_16,
    );
    let mut reader = decoder.read_info().map_err(|e| format!("png header: {e}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "png: image too large to decode".to_string())?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("png decode: {e}"))?;
    let (width, height) = (info.width, info.height);
    if width == 0 || height == 0 {
        return Err(format!("png: degenerate size {width}x{height}"));
    }

    let (color, depth) = reader.output_color_type();
    if depth != BitDepth::Eight {
        return Err(format!("png: expected 8-bit samples after transform, got {depth:?}"));
    }
    let pixels = width as usize * height as usize;
    let channels = match color {
        ColorType::Rgba => 4,
        ColorType::Rgb => 3,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Grayscale => 1,
        other => return Err(format!("png: unsupported colour type {other:?} after transform")),
    };
    if info.buffer_size() != pixels * channels {
        return Err(format!(
            "png: decoder returned {} bytes for {width}x{height}x{channels}",
            info.buffer_size()
        ));
    }

    let rgba = match channels {
        4 => {
            buf.truncate(pixels * 4);
            buf
        }
        _ => {
            let mut out = Vec::with_capacity(pixels * 4);
            for px in buf[..pixels * channels].chunks_exact(channels) {
                let (grey, alpha) = (px[0], *px.last().unwrap());
                match channels {
                    3 => out.extend_from_slice(&[px[0], px[1], px[2], 255]),
                    2 => out.extend_from_slice(&[grey, grey, grey, alpha]),
                    _ => out.extend_from_slice(&[grey, grey, grey, 255]),
                }
            }
            out
        }
    };

    assert_eq!(rgba.len(), pixels * 4, "frame must be exactly width*height*4 bytes");
    Ok(Frame { width, height, rgba })
}

/// Whole-number nearest-neighbor magnification — pixel replication, nothing
/// else. This is constitutional (DESIGN.md §2, PORTS.md §9): any averaging
/// would invent colours the artist never chose, and one blurry buddy breaks
/// the spell. There is deliberately no fractional variant of this function.
pub fn scale_nearest(frame: &Frame, scale: u32) -> Frame {
    assert!(scale >= 1, "sprite scale must be a whole number >= 1, got {scale}");
    if scale == 1 {
        return frame.clone();
    }

    let width = frame.width * scale;
    let mut rgba = Vec::with_capacity(width as usize * (frame.height * scale) as usize * 4);
    for row in 0..frame.height as usize {
        let start = row * frame.width as usize * 4;
        let source = &frame.rgba[start..start + frame.width as usize * 4];
        let mut wide = Vec::with_capacity(width as usize * 4);
        for px in source.chunks_exact(4) {
            for _ in 0..scale {
                wide.extend_from_slice(px);
            }
        }
        for _ in 0..scale {
            rgba.extend_from_slice(&wide);
        }
    }
    Frame { width, height: frame.height * scale, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    macro_rules! art {
        ($name:literal, $ext:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../Sources/Scoot/Resources/Sprites/",
                $name,
                $ext
            ))
        };
    }

    const CLASSIC_PNG: &[u8] = art!("buddy-classic", ".png");
    const CLASSIC_JSON: &[u8] = art!("buddy-classic", ".json");
    // A v0.2 cast member: 40x32 dance frames (16px art + a 2px lean apron per
    // side, x2). M2 does not ship the cast, but PORTS.md requires the door
    // stay open, and a loader that assumed square frames would close it.
    const BLOB_PNG: &[u8] = art!("buddy-round-blob", ".png");
    const BLOB_JSON: &[u8] = art!("buddy-round-blob", ".json");
    // One of the two committed 6 fps floaters.
    const GHOST_PNG: &[u8] = art!("buddy-baby-ghost", ".png");
    const GHOST_JSON: &[u8] = art!("buddy-baby-ghost", ".json");

    fn colours(frame: &Frame) -> HashSet<[u8; 4]> {
        frame.rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    #[test]
    fn classic_matches_its_committed_sidecar() {
        let sheet = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap();
        assert_eq!(sheet.manifest.name, "buddy-classic");
        assert_eq!(sheet.manifest.frame_width, 32);
        assert_eq!(sheet.manifest.frame_height, 32);
        assert_eq!(sheet.manifest.frame_count, 4);
        assert_eq!(sheet.manifest.fps, 10.0);
        assert_eq!(sheet.frames.len(), 4);
    }

    #[test]
    fn a_cast_sheet_loads_at_forty_by_thirty_two() {
        let sheet = SpriteSheet::load(BLOB_PNG, BLOB_JSON).unwrap();
        assert_eq!((sheet.manifest.frame_width, sheet.manifest.frame_height), (40, 32));
        for frame in &sheet.frames {
            assert_eq!((frame.width, frame.height), (40, 32));
            assert_eq!(frame.rgba.len(), 40 * 32 * 4);
        }
    }

    #[test]
    fn frames_are_full_size_and_all_different() {
        let sheet = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap();
        for frame in &sheet.frames {
            assert_eq!((frame.width, frame.height), (32, 32));
            assert_eq!(frame.rgba.len(), 32 * 32 * 4);
        }
        // Slicing at a wrong offset tends to yield duplicate or shifted-but-
        // equal frames, so distinctness is the cheap proof the crop is right.
        let distinct: HashSet<&Vec<u8>> = sheet.frames.iter().map(|f| &f.rgba).collect();
        assert_eq!(distinct.len(), 4, "buddy-classic's four frames should all differ");
    }

    #[test]
    fn the_strip_decodes_to_its_full_width() {
        let strip = decode_png_rgba(CLASSIC_PNG).unwrap();
        assert_eq!((strip.width, strip.height), (128, 32));
        assert_eq!(strip.rgba.len(), 128 * 32 * 4);
    }

    #[test]
    fn frames_come_from_the_right_columns_of_the_strip() {
        let strip = decode_png_rgba(CLASSIC_PNG).unwrap();
        let sheet = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap();
        for (index, frame) in sheet.frames.iter().enumerate() {
            let row = 7;
            let x = index * 32;
            let start = (row * 128 + x) * 4;
            assert_eq!(
                &frame.rgba[row * 32 * 4..(row + 1) * 32 * 4],
                &strip.rgba[start..start + 32 * 4],
                "frame {index} row {row}"
            );
        }
    }

    #[test]
    fn a_manifest_missing_a_field_is_rejected() {
        let json = br#"{"name":"x","frameWidth":32,"frameHeight":32,"frameCount":4}"#;
        let err = SpriteManifest::parse(json).unwrap_err();
        assert!(err.contains("fps"), "{err}");
    }

    #[test]
    fn a_zero_or_negative_field_is_rejected() {
        let zero = br#"{"name":"x","frameWidth":0,"frameHeight":32,"frameCount":4,"fps":10}"#;
        assert!(SpriteManifest::parse(zero).unwrap_err().contains("frameWidth"));

        let negative = br#"{"name":"x","frameWidth":-32,"frameHeight":32,"frameCount":4,"fps":10}"#;
        assert!(SpriteManifest::parse(negative).is_err());

        let no_fps = br#"{"name":"x","frameWidth":32,"frameHeight":32,"frameCount":4,"fps":0}"#;
        assert!(SpriteManifest::parse(no_fps).unwrap_err().contains("fps"));
    }

    #[test]
    fn a_frame_count_past_the_strip_fails_the_whole_sheet() {
        // buddy-classic is exactly 4x32 wide, so a fifth frame has nowhere to
        // come from. macOS returns nil for the sheet rather than four frames.
        let json = br#"{"name":"buddy-classic","frameWidth":32,"frameHeight":32,"frameCount":5,"fps":10}"#;
        let err = SpriteSheet::load(CLASSIC_PNG, json).unwrap_err();
        assert!(err.contains("strip is 128x32"), "{err}");
    }

    #[test]
    fn a_frame_taller_than_the_strip_fails_the_whole_sheet() {
        let json = br#"{"name":"buddy-classic","frameWidth":32,"frameHeight":48,"frameCount":4,"fps":10}"#;
        assert!(SpriteSheet::load(CLASSIC_PNG, json).is_err());
    }

    #[test]
    fn garbage_png_bytes_are_an_error_not_a_panic() {
        assert!(decode_png_rgba(b"not a png at all").is_err());
        assert!(decode_png_rgba(&CLASSIC_PNG[..40]).is_err());
    }

    #[test]
    fn scale_of_one_is_identity() {
        let frame = &SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap().frames[0];
        assert_eq!(&scale_nearest(frame, 1), frame);
    }

    #[test]
    fn scaling_triples_both_dimensions_and_invents_no_colours() {
        let frame = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap().frames[0].clone();
        let big = scale_nearest(&frame, 3);
        assert_eq!((big.width, big.height), (96, 96));
        assert_eq!(big.rgba.len(), 96 * 96 * 4);

        // The guard on the constitutional rule: an interpolating resize blends
        // neighbours and so emits colours the source never contained. Pure
        // replication cannot, no matter the factor.
        let source = colours(&frame);
        for colour in colours(&big) {
            assert!(source.contains(&colour), "scaling invented {colour:?}");
        }
    }

    #[test]
    fn scaling_replicates_each_pixel_into_a_square_block() {
        let frame = Frame {
            width: 2,
            height: 2,
            rgba: vec![1, 1, 1, 255, 2, 2, 2, 255, 3, 3, 3, 255, 4, 4, 4, 255],
        };
        let big = scale_nearest(&frame, 2);
        assert_eq!((big.width, big.height), (4, 4));
        assert_eq!(
            big.rgba[..16].to_vec(),
            vec![1, 1, 1, 255, 1, 1, 1, 255, 2, 2, 2, 255, 2, 2, 2, 255]
        );
        assert_eq!(big.rgba[..16], big.rgba[16..32], "the row should repeat verbatim");
    }

    #[test]
    #[should_panic(expected = "whole number")]
    fn a_zero_scale_is_a_programming_error() {
        scale_nearest(&Frame { width: 1, height: 1, rgba: vec![0, 0, 0, 0] }, 0);
    }

    #[test]
    fn idle_is_the_last_frame_at_ten_fps_and_the_first_below_eight() {
        let grounded = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).unwrap();
        assert_eq!(grounded.manifest.fps, 10.0);
        assert_eq!(grounded.idle_frame_index(), 3);

        let floater = SpriteSheet::load(GHOST_PNG, GHOST_JSON).unwrap();
        assert_eq!(floater.manifest.fps, 6.0, "buddy-baby-ghost is a committed floater");
        assert_eq!(floater.idle_frame_index(), 0);
    }
}
