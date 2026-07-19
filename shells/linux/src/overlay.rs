//! The buddy overlay — progressive enhancement, X11 only (PORTS.md §8).
//!
//! "Buddy walks/hops in from the screen edge into a chosen corner, does its
//! dance ~10s, waves, leaves. Click it = credit + celebration. Ignores you
//! politely." (PRODUCT.md §3.) This is the hero nudge style where the
//! platform allows one; on GNOME Wayland it simply does not exist, and the
//! style catalog absorbs that by design.
//!
//! The window is an override-redirect X11 window: no window manager
//! decoration, no focus stealing, no taskbar entry. Input is shaped to the
//! buddy's own rectangle via the XFIXES/SHAPE extensions, so clicks anywhere
//! else fall through to whatever the user was actually working on — the
//! overlay must never eat a click meant for their editor.
//!
//! Rendering is the constitutional kind: 32-bit ARGB visual, integer sprite
//! scaling, nearest-neighbor, integral origins (DESIGN.md §2).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

use crate::app::AppEvent;
use crate::assets::{self, Image, SpriteManifest};
use crate::nudge::{NudgeStyle, MAX_NUDGE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

impl Corner {
    /// §8.6's `overlayCorner` values. An unknown value falls back to the
    /// documented default rather than refusing to draw.
    pub fn parse(s: &str) -> Self {
        match s {
            "bottomLeft" => Self::BottomLeft,
            "topRight" => Self::TopRight,
            "topLeft" => Self::TopLeft,
            _ => Self::BottomRight,
        }
    }

    /// Integral origin for a `w`x`h` buddy on a `sw`x`sh` screen, inset by
    /// `margin`. Returns integers because a fractional origin would put the
    /// sprite on a half-pixel and blur every edge.
    pub fn origin(self, sw: i32, sh: i32, w: i32, h: i32, margin: i32) -> (i32, i32) {
        match self {
            Self::BottomRight => (sw - w - margin, sh - h - margin),
            Self::BottomLeft => (margin, sh - h - margin),
            Self::TopRight => (sw - w - margin, margin),
            Self::TopLeft => (margin, margin),
        }
    }
}

/// Distance from the screen edge. Enough that the buddy reads as *placed*
/// rather than falling off, and clear of GNOME's top bar / dock shadows.
const MARGIN: i32 = 48;

pub struct BuddyOverlayStyle {
    tx: Sender<AppEvent>,
    corner: Corner,
    scale: u32,
    /// Set when a performance is already on screen, so a second nudge cannot
    /// stack two buddies in the same corner.
    performing: Arc<AtomicBool>,
}

impl BuddyOverlayStyle {
    /// Returns `None` when X11 is unreachable — the caller records
    /// `overlay=none` and the style is simply never offered.
    pub fn new(tx: Sender<AppEvent>, corner: Corner, scale: u32) -> Option<Self> {
        let (conn, _) = x11rb::connect(None).ok()?;
        // Composite/ARGB support is what makes a transparent overlay possible
        // at all; prove we can find a 32-bit visual before claiming the style.
        let screen = conn.setup().roots.first()?;
        argb_visual(screen)?;
        Some(Self { tx, corner, scale: scale.max(1), performing: Arc::new(AtomicBool::new(false)) })
    }
}

impl NudgeStyle for BuddyOverlayStyle {
    fn id(&self) -> &'static str {
        crate::storage::STYLE_BUDDY_OVERLAY
    }

    fn fire(&mut self, cancel: Arc<AtomicBool>) {
        if self.performing.swap(true, Ordering::SeqCst) {
            return; // already dancing
        }
        let tx = self.tx.clone();
        let corner = self.corner;
        let scale = self.scale;
        let performing = self.performing.clone();
        std::thread::spawn(move || {
            if let Err(e) = perform(tx, corner, scale, cancel) {
                eprintln!("scoot: overlay failed ({e})");
            }
            performing.store(false, Ordering::SeqCst);
        });
    }
}

/// Find a 32-bit TrueColor visual — required for per-pixel alpha. Without a
/// compositor the window still maps, it just composites against black, which
/// is why the capability check is "can we find the visual", not "is a
/// compositor running".
fn argb_visual(screen: &Screen) -> Option<(u32, u8)> {
    for depth in &screen.allowed_depths {
        if depth.depth == 32 {
            for visual in &depth.visuals {
                if visual.class == VisualClass::TRUE_COLOR {
                    return Some((visual.visual_id, depth.depth));
                }
            }
        }
    }
    None
}

fn perform(
    tx: Sender<AppEvent>,
    corner: Corner,
    scale: u32,
    cancel: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let (visual_id, depth) = argb_visual(screen).ok_or("no 32-bit visual")?;

    let manifest = SpriteManifest::parse(assets::BUDDY_MANIFEST).ok_or("bad sprite manifest")?;
    let sheet = Image::decode(assets::BUDDY_SHEET).ok_or("bad sprite sheet")?;
    let frames: Vec<Image> = (0..manifest.frame_count)
        .filter_map(|i| sheet.frame(i, manifest.frame_count))
        .map(|f| f.scaled(scale))
        .collect();
    if frames.is_empty() {
        return Err("no sprite frames".into());
    }

    let (w, h) = (frames[0].width as i32, frames[0].height as i32);
    let (x, y) = corner.origin(
        screen.width_in_pixels as i32,
        screen.height_in_pixels as i32,
        w,
        h,
        MARGIN,
    );

    // An ARGB window needs its own colormap; inheriting the root's 24-bit one
    // is a BadMatch.
    let colormap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual_id)?;

    let window = conn.generate_id()?;
    conn.create_window(
        depth,
        window,
        screen.root,
        x as i16,
        y as i16,
        w as u16,
        h as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        visual_id,
        &CreateWindowAux::new()
            // override_redirect: the window manager leaves us alone entirely —
            // no decoration, no focus, no workspace reshuffling.
            .override_redirect(1)
            .background_pixel(0)
            .border_pixel(0)
            .colormap(colormap)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
    )?;

    // Type + state hints. Even override-redirect, these tell a compositor how
    // to treat us: an always-on-top utility surface that is not a real window.
    let atom = |name: &str| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
    };
    let net_wm_window_type = atom("_NET_WM_WINDOW_TYPE")?;
    let notification = atom("_NET_WM_WINDOW_TYPE_NOTIFICATION")?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_window_type,
        AtomEnum::ATOM,
        &[notification],
    )?;
    let net_wm_state = atom("_NET_WM_STATE")?;
    let above = atom("_NET_WM_STATE_ABOVE")?;
    let sticky = atom("_NET_WM_STATE_STICKY")?; // follow the user across workspaces
    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_state,
        AtomEnum::ATOM,
        &[above, sticky],
    )?;
    let desktop = atom("_NET_WM_DESKTOP")?;
    conn.change_property32(PropMode::REPLACE, window, desktop, AtomEnum::CARDINAL, &[0xFFFF_FFFF])?;

    // Clicks land only on the buddy's own rectangle; everywhere else in the
    // window (there is nothing else, but a future speech bubble would be)
    // falls through to the app underneath.
    if conn.extension_information(shape::X11_EXTENSION_NAME)?.is_some() {
        let region = conn.generate_id()?;
        conn.create_pixmap(1, region, window, w as u16, h as u16)?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, region, &CreateGCAux::new().foreground(1))?;
        conn.poly_fill_rectangle(
            region,
            gc,
            &[Rectangle { x: 0, y: 0, width: w as u16, height: h as u16 }],
        )?;
        conn.shape_mask(shape::SO::SET, shape::SK::INPUT, window, 0, 0, region)?;
        conn.free_gc(gc)?;
        conn.free_pixmap(region)?;
    }

    conn.map_window(window)?;
    conn.flush()?;

    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new())?;

    let frame_time = Duration::from_secs_f64(1.0 / manifest.fps.max(1.0));
    let started = Instant::now();
    let mut frame_index = 0usize;
    let mut clicked = false;

    loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() >= MAX_NUDGE {
            break;
        }

        let frame = &frames[frame_index % frames.len()];
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            frame.width as u16,
            frame.height as u16,
            0,
            0,
            0,
            depth,
            &to_x11_bgra(frame),
        )?;
        conn.flush()?;
        frame_index += 1;

        // Drain input for one frame's worth of time rather than sleeping
        // blind, so a click is answered immediately.
        let until = Instant::now() + frame_time;
        while Instant::now() < until {
            match conn.poll_for_event()? {
                Some(Event::ButtonPress(_)) => {
                    clicked = true;
                    break;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(8)),
            }
        }
        if clicked {
            break;
        }
    }

    conn.destroy_window(window)?;
    conn.free_colormap(colormap)?;
    conn.flush()?;

    // The outcome is the coordinator's to act on: a click credits a move, a
    // timeout is logged and nothing else. No sad state either way.
    let _ = tx.send(if clicked {
        AppEvent::OverlayClicked
    } else {
        AppEvent::OverlayTimedOut
    });
    Ok(())
}

/// X11's Z_PIXMAP on a little-endian server wants BGRA byte order; our sprites
/// are RGBA. Also premultiplies alpha, which is what a compositor expects for
/// a 32-bit ARGB window — skipping it gives every semi-transparent edge a
/// bright halo.
fn to_x11_bgra(img: &Image) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.rgba.len());
    for px in img.rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let p = |c: u8| ((c as u32 * a) / 255) as u8;
        out.extend_from_slice(&[p(px[2]), p(px[1]), p(px[0]), px[3]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_place_the_buddy_inside_the_screen_with_integral_origins() {
        let (sw, sh, w, h) = (1920, 1080, 96, 96);
        for corner in [
            Corner::BottomRight,
            Corner::BottomLeft,
            Corner::TopRight,
            Corner::TopLeft,
        ] {
            let (x, y) = corner.origin(sw, sh, w, h, MARGIN);
            assert!(x >= 0 && y >= 0, "{corner:?} placed off-screen at ({x},{y})");
            assert!(x + w <= sw && y + h <= sh, "{corner:?} overflows at ({x},{y})");
        }

        assert_eq!(
            Corner::BottomRight.origin(sw, sh, w, h, MARGIN),
            (1920 - 96 - 48, 1080 - 96 - 48)
        );
        assert_eq!(Corner::TopLeft.origin(sw, sh, w, h, MARGIN), (48, 48));
    }

    #[test]
    fn the_corner_setting_parses_and_defaults_safely() {
        assert_eq!(Corner::parse("bottomRight"), Corner::BottomRight);
        assert_eq!(Corner::parse("bottomLeft"), Corner::BottomLeft);
        assert_eq!(Corner::parse("topRight"), Corner::TopRight);
        assert_eq!(Corner::parse("topLeft"), Corner::TopLeft);
        // Garbage must not stop the buddy appearing.
        assert_eq!(Corner::parse("middle"), Corner::BottomRight);
        assert_eq!(Corner::parse(""), Corner::BottomRight);
    }

    #[test]
    fn pixels_are_premultiplied_bgra_for_the_compositor() {
        // Opaque red RGBA -> BGRA, unchanged by premultiplication.
        let opaque = Image { width: 1, height: 1, rgba: vec![255, 0, 0, 255] };
        assert_eq!(to_x11_bgra(&opaque), vec![0, 0, 255, 255]);

        // Half-transparent white -> each channel scaled by alpha. Without this
        // the compositor draws a bright halo around every soft edge.
        let half = Image { width: 1, height: 1, rgba: vec![255, 255, 255, 128] };
        assert_eq!(to_x11_bgra(&half), vec![128, 128, 128, 128]);

        // Fully transparent contributes no colour at all.
        let clear = Image { width: 1, height: 1, rgba: vec![255, 255, 255, 0] };
        assert_eq!(to_x11_bgra(&clear), vec![0, 0, 0, 0]);
    }

    #[test]
    fn the_sprite_scales_by_whole_pixels_only() {
        let sheet = Image::decode(assets::BUDDY_SHEET).unwrap();
        let manifest = SpriteManifest::parse(assets::BUDDY_MANIFEST).unwrap();
        let frame = sheet.frame(0, manifest.frame_count).unwrap();
        for scale in 1..=4 {
            let scaled = frame.scaled(scale);
            assert_eq!(scaled.width, frame.width * scale);
            assert_eq!(scaled.height, frame.height * scale);
        }
    }
}
