//! The nudge style seam (PHILOSOPHY.md §2, TECHNICAL.md's `NudgeStyle`).
//!
//! A style knows how to make itself noticed and how to stand down. It knows
//! nothing about scheduling, credit, or why it fired — the coordinator asks
//! the core and then asks the enabled styles to perform.
//!
//! Two rules bind every style here:
//!
//! * **12 seconds, maximum** (PRODUCT.md §3). After that the app stands down
//!   until the next interval. No re-tell, no escalation, no second sound.
//! * **No guilt.** Ignoring a nudge produces a wave and silence, never a
//!   badge, a counter, or a sadder buddy (VISION.md value 2).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::assets;

/// PRODUCT.md §3's design rule, and a house rule for this milestone.
pub const MAX_NUDGE: Duration = Duration::from_secs(12);

pub trait NudgeStyle: Send {
    fn id(&self) -> &'static str;
    /// Perform. Must not block the coordinator — a style that takes time runs
    /// it on its own thread and watches `cancel`.
    fn fire(&mut self, cancel: Arc<AtomicBool>);
}

// ----------------------------------------------------------------- chime

/// "A distinctive, warm 2-note sound. No visuals beyond menu bar animation.
/// For minimalists." (PRODUCT.md §3) — and on GNOME Wayland, the headline act.
pub struct ChimeStyle {
    /// Opening the audio device costs real time and can fail on a box with no
    /// sink; hold it open so the nudge is never late and never a surprise
    /// error at 3pm.
    sink: rodio::MixerDeviceSink,
}

impl ChimeStyle {
    pub fn new() -> Option<Self> {
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(sink) => Some(Self { sink }),
            Err(e) => {
                eprintln!("scoot: no audio output ({e}); the chime style is unavailable");
                None
            }
        }
    }
}

impl NudgeStyle for ChimeStyle {
    fn id(&self) -> &'static str {
        crate::storage::STYLE_SOUND
    }

    fn fire(&mut self, _cancel: Arc<AtomicBool>) {
        let cursor = std::io::Cursor::new(assets::NUDGE_CHIME);
        match rodio::play(self.sink.mixer(), cursor) {
            // Let it ring out on the audio thread. The chime is ~1s, well
            // inside MAX_NUDGE, and detaching keeps the coordinator's tick
            // loop from ever waiting on audio.
            Ok(player) => player.detach(),
            Err(e) => eprintln!("scoot: chime failed to play ({e})"),
        }
    }
}

// ----------------------------------------------------------- icon bounce

/// "Icon-only animation, impossible to miss if you glance up, invisible if in
/// flow" (PRODUCT.md §3). The tray-side equivalent of the menu bar wiggle,
/// and the visual nudge on every cell that has no overlay.
pub struct IconBounceStyle {
    tray: ksni::blocking::Handle<crate::tray::ScootTray>,
}

impl IconBounceStyle {
    pub fn new(tray: ksni::blocking::Handle<crate::tray::ScootTray>) -> Self {
        Self { tray }
    }
}

/// Frames 1-4 of the atlas, at the 10fps the rest of Scoot animates at
/// (DESIGN.md §2: "8-12 fps — charm lives at low fps"). Three cycles is a
/// glanceable burst that ends well inside the 12s ceiling.
const BOUNCE_FRAME: Duration = Duration::from_millis(100);
const BOUNCE_CYCLES: u32 = 3;

impl NudgeStyle for IconBounceStyle {
    fn id(&self) -> &'static str {
        crate::storage::STYLE_ICON_BOUNCE
    }

    fn fire(&mut self, cancel: Arc<AtomicBool>) {
        let tray = self.tray.clone();
        std::thread::spawn(move || {
            for _ in 0..BOUNCE_CYCLES {
                for frame in 1..assets::TRAY_FRAME_COUNT {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if tray.update(|t| t.frame = frame).is_none() {
                        return; // tray gone; nothing to animate
                    }
                    std::thread::sleep(BOUNCE_FRAME);
                }
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
            }
            // Always return to calm, cancelled or not: the icon must never be
            // left mid-squash (VISION.md value 1, "invisible until helpful").
            let _ = tray.update(|t| t.frame = 0);
        });
    }
}

/// A short celebration on the tray icon when a move is credited — the
/// "Celebrating" state in PRODUCT.md §1's icon table.
pub fn celebrate(tray: &ksni::blocking::Handle<crate::tray::ScootTray>) {
    let tray = tray.clone();
    std::thread::spawn(move || {
        for _ in 0..2 {
            for frame in 1..assets::TRAY_FRAME_COUNT {
                if tray.update(|t| t.frame = frame).is_none() {
                    return;
                }
                std::thread::sleep(BOUNCE_FRAME);
            }
        }
        let _ = tray.update(|t| t.frame = 0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nudge_ceiling_is_the_documented_twelve_seconds() {
        assert_eq!(MAX_NUDGE, Duration::from_secs(12));
        // And the bounce must finish comfortably inside it.
        let bounce = BOUNCE_FRAME * (assets::TRAY_FRAME_COUNT - 1) * BOUNCE_CYCLES;
        assert!(bounce < MAX_NUDGE, "bounce runs {bounce:?}");
    }

    #[test]
    fn the_bounce_runs_at_a_charming_frame_rate() {
        // DESIGN.md §2 pins the animation band at 8-12 fps.
        let fps = 1000 / BOUNCE_FRAME.as_millis();
        assert!((8..=12).contains(&fps), "{fps} fps is outside the design band");
    }
}
