//! The Settings window — the port of the macOS `SettingsView`.
//!
//! Pure Layer 1 (DESIGN.md §1): real Win32 controls, the shell's own font, the
//! user's theme, standard keyboard behaviour. **Restraint is the aesthetic** —
//! PRODUCT.md §7 calls the v1 settings surface "deliberately small", and the
//! design heuristic is "would this look native inside Settings?". So there is
//! no branding here, no custom-drawn control, and nothing invented.
//!
//! Sections mirror macOS one for one: Rhythm, Nudge styles, Buddy, System.
//! Only one window ever exists; asking again brings the existing one forward,
//! because two Settings windows disagreeing about the same JSON file is a bug
//! waiting to happen.

use std::cell::RefCell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, SetBkMode, SetTextColor, HDC, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Controls::ODT_COMBOBOX;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::storage::settings::{
    OverlayCorner, Settings, INTERVAL_CHOICES, SCALE_CHOICES, STYLE_BUDDY_OVERLAY,
    STYLE_ICON_BOUNCE, STYLE_SOUND,
};
use crate::ui::{self, Brush, Font, Theme};

const WIDTH: i32 = 420;
const PAD: i32 = 20;
const ROW: i32 = 30;
const GAP: i32 = 10;
const LABEL_W: i32 = 150;

// Control ids. The style rows are contiguous so the handler can index them.
const ID_INTERVAL: i32 = 2001;
const ID_CORNER: i32 = 2002;
const ID_SIZE: i32 = 2003;
const ID_LAUNCH: i32 = 2004;
const ID_TELEMETRY: i32 = 2005;
const ID_REVEAL: i32 = 2006;
const ID_STYLE_BASE: i32 = 2100;
const ID_PREVIEW_BASE: i32 = 2200;

/// The three styles, in registration order — the order the dispatcher fires
/// them and the order macOS lists them.
const STYLE_ROWS: [(&str, &str); 3] = [
    (STYLE_BUDDY_OVERLAY, "Buddy drop-in"),
    (STYLE_SOUND, "Chime"),
    (STYLE_ICON_BOUNCE, "Menu bar bounce"),
];

thread_local! {
    static WINDOW: RefCell<Option<SettingsState>> = const { RefCell::new(None) };
}

struct SettingsState {
    hwnd: HWND,
    theme: Theme,
    font: Font,
    background: Brush,
}

/// Open Settings, or raise it if it is already open.
pub fn open(settings: &Settings, launch_at_login: bool) {
    if let Some(existing) = WINDOW.with(|w| w.borrow().as_ref().map(|s| s.hwnd)) {
        unsafe {
            let _ = ShowWindow(existing, SW_RESTORE);
            let _ = SetForegroundWindow(existing);
        }
        return;
    }
    let _ = create(settings, launch_at_login);
}

pub fn close() {
    if let Some(hwnd) = WINDOW.with(|w| w.borrow().as_ref().map(|s| s.hwnd)) {
        unsafe { let _ = DestroyWindow(hwnd); }
    }
}

fn create(settings: &Settings, launch_at_login: bool) -> Result<(), String> {
    let class = ui::to_wide("ScootSettings");
    let title = ui::to_wide("Scoot Settings");
    let instance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map_err(|e| format!("GetModuleHandle: {e}"))?
    };
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(settings_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }

    // Not resizable and not maximizable: the layout is a fixed form, and a
    // stretched form with controls stranded at the top left is the classic
    // tell of a port that did not think about it.
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            CW_USEDEFAULT, CW_USEDEFAULT, 100, 100,
            None, None, Some(instance.into()), None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))?
    };

    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let theme = Theme::current();
    ui::apply_titlebar_theme(hwnd, theme.is_dark);
    let font = Font::shell_ui(dpi);
    let height = build_controls(hwnd, dpi, &font, theme, settings, launch_at_login);

    // Size the client area to the form, not the window frame.
    let mut want = RECT { left: 0, top: 0, right: ui::scaled(WIDTH, dpi), bottom: height };
    unsafe {
        let _ = AdjustWindowRectEx(&mut want, style, false, Default::default());
        let _ = SetWindowPos(
            hwnd, None, 0, 0,
            want.right - want.left, want.bottom - want.top,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }

    WINDOW.with(|w| {
        *w.borrow_mut() = Some(SettingsState {
            hwnd,
            theme,
            font,
            background: Brush::solid(theme.background),
        })
    });
    Ok(())
}

/// Lay the form out top to bottom, returning the height it needs.
fn build_controls(
    hwnd: HWND,
    dpi: u32,
    font: &Font,
    theme: Theme,
    settings: &Settings,
    launch_at_login: bool,
) -> i32 {
    let s = |v: i32| ui::scaled(v, dpi);
    let mut y = s(PAD);
    let full = s(WIDTH) - 2 * s(PAD);

    let mut add = |class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32| -> HWND {
        let cls = ui::to_wide(class);
        let label = ui::to_wide(text);
        let hmenu = if id != 0 { Some(HMENU(id as *mut _)) } else { None };
        let control = unsafe {
            CreateWindowExW(
                Default::default(),
                PCWSTR(cls.as_ptr()),
                PCWSTR(label.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
                x, y, w, h,
                Some(hwnd), hmenu, None, None,
            )
        }
        .unwrap_or_default();
        if !control.is_invalid() {
            unsafe {
                SendMessageW(control, WM_SETFONT, Some(WPARAM(font.0 .0 as usize)), Some(LPARAM(1)));
            }
            ui::apply_control_theme(control, theme.is_dark);
        }
        control
    };

    let mut header = |add: &mut dyn FnMut(&str, &str, u32, i32, i32, i32, i32, i32) -> HWND,
                      text: &str,
                      y: &mut i32| {
        add("STATIC", text, 0, s(PAD), *y, full, s(20), 0);
        *y += s(24);
    };

    // --- Rhythm ---
    header(&mut add, "Rhythm", &mut y);
    add("STATIC", "Remind me every", 0, s(PAD), y + s(4), s(LABEL_W), s(20), 0);
    let combo = add(
        "COMBOBOX", "",
        (CBS_DROPDOWNLIST as u32) | (CBS_OWNERDRAWFIXED as u32) | (CBS_HASSTRINGS as u32) | (WS_TABSTOP.0) | (WS_VSCROLL.0),
        s(PAD) + s(LABEL_W), y, full - s(LABEL_W), s(ROW) * 8, ID_INTERVAL,
    );
    for (index, minutes) in INTERVAL_CHOICES.iter().enumerate() {
        let label = if *minutes == 1 {
            "1 minute (testing)".to_string()
        } else {
            format!("{minutes} minutes")
        };
        let w = ui::to_wide(&label);
        unsafe {
            SendMessageW(combo, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            if *minutes == settings.interval_minutes {
                SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(index)), None);
            }
        }
    }
    y += s(ROW) + s(GAP);

    // --- Nudge styles ---
    header(&mut add, "Nudge styles", &mut y);
    for (index, (id, display)) in STYLE_ROWS.iter().enumerate() {
        let check = add(
            "BUTTON", display,
            (BS_AUTOCHECKBOX as u32) | (WS_TABSTOP.0),
            s(PAD), y + s(4), full - s(90), s(22),
            ID_STYLE_BASE + index as i32,
        );
        if settings.is_style_enabled(id) {
            unsafe { SendMessageW(check, BM_SETCHECK, Some(WPARAM(1)), None); }
        }
        add(
            "BUTTON", "Preview",
            (BS_OWNERDRAW as u32) | (WS_TABSTOP.0),
            s(WIDTH) - s(PAD) - s(80), y, s(80), s(26),
            ID_PREVIEW_BASE + index as i32,
        );
        y += s(ROW);
    }
    y += s(GAP);

    // --- Buddy ---
    header(&mut add, "Buddy", &mut y);
    add("STATIC", "Corner", 0, s(PAD), y + s(4), s(LABEL_W), s(20), 0);
    let corner = add(
        "COMBOBOX", "",
        (CBS_DROPDOWNLIST as u32) | (CBS_OWNERDRAWFIXED as u32) | (CBS_HASSTRINGS as u32) | (WS_TABSTOP.0) | (WS_VSCROLL.0),
        s(PAD) + s(LABEL_W), y, full - s(LABEL_W), s(ROW) * 6, ID_CORNER,
    );
    for (index, option) in OverlayCorner::ALL.iter().enumerate() {
        let w = ui::to_wide(option.label());
        unsafe {
            SendMessageW(corner, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            if *option == settings.corner() {
                SendMessageW(corner, CB_SETCURSEL, Some(WPARAM(index)), None);
            }
        }
    }
    y += s(ROW);

    add("STATIC", "Size", 0, s(PAD), y + s(4), s(LABEL_W), s(20), 0);
    let size = add(
        "COMBOBOX", "",
        (CBS_DROPDOWNLIST as u32) | (CBS_OWNERDRAWFIXED as u32) | (CBS_HASSTRINGS as u32) | (WS_TABSTOP.0) | (WS_VSCROLL.0),
        s(PAD) + s(LABEL_W), y, full - s(LABEL_W), s(ROW) * 5, ID_SIZE,
    );
    for (index, (scale, label)) in SCALE_CHOICES.iter().enumerate() {
        let w = ui::to_wide(label);
        unsafe {
            SendMessageW(size, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            if *scale == settings.buddy_scale {
                SendMessageW(size, CB_SETCURSEL, Some(WPARAM(index)), None);
            }
        }
    }
    y += s(ROW) + s(GAP);

    // --- System ---
    header(&mut add, "System", &mut y);
    let launch = add(
        "BUTTON", "Launch at login",
        (BS_AUTOCHECKBOX as u32) | (WS_TABSTOP.0),
        s(PAD), y, full, s(22), ID_LAUNCH,
    );
    if launch_at_login {
        unsafe { SendMessageW(launch, BM_SETCHECK, Some(WPARAM(1)), None); }
    }
    y += s(26);

    let telemetry = add(
        "BUTTON", "Share anonymous counts",
        (BS_AUTOCHECKBOX as u32) | (WS_TABSTOP.0),
        s(PAD), y, full, s(22), ID_TELEMETRY,
    );
    if settings.telemetry_enabled {
        unsafe { SendMessageW(telemetry, BM_SETCHECK, Some(WPARAM(1)), None); }
    }
    y += s(24);

    // The honest one-sentence disclosure, matching the macOS wording but
    // truthful about where the data goes on this platform (nowhere).
    add(
        "STATIC",
        "Anonymous counts only \u{2014} nudges shown, moves credited. Never content, \
         never keystrokes. Nothing leaves this PC; the log below is all there is.",
        0,
        s(PAD), y, full, s(64), 0,
    );
    y += s(68);

    add(
        "BUTTON", "Reveal local event log",
        (BS_OWNERDRAW as u32) | (WS_TABSTOP.0),
        s(PAD), y, s(180), s(28), ID_REVEAL,
    );
    y += s(28) + s(PAD);
    y
}

extern "system" fn settings_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            handle_command(hwnd, id, code, HWND(lparam.0 as *mut _));
            LRESULT(0)
        }
        // Standard controls do not follow the app theme on their own; without
        // these a dark window shows white boxes behind every label.
        WM_DRAWITEM => {
            let item = unsafe { &*(lparam.0 as *const windows::Win32::UI::Controls::DRAWITEMSTRUCT) };
            WINDOW.with(|w| {
                if let Some(state) = w.borrow().as_ref() {
                    if item.CtlType == ODT_COMBOBOX {
                        ui::draw_combo_item(item, state.theme, state.font.0);
                    } else {
                        ui::draw_push_button(item, state.theme, state.font.0);
                    }
                }
            });
            LRESULT(1)
        }
        // Owner-drawn combos must be told how tall a row is, or the list
        // collapses to the default and the text is clipped.
        WM_MEASUREITEM => {
            let item = unsafe { &mut *(lparam.0 as *mut windows::Win32::UI::Controls::MEASUREITEMSTRUCT) };
            let dpi = WINDOW
                .with(|w| w.borrow().as_ref().map(|s| unsafe { GetDpiForWindow(s.hwnd) }))
                .unwrap_or(96)
                .max(96);
            item.itemHeight = ui::scaled(22, dpi) as u32;
            LRESULT(1)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG => {
            let dc = HDC(wparam.0 as *mut _);
            WINDOW.with(|w| {
                let borrowed = w.borrow();
                let Some(state) = borrowed.as_ref() else {
                    return LRESULT(0);
                };
                unsafe {
                    SetTextColor(dc, state.theme.text);
                    SetBkMode(dc, TRANSPARENT);
                }
                LRESULT(state.background.0 .0 as isize)
            })
        }
        WM_ERASEBKGND => {
            let dc = HDC(wparam.0 as *mut _);
            let mut rect = RECT::default();
            unsafe { let _ = GetClientRect(hwnd, &mut rect); }
            let theme = WINDOW
                .with(|w| w.borrow().as_ref().map(|s| s.theme))
                .unwrap_or_else(Theme::current);
            let brush = unsafe { CreateSolidBrush(theme.background) };
            unsafe {
                windows::Win32::Graphics::Gdi::FillRect(dc, &rect, brush);
                let _ = DeleteObject(brush.into());
            }
            LRESULT(1)
        }
        WM_CLOSE => {
            unsafe { let _ = DestroyWindow(hwnd); }
            LRESULT(0)
        }
        WM_DESTROY => {
            WINDOW.with(|w| *w.borrow_mut() = None);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn handle_command(_hwnd: HWND, id: i32, code: u32, control: HWND) {
    // Combo boxes report many things; only a completed selection is a change.
    let selection_changed = code == CBN_SELCHANGE;
    match id {
        ID_INTERVAL if selection_changed => {
            let index = unsafe { SendMessageW(control, CB_GETCURSEL, None, None) }.0;
            if let Some(minutes) = INTERVAL_CHOICES.get(index as usize) {
                crate::app::apply_setting(crate::app::SettingChange::Interval(*minutes));
            }
        }
        ID_CORNER if selection_changed => {
            let index = unsafe { SendMessageW(control, CB_GETCURSEL, None, None) }.0;
            if let Some(corner) = OverlayCorner::ALL.get(index as usize) {
                crate::app::apply_setting(crate::app::SettingChange::Corner(*corner));
            }
        }
        ID_SIZE if selection_changed => {
            let index = unsafe { SendMessageW(control, CB_GETCURSEL, None, None) }.0;
            if let Some((scale, _)) = SCALE_CHOICES.get(index as usize) {
                crate::app::apply_setting(crate::app::SettingChange::Scale(*scale));
            }
        }
        ID_LAUNCH => {
            let on = unsafe { SendMessageW(control, BM_GETCHECK, None, None) }.0 == 1;
            crate::app::apply_setting(crate::app::SettingChange::LaunchAtLogin(on));
        }
        ID_TELEMETRY => {
            let on = unsafe { SendMessageW(control, BM_GETCHECK, None, None) }.0 == 1;
            crate::app::apply_setting(crate::app::SettingChange::Telemetry(on));
        }
        ID_REVEAL => crate::app::dispatch_command(crate::tray::CMD_REVEAL_LOG),
        _ => {
            if let Some(index) = index_in(id, ID_STYLE_BASE, STYLE_ROWS.len()) {
                let on = unsafe { SendMessageW(control, BM_GETCHECK, None, None) }.0 == 1;
                crate::app::apply_setting(crate::app::SettingChange::Style(
                    STYLE_ROWS[index].0,
                    on,
                ));
            } else if let Some(index) = index_in(id, ID_PREVIEW_BASE, STYLE_ROWS.len()) {
                crate::app::preview_style(STYLE_ROWS[index].0);
            }
        }
    }
}

fn index_in(id: i32, base: i32, count: usize) -> Option<usize> {
    let offset = id.checked_sub(base)?;
    (offset >= 0 && (offset as usize) < count).then_some(offset as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_the_macos_width() {
        assert_eq!(WIDTH, 420, "matches SettingsView's 420pt form");
    }

    #[test]
    fn the_style_rows_are_the_contract_ids_in_registration_order() {
        assert_eq!(STYLE_ROWS[0].0, STYLE_BUDDY_OVERLAY);
        assert_eq!(STYLE_ROWS[1].0, STYLE_SOUND);
        assert_eq!(STYLE_ROWS[2].0, STYLE_ICON_BOUNCE);
    }

    #[test]
    fn the_display_names_match_the_styles_themselves() {
        assert_eq!(STYLE_ROWS[0].1, "Buddy drop-in");
        assert_eq!(STYLE_ROWS[1].1, "Chime");
        assert_eq!(STYLE_ROWS[2].1, "Menu bar bounce");
    }

    #[test]
    fn control_ids_do_not_collide_across_families() {
        let mut seen = std::collections::HashSet::new();
        for id in [ID_INTERVAL, ID_CORNER, ID_SIZE, ID_LAUNCH, ID_TELEMETRY, ID_REVEAL] {
            assert!(seen.insert(id), "duplicate id {id}");
        }
        for index in 0..STYLE_ROWS.len() as i32 {
            assert!(seen.insert(ID_STYLE_BASE + index));
            assert!(seen.insert(ID_PREVIEW_BASE + index));
        }
    }

    #[test]
    fn ranged_ids_only_match_inside_their_family() {
        assert_eq!(index_in(ID_STYLE_BASE, ID_STYLE_BASE, 3), Some(0));
        assert_eq!(index_in(ID_STYLE_BASE + 2, ID_STYLE_BASE, 3), Some(2));
        assert_eq!(index_in(ID_STYLE_BASE + 3, ID_STYLE_BASE, 3), None);
        assert_eq!(index_in(ID_INTERVAL, ID_STYLE_BASE, 3), None);
        assert_eq!(index_in(ID_PREVIEW_BASE, ID_STYLE_BASE, 3), None);
    }

    #[test]
    fn every_interval_the_menu_offers_is_representable() {
        // The combo is populated from the same constant the scheduler clamps
        // against, so a choice can never be unselectable.
        for minutes in INTERVAL_CHOICES {
            assert!(minutes >= 1 && minutes <= 24 * 60);
        }
    }
}
