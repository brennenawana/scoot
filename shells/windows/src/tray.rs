//! The tray citizen: `Shell_NotifyIcon` plus the popup menu (PORTS.md §7).
//!
//! VISION.md §1 asks Scoot to feel "as inevitable as the Apple logo in the
//! corner". The Windows equivalent is a notification-area icon that idles
//! quietly, animates only when it has something to say, and carries the whole
//! settings surface in its menu — PORTS.md §7 explicitly accepts tray toggles
//! plus `settings.json` as v0.1 parity, so there is no settings window here.
//!
//! Two Win32 obligations this module exists to honour:
//!
//! * **Explorer restarts.** When it does, every tray icon is gone and the
//!   shell broadcasts `TaskbarCreated`. An app that ignores it silently
//!   vanishes from the tray until relaunched — which for a background app means
//!   the user believes it crashed.
//! * **`TrackPopupMenu` needs a foreground window** or the menu refuses to
//!   dismiss when clicked away, leaving a stuck menu on the desktop.

use std::collections::HashMap;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetSystemMetrics, RegisterWindowMessageW,
    SetForegroundWindow, TrackPopupMenu, HMENU, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MF_UNCHECKED, SM_CXSMICON, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP,
};

use crate::render::dib::Icon;
use crate::render::sprite::{decode_png_rgba, Frame};
use crate::storage::settings::{
    OverlayCorner, Settings, INTERVAL_CHOICES, SCALE_CHOICES, STYLE_BUDDY_OVERLAY,
    STYLE_ICON_BOUNCE, STYLE_SOUND,
};

/// Our tray callback message. `WM_APP` and above are reserved for
/// application-private messages, so this can never collide with a system one.
pub const WM_TRAYICON: u32 = WM_APP + 1;

/// Tray art, generated at each size by `scripts/gen-windows-icons.py` and
/// embedded. Never rescaled at runtime — PORTS.md §7 and DESIGN.md §2 both
/// forbid it, because a 16px sprite resampled to 20px is mush.
///
/// One entry per rung of the Windows display-scaling dropdown, because
/// `Shell_NotifyIcon` does not letterbox a mismatched icon, it stretches it.
/// Skipping 28 (175%) or 36 (225%) would not give those users a smaller
/// buddy, it would give them an interpolated one.
const TRAY_ART: [(i32, &[u8]); 9] = [
    (16, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-16.png"))),
    (20, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-20.png"))),
    (24, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-24.png"))),
    (28, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-28.png"))),
    (32, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-32.png"))),
    (36, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-36.png"))),
    (40, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-40.png"))),
    (48, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-48.png"))),
    (56, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tray/tray-56.png"))),
];

/// The five beats in every tray strip: calm, then four bounce frames.
///
/// Frame 0 is the resting pose, matching the macOS `menubar_atlas` layout this
/// art is the port of. Note this is the *opposite* end from
/// [`crate::render::sprite::SpriteSheet::idle_frame_index`], which picks the
/// last frame — that heuristic is for the 40x32 *dance* strips, where the
/// bounce cycle ends on the rest pose. Tray strips start on it. The two must
/// not be crossed, so the tray slices its own frames here rather than going
/// through `SpriteSheet`.
pub const FRAME_CALM: usize = 0;
pub const FRAME_COUNT: usize = 5;

// Menu command ids. Grouped in ranges so a whole family can be recognised with
// a range check rather than a match arm per entry.
pub const CMD_NUDGE_NOW: u32 = 1;
pub const CMD_PAUSE: u32 = 2;
pub const CMD_RESUME: u32 = 3;
pub const CMD_REVEAL_LOG: u32 = 4;
pub const CMD_LAUNCH_AT_LOGIN: u32 = 5;
pub const CMD_TELEMETRY: u32 = 6;
pub const CMD_QUIT: u32 = 7;
pub const CMD_INTERVAL_BASE: u32 = 100;
pub const CMD_STYLE_BASE: u32 = 200;
pub const CMD_PREVIEW_BASE: u32 = 300;
pub const CMD_CORNER_BASE: u32 = 400;
pub const CMD_SCALE_BASE: u32 = 500;

/// Everything the menu needs to render itself. Passed in rather than read from
/// globals so the menu is a pure function of state — which is what makes it
/// testable without a desktop.
pub struct MenuState<'a> {
    pub status_line: String,
    pub is_paused: bool,
    pub settings: &'a Settings,
    pub launch_at_login: bool,
    pub styles: Vec<(&'static str, &'static str)>,
}

pub struct Tray {
    hwnd: HWND,
    frames: Vec<Frame>,
    /// Kept alive because `Shell_NotifyIcon` shows the handle we gave it; the
    /// previous icon is only destroyed once the shell has the new one.
    current: Option<Icon>,
    current_index: usize,
    added: bool,
    taskbar_created: u32,
}

impl Tray {
    /// Build the tray icon for this monitor's DPI and add it to the shell.
    pub fn new(hwnd: HWND) -> Result<Tray, String> {
        let wanted = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let frames = load_frames(wanted)?;

        // Registered before the icon is added: if Explorer were to restart in
        // the window between, we would already know the message to listen for.
        let taskbar_created =
            unsafe { RegisterWindowMessageW(PCWSTR(to_wide("TaskbarCreated").as_ptr())) };

        let mut tray = Tray {
            hwnd,
            frames,
            current: None,
            current_index: FRAME_CALM,
            added: false,
            taskbar_created,
        };
        tray.add()?;
        Ok(tray)
    }

    pub fn taskbar_created_message(&self) -> u32 {
        self.taskbar_created
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            ..Default::default()
        };
        if let Some(icon) = &self.current {
            data.hIcon = icon.0;
        }
        let tip = to_wide("Scoot");
        data.szTip[..tip.len()].copy_from_slice(&tip);
        data
    }

    fn add(&mut self) -> Result<(), String> {
        self.current = Some(Icon::from_frame(&self.frames[FRAME_CALM])?);
        self.current_index = FRAME_CALM;
        let data = self.data();
        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
        self.added = ok;
        if ok {
            Ok(())
        } else {
            Err("Shell_NotifyIcon(NIM_ADD) failed".to_string())
        }
    }

    /// Re-add after Explorer restarts. The old registration died with the old
    /// taskbar, so this is an add, not a modify.
    pub fn reinstate(&mut self) {
        self.added = false;
        let _ = self.add();
    }

    /// Show one of the five beats. A no-op when it is already showing, so the
    /// animation timer can call this every frame without churning GDI handles.
    pub fn set_frame(&mut self, index: usize) {
        let index = index.min(self.frames.len().saturating_sub(1));
        if index == self.current_index && self.current.is_some() {
            return;
        }
        let Ok(icon) = Icon::from_frame(&self.frames[index]) else { return };
        let previous = self.current.replace(icon);
        self.current_index = index;
        let data = self.data();
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
        // Only now is the old handle certainly unused by the shell.
        drop(previous);
    }

    pub fn show_calm(&mut self) {
        self.set_frame(FRAME_CALM);
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

/// Build and run the popup menu, returning the chosen command id.
///
/// Free-standing, and taking a bare `HWND` rather than `&Tray`, for a reason
/// that is not obvious: `TrackPopupMenu` runs its own modal message loop. Any
/// borrow held across this call is held while timers fire and re-enter the
/// window proc — so a `&self` here would panic the moment the tray-bounce
/// animation tried to change the icon with the menu open.
pub fn show_menu(hwnd: HWND, state: &MenuState) -> Option<u32> {
    {
        let menu = build_menu(state)?;
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point).ok()? };

        // Without this the menu will not close when the user clicks elsewhere:
        // a popup owned by a background window never gets the activation change
        // that dismisses it.
        unsafe { SetForegroundWindow(hwnd) };

        let choice = unsafe {
            TrackPopupMenu(
                menu.0,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
                point.x,
                point.y,
                None,
                hwnd,
                None,
            )
        };
        // The documented companion to the SetForegroundWindow call above —
        // it lets the menu's modal loop unwind cleanly.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                windows::Win32::UI::WindowsAndMessaging::WM_NULL,
                WPARAM(0),
                LPARAM(0),
            )
            .ok()
        };

        (choice.0 != 0).then_some(choice.0 as u32)
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if self.added {
            let data = self.data();
            unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        }
    }
}

/// Owns an HMENU so every early return destroys it.
struct OwnedMenu(HMENU);

impl Drop for OwnedMenu {
    fn drop(&mut self) {
        unsafe { let _ = DestroyMenu(self.0); }
    }
}

fn build_menu(state: &MenuState) -> Option<OwnedMenu> {
    let menu = OwnedMenu(unsafe { CreatePopupMenu() }.ok()?);
    // Held so every submenu HMENU stays owned until TrackPopupMenu is done;
    // MF_POPUP transfers ownership to the parent, so these are leaked into the
    // parent deliberately and destroyed with it.
    let mut sub_handles: Vec<HMENU> = Vec::new();

    // The status line is the popover's job on macOS; here it is a disabled
    // first item, which is the tray idiom for "information, not an action".
    append_string(&menu.0, MF_STRING | MF_GRAYED, 0, &state.status_line);
    append_separator(&menu.0);

    append_string(&menu.0, MF_STRING, CMD_NUDGE_NOW, "Nudge Now");
    if state.is_paused {
        append_string(&menu.0, MF_STRING, CMD_RESUME, "Resume");
    } else {
        append_string(&menu.0, MF_STRING, CMD_PAUSE, "Pause 1 Hour");
    }
    append_separator(&menu.0);

    // Remind me every…
    let intervals = unsafe { CreatePopupMenu() }.ok()?;
    sub_handles.push(intervals);
    for (index, minutes) in INTERVAL_CHOICES.iter().enumerate() {
        let label = if *minutes == 1 {
            "1 minute (testing)".to_string()
        } else {
            format!("{minutes} minutes")
        };
        let checked = state.settings.interval_minutes == *minutes;
        append_string(
            &intervals,
            MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
            CMD_INTERVAL_BASE + index as u32,
            &label,
        );
    }
    append_submenu(&menu.0, intervals, "Remind me every");

    // Nudge styles, with a preview per style.
    let styles = unsafe { CreatePopupMenu() }.ok()?;
    sub_handles.push(styles);
    for (index, (id, display)) in state.styles.iter().enumerate() {
        let checked = state.settings.is_style_enabled(id);
        append_string(
            &styles,
            MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
            CMD_STYLE_BASE + index as u32,
            display,
        );
    }
    append_separator(&styles);
    for (index, (_, display)) in state.styles.iter().enumerate() {
        append_string(
            &styles,
            MF_STRING,
            CMD_PREVIEW_BASE + index as u32,
            &format!("Preview {}", display.to_lowercase()),
        );
    }
    append_submenu(&menu.0, styles, "Nudge styles");

    // Buddy corner and size.
    let buddy = unsafe { CreatePopupMenu() }.ok()?;
    sub_handles.push(buddy);
    for (index, corner) in OverlayCorner::ALL.iter().enumerate() {
        let checked = state.settings.corner() == *corner;
        append_string(
            &buddy,
            MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
            CMD_CORNER_BASE + index as u32,
            corner.label(),
        );
    }
    append_separator(&buddy);
    for (index, (scale, label)) in SCALE_CHOICES.iter().enumerate() {
        let checked = state.settings.buddy_scale == *scale;
        append_string(
            &buddy,
            MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
            CMD_SCALE_BASE + index as u32,
            label,
        );
    }
    append_submenu(&menu.0, buddy, "Buddy");
    append_separator(&menu.0);

    append_string(
        &menu.0,
        MF_STRING | if state.launch_at_login { MF_CHECKED } else { MF_UNCHECKED },
        CMD_LAUNCH_AT_LOGIN,
        "Launch at login",
    );
    append_string(
        &menu.0,
        MF_STRING | if state.settings.telemetry_enabled { MF_CHECKED } else { MF_UNCHECKED },
        CMD_TELEMETRY,
        "Share anonymous counts",
    );
    append_string(&menu.0, MF_STRING, CMD_REVEAL_LOG, "Reveal local event log");
    append_separator(&menu.0);
    append_string(&menu.0, MF_STRING, CMD_QUIT, "Quit Scoot");

    Some(menu)
}

fn append_string(
    menu: &HMENU,
    flags: windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS,
    id: u32,
    text: &str,
) {
    let wide = to_wide(text);
    unsafe { let _ = AppendMenuW(*menu, flags, id as usize, PCWSTR(wide.as_ptr())); }
}

fn append_separator(menu: &HMENU) {
    unsafe { let _ = AppendMenuW(*menu, MF_SEPARATOR, 0, PCWSTR::null()); }
}

fn append_submenu(parent: &HMENU, child: HMENU, text: &str) {
    let wide = to_wide(text);
    unsafe {
        let _ = AppendMenuW(*parent, MF_POPUP, child.0 as usize, PCWSTR(wide.as_ptr()));
    }
}

/// Pick the art generated for this size, preferring an exact match and
/// otherwise the nearest one — never a rescale.
fn load_frames(wanted: i32) -> Result<Vec<Frame>, String> {
    let (_, bytes) = TRAY_ART
        .iter()
        .min_by_key(|(size, _)| (size - wanted).abs())
        .ok_or_else(|| "no tray art embedded".to_string())?;
    let strip = decode_png_rgba(bytes)?;
    if strip.width % FRAME_COUNT as u32 != 0 {
        return Err(format!("tray strip {}px is not {FRAME_COUNT} whole frames", strip.width));
    }
    let side = strip.width / FRAME_COUNT as u32;
    if side != strip.height {
        return Err(format!("tray frames must be square, got {side}x{}", strip.height));
    }
    Ok((0..FRAME_COUNT)
        .map(|index| {
            let x = index as u32 * side;
            let mut rgba = Vec::with_capacity((side * side * 4) as usize);
            for row in 0..side as usize {
                let start = (row * strip.width as usize + x as usize) * 4;
                rgba.extend_from_slice(&strip.rgba[start..start + side as usize * 4]);
            }
            Frame { width: side, height: side, rgba }
        })
        .collect())
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Which style index a style command refers to, if any.
pub fn style_index(command: u32, base: u32, count: usize) -> Option<usize> {
    let index = command.checked_sub(base)? as usize;
    (index < count).then_some(index)
}

/// A menu command paired with what it means. Kept as data so `app.rs` can
/// dispatch without re-deriving the ranges.
pub fn describe_commands() -> HashMap<u32, &'static str> {
    HashMap::from([
        (CMD_NUDGE_NOW, "nudge_now"),
        (CMD_PAUSE, "pause"),
        (CMD_RESUME, "resume"),
        (CMD_REVEAL_LOG, "reveal_log"),
        (CMD_LAUNCH_AT_LOGIN, "launch_at_login"),
        (CMD_TELEMETRY, "telemetry"),
        (CMD_QUIT, "quit"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_tray_strip_is_five_square_frames() {
        for (size, bytes) in TRAY_ART {
            let strip = decode_png_rgba(bytes)
                .unwrap_or_else(|e| panic!("{size}px tray art failed to decode: {e}"));
            assert_eq!(
                strip.width,
                size as u32 * FRAME_COUNT as u32,
                "{size}px strip should be {FRAME_COUNT} frames wide"
            );
            assert_eq!(strip.height, size as u32, "{size}px strip height");
        }
    }

    #[test]
    fn frames_load_at_every_size_we_ship() {
        for (size, _) in TRAY_ART {
            let frames = load_frames(size).unwrap_or_else(|e| panic!("{size}px: {e}"));
            assert_eq!(frames.len(), FRAME_COUNT);
            for frame in &frames {
                assert_eq!(frame.width, size as u32);
                assert_eq!(frame.height, size as u32);
                assert_eq!(frame.rgba.len(), (size * size * 4) as usize);
            }
        }
    }

    #[test]
    fn the_calm_frame_is_colored_not_a_macos_template_silhouette() {
        // macOS frame 0 is black-plus-alpha for NSImage.isTemplate tinting.
        // Windows takes a full-colour HICON, so shipping the template here
        // would put a black blob in every tray. Guard it.
        let frames = load_frames(16).unwrap();
        let opaque: Vec<[u8; 3]> = frames[FRAME_CALM]
            .rgba
            .chunks_exact(4)
            .filter(|px| px[3] > 0)
            .map(|px| [px[0], px[1], px[2]])
            .collect();
        assert!(!opaque.is_empty(), "calm frame is entirely transparent");
        let distinct: std::collections::HashSet<_> = opaque.iter().collect();
        assert!(
            distinct.len() > 1,
            "calm frame has a single colour — it looks like a template silhouette"
        );
    }

    #[test]
    fn every_frame_has_something_to_draw() {
        for (size, _) in TRAY_ART {
            let frames = load_frames(size).unwrap();
            for (index, frame) in frames.iter().enumerate() {
                let opaque = frame.rgba.chunks_exact(4).filter(|px| px[3] > 0).count();
                assert!(opaque > 0, "{size}px frame {index} is empty");
            }
        }
    }

    #[test]
    fn every_windows_scaling_rung_has_art_at_exactly_its_size() {
        // SM_CXSMICON is MulDiv(16, dpi, 96). If any of these misses, Explorer
        // stretches our icon into the gap — a fractional resample, which
        // PORTS.md §9 forbids outright. This is the regression guard for the
        // 175%/225% hole found in review.
        for percent in [100, 125, 150, 175, 200, 225, 250, 300, 350] {
            let dpi = 96 * percent / 100;
            let wanted = 16 * dpi / 96;
            let frames = load_frames(wanted).unwrap_or_else(|e| panic!("{percent}%: {e}"));
            assert_eq!(
                frames[0].width as i32, wanted,
                "{percent}% scaling wants {wanted}px and got {}px — the shell would interpolate",
                frames[0].width
            );
        }
    }

    #[test]
    fn an_odd_requested_size_picks_the_nearest_art_never_a_rescale() {
        // 22px sits between the 20 and 24 strips; we must get one of them
        // verbatim rather than a resampled 22.
        let frames = load_frames(22).unwrap();
        assert!(
            frames[0].width == 20 || frames[0].width == 24,
            "got {}px — that can only come from rescaling",
            frames[0].width
        );
    }

    #[test]
    fn style_index_only_matches_inside_its_range() {
        assert_eq!(style_index(CMD_STYLE_BASE, CMD_STYLE_BASE, 3), Some(0));
        assert_eq!(style_index(CMD_STYLE_BASE + 2, CMD_STYLE_BASE, 3), Some(2));
        assert_eq!(style_index(CMD_STYLE_BASE + 3, CMD_STYLE_BASE, 3), None);
        assert_eq!(style_index(CMD_NUDGE_NOW, CMD_STYLE_BASE, 3), None);
    }

    #[test]
    fn command_ranges_do_not_overlap() {
        let singles = [
            CMD_NUDGE_NOW, CMD_PAUSE, CMD_RESUME, CMD_REVEAL_LOG,
            CMD_LAUNCH_AT_LOGIN, CMD_TELEMETRY, CMD_QUIT,
        ];
        let bases = [
            (CMD_INTERVAL_BASE, INTERVAL_CHOICES.len()),
            (CMD_STYLE_BASE, 3),
            (CMD_PREVIEW_BASE, 3),
            (CMD_CORNER_BASE, OverlayCorner::ALL.len()),
            (CMD_SCALE_BASE, SCALE_CHOICES.len()),
        ];
        let mut used = std::collections::HashSet::new();
        for id in singles {
            assert!(used.insert(id), "duplicate command id {id}");
            assert_ne!(id, 0, "0 means 'nothing chosen' to TrackPopupMenu");
        }
        for (base, count) in bases {
            for offset in 0..count as u32 {
                assert!(used.insert(base + offset), "command id {} collides", base + offset);
            }
        }
    }

    #[test]
    fn the_style_ids_the_menu_offers_are_the_contract_ids() {
        for id in [STYLE_BUDDY_OVERLAY, STYLE_SOUND, STYLE_ICON_BOUNCE] {
            assert!(!id.is_empty());
        }
    }

    #[test]
    fn wide_strings_are_nul_terminated() {
        let wide = to_wide("Quit Scoot");
        assert_eq!(*wide.last().unwrap(), 0);
        assert_eq!(wide.len(), "Quit Scoot".len() + 1);
    }
}
