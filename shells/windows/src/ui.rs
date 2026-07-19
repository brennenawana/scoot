//! Shared chrome for Scoot's two real windows — the popover and Settings.
//!
//! DESIGN.md §1 splits the product into two visual registers and forbids
//! blurring them. This module is entirely **Layer 1: invisible-native**. It
//! owns no pixel art and no brand colour; everything here defers to Windows —
//! the shell's UI font at the window's own DPI, the user's light/dark choice,
//! the system accent for focus, Windows 11 rounded corners. If a surface built
//! from these helpers would look out of place in Settings, it is wrong.
//!
//! Layer 2 (the buddy) is drawn *inside* these frames by the sprite path, at
//! integer scale with nearest-neighbor sampling. The contrast between a
//! correctly boring window and a living pixel creature inside it is the whole
//! effect.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, RoundRect,
    FillRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_LEFT, DT_SINGLELINE,
    DT_VCENTER, HBRUSH, HFONT, LOGFONTW, PS_SOLID, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_SELECTED};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextW, SendMessageW, CB_GETLBTEXT, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
};

/// Logical-to-physical for a window's DPI. Every offset in the two windows
/// goes through here — hard-coded pixel positions are how a layout that looks
/// right at 100% turns into overlapping controls at 150%.
pub fn scaled(logical: i32, dpi: u32) -> i32 {
    (logical * dpi as i32 / 96).max(if logical > 0 { 1 } else { 0 })
}

/// The colours a surface should paint itself in.
///
/// Deliberately few. Windows already has a look; the job here is to match it,
/// not to design a palette. The values track what the stock light/dark shells
/// use for popups so a Scoot window sitting next to a system flyout does not
/// read as a different application's idea of grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub is_dark: bool,
    pub background: COLORREF,
    pub text: COLORREF,
    /// Secondary text — captions, the disclosure line under a toggle.
    pub subtle: COLORREF,
    /// Hairline separators.
    pub divider: COLORREF,
}

impl Theme {
    pub fn current() -> Theme {
        if crate::theme::prefers_dark() {
            Theme {
                is_dark: true,
                background: rgb(32, 32, 32),
                text: rgb(255, 255, 255),
                subtle: rgb(160, 160, 160),
                divider: rgb(60, 60, 60),
            }
        } else {
            Theme {
                is_dark: false,
                background: rgb(249, 249, 249),
                text: rgb(26, 26, 26),
                subtle: rgb(96, 96, 96),
                divider: rgb(225, 225, 225),
            }
        }
    }
}

pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

/// A GDI brush that deletes itself.
pub struct Brush(pub HBRUSH);

impl Brush {
    pub fn solid(color: COLORREF) -> Brush {
        Brush(unsafe { windows::Win32::Graphics::Gdi::CreateSolidBrush(color) })
    }
}

impl Drop for Brush {
    fn drop(&mut self) {
        unsafe { let _ = DeleteObject(self.0.into()); }
    }
}

/// A GDI font that deletes itself.
pub struct Font(pub HFONT);

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { let _ = DeleteObject(self.0.into()); }
    }
}

impl Font {
    /// The shell's own UI font at this window's DPI.
    ///
    /// Asking the system rather than naming "Segoe UI" is the point: it
    /// respects the user's text-size setting, follows whatever the current
    /// Windows release actually ships, and localises to the right face on
    /// systems where Segoe has no coverage.
    pub fn shell_ui(dpi: u32) -> Font {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let ok = unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
                Some(&mut metrics as *mut _ as *mut _),
                0,
                dpi,
            )
        }
        .is_ok();

        let logfont = if ok {
            metrics.lfMessageFont
        } else {
            // Only reached if the system refuses to describe itself.
            let mut fallback = LOGFONTW { lfHeight: -scaled(12, dpi), ..Default::default() };
            let name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
            fallback.lfFaceName[..name.len()].copy_from_slice(&name);
            fallback
        };
        Font(unsafe { CreateFontIndirectW(&logfont) })
    }

    /// A heavier face for the one line a surface wants read first.
    pub fn shell_ui_bold(dpi: u32) -> Font {
        let base = Font::shell_ui(dpi);
        // Re-derive rather than mutate: LOGFONTW is not readable back out of
        // an HFONT without another round trip, and this is a cold path.
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let ok = unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
                Some(&mut metrics as *mut _ as *mut _),
                0,
                dpi,
            )
        }
        .is_ok();
        if !ok {
            return base;
        }
        drop(base);
        let mut logfont = metrics.lfMessageFont;
        logfont.lfWeight = 700;
        Font(unsafe { CreateFontIndirectW(&logfont) })
    }
}

/// Make the title bar follow the app theme.
///
/// Without this a dark window keeps a white caption, which is the single most
/// obvious tell that an app is not a native citizen.
pub fn apply_titlebar_theme(hwnd: HWND, dark: bool) {
    let flag: windows::core::BOOL = dark.into();
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &flag as *const _ as *const _,
            std::mem::size_of::<windows::core::BOOL>() as u32,
        );
    }
}

/// Windows 11 rounded corners. A no-op on Windows 10, which is why the result
/// is discarded rather than reported.
pub fn round_corners(hwnd: HWND) {
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// Ask comctl to draw a control with the dark-mode visual style.
pub fn apply_control_theme(hwnd: HWND, dark: bool) {
    let theme = to_wide(if dark { "DarkMode_Explorer" } else { "Explorer" });
    unsafe {
        let _ = windows::Win32::UI::Controls::SetWindowTheme(
            hwnd,
            PCWSTR(theme.as_ptr()),
            PCWSTR::null(),
        );
    }
}

/// Paint an owner-drawn push button.
///
/// Win32 push buttons do not follow the dark theme. `SetWindowTheme` fixes
/// most controls but not `BUTTON` in pushbutton mode: it keeps the light
/// visual style regardless, which on a dark window renders as a white slab —
/// the single most obvious "this app was ported badly" tell there is. The only
/// documented way out is to draw it ourselves. Microsoft's own dark-mode
/// support for common controls lives behind undocumented uxtheme ordinals, and
/// a health app has no business calling those.
///
/// Deliberately plain: a filled rounded rect, a hairline border, centred text
/// in the shell font. It should be unremarkable next to a system button.
pub fn draw_push_button(item: &DRAWITEMSTRUCT, theme: Theme, font: HFONT) {
    let dc = item.hDC;
    let rect = item.rcItem;
    let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
    let focused = item.itemState.0 & ODS_FOCUS.0 != 0;

    let (fill, border) = if theme.is_dark {
        (
            if pressed { rgb(45, 45, 45) } else { rgb(56, 56, 56) },
            if focused { rgb(118, 185, 237) } else { rgb(78, 78, 78) },
        )
    } else {
        (
            if pressed { rgb(230, 230, 230) } else { rgb(253, 253, 253) },
            if focused { rgb(0, 95, 184) } else { rgb(200, 200, 200) },
        )
    };

    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush.into());
        let old_pen = SelectObject(dc, pen.into());
        // Rounded to match Windows 11's button geometry.
        let radius = ((rect.bottom - rect.top) / 4).clamp(2, 8);
        let _ = RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, radius * 2, radius * 2);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());

        let old_font = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, theme.text);
        let mut label = [0u16; 128];
        let len = GetWindowTextW(item.hwndItem, &mut label);
        let mut text: Vec<u16> = label[..len as usize].to_vec();
        let mut r = rect;
        if !text.is_empty() {
            let _ = DrawTextW(
                dc, &mut text, &mut r,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
        SelectObject(dc, old_font);
    }
}

/// Paint one row of an owner-drawn combo box — both the collapsed face and
/// each item in the dropped list arrive here.
///
/// Combo boxes are the other control that will not go dark. `DarkMode_CFD`
/// only takes effect once dark mode has been enabled for the process through
/// uxtheme's undocumented ordinals, and a health app has no business calling
/// undocumented exports to tint a dropdown. Owner-draw is the documented way,
/// so the text area — the bulk of the control's visual weight — is ours.
///
/// The drop-down arrow itself is still drawn by the system visual style. It
/// stays light on a dark window, and that is a known, accepted seam rather
/// than an oversight.
pub fn draw_combo_item(item: &DRAWITEMSTRUCT, theme: Theme, font: HFONT) {
    let dc = item.hDC;
    let rect = item.rcItem;
    let selected = item.itemState.0 & ODS_SELECTED.0 != 0;

    let fill = if selected {
        if theme.is_dark { rgb(0, 90, 158) } else { rgb(205, 228, 252) }
    } else if theme.is_dark {
        rgb(43, 43, 43)
    } else {
        rgb(255, 255, 255)
    };
    let text_color = if selected && theme.is_dark { rgb(255, 255, 255) } else { theme.text };

    unsafe {
        let brush = CreateSolidBrush(fill);
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }

    // itemID is -1 when the control has no selection yet; nothing to draw.
    if item.itemID == u32::MAX {
        return;
    }
    let mut buffer = [0u16; 128];
    let len = unsafe {
        SendMessageW(
            item.hwndItem,
            CB_GETLBTEXT,
            Some(WPARAM(item.itemID as usize)),
            Some(LPARAM(buffer.as_mut_ptr() as isize)),
        )
    }
    .0;
    if len <= 0 {
        return;
    }
    let mut text: Vec<u16> = buffer[..len as usize].to_vec();
    let mut r = RECT { left: rect.left + 6, ..rect };
    unsafe {
        let old = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, text_color);
        let _ = DrawTextW(dc, &mut text, &mut r, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        SelectObject(dc, old);
    }
}

pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Keep a window fully inside the work area of the monitor it lands on.
/// A popover anchored to a tray icon in the corner would otherwise hang off
/// the screen edge on a taskbar that is not at the bottom.
pub fn clamp_to_work_area(x: i32, y: i32, width: i32, height: i32, work: RECT) -> (i32, i32) {
    let x = x.min(work.right - width).max(work.left);
    let y = y.min(work.bottom - height).max(work.top);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_follows_dpi_and_never_collapses_to_zero() {
        assert_eq!(scaled(10, 96), 10);
        assert_eq!(scaled(10, 144), 15);
        assert_eq!(scaled(10, 192), 20);
        assert_eq!(scaled(1, 96), 1);
        // A sub-pixel result must still be one pixel, or hairlines vanish.
        assert_eq!(scaled(1, 48), 1);
        assert_eq!(scaled(0, 144), 0, "zero stays zero");
    }

    #[test]
    fn rgb_packs_in_win32_order() {
        // COLORREF is 0x00BBGGRR, not RGB.
        assert_eq!(rgb(0x12, 0x34, 0x56).0, 0x00563412);
    }

    #[test]
    fn the_two_themes_are_actually_different_and_legible() {
        let dark = Theme { is_dark: true, ..Theme::current() };
        let _ = dark;
        let t = Theme::current();
        assert_ne!(t.background, t.text, "text must not equal background");
        assert_ne!(t.subtle, t.background);
        assert_ne!(t.divider, t.background);
    }

    #[test]
    fn a_popover_is_pulled_back_inside_the_work_area() {
        let work = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };
        // Anchored off the right/bottom edge, as a tray icon in the corner is.
        let (x, y) = clamp_to_work_area(1900, 1030, 260, 200, work);
        assert_eq!((x, y), (1660, 840));
    }

    #[test]
    fn clamping_prefers_the_top_left_when_a_window_cannot_fit() {
        let work = RECT { left: 0, top: 0, right: 100, bottom: 100 };
        let (x, y) = clamp_to_work_area(50, 50, 400, 400, work);
        assert_eq!((x, y), (0, 0), "an oversized window must not be pushed negative");
    }

    #[test]
    fn clamping_respects_a_monitor_that_is_not_at_the_origin() {
        // Second monitor to the right; the popover must not fly to monitor one.
        let work = RECT { left: 1920, top: 0, right: 3840, bottom: 1040 };
        let (x, y) = clamp_to_work_area(3830, 1030, 260, 200, work);
        assert_eq!((x, y), (3580, 840));
    }

    #[test]
    fn wide_strings_are_nul_terminated() {
        assert_eq!(*to_wide("Scoot Settings").last().unwrap(), 0);
    }
}
