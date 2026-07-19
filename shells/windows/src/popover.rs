//! The left-click popover — Scoot's face when you go looking for it.
//!
//! This is the port of the macOS `PopoverView`, and it is a *window*, not a
//! menu. That distinction is the design decision (DESIGN.md §5, the menu-bar
//! card): the thing you open should show you your buddy and tell you when the
//! next nudge is, not present a list of verbs. A dropdown would be a faster
//! build and a worse product.
//!
//! Two registers meet here, exactly as DESIGN.md §1 intends. The frame is
//! Layer 1 — system font, system colours, rounded corners, dismisses on
//! deactivate like every other flyout. The portrait inside it is Layer 2 —
//! pixel art at integer scale, nearest-neighbor, no smoothing. The contrast is
//! the point.
//!
//! Like the macOS popover, the content is built on show and torn down on
//! close. A retained view with a live animation timer costs CPU forever after
//! the first open, which is the bug the macOS side documents at
//! `StatusItemController.swift:16`.

use std::cell::RefCell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, SelectObject,
    SetBkMode, SetTextColor, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, COLORONCOLOR,
    DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HDC, InvalidateRect, PAINTSTRUCT,
    SRCCOPY, SetStretchBltMode, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::nudge::overlay::scale_nearest;
use crate::render::sprite::{Frame, SpriteSheet};
use crate::tray;
use crate::ui::{self, Font, Theme};

/// Logical metrics, mirroring the 260pt macOS popover.
const WIDTH: i32 = 260;
const PAD: i32 = 16;
const PORTRAIT_BOX: i32 = 72;
const BUTTON_H: i32 = 30;
const GAP: i32 = 8;

/// The portrait's own animation rate. Slow on purpose — the macOS popover
/// passes `fps: 4`. A portrait dancing at full speed reads as agitated when
/// you have only opened the window to check the time.
const PORTRAIT_FPS: f64 = 4.0;

const TIMER_TICK: usize = 1;

/// Control ids, distinct from the tray command range so a stray WM_COMMAND
/// cannot be mistaken for a menu choice.
const ID_NUDGE_NOW: i32 = 1001;
const ID_PAUSE_RESUME: i32 = 1002;
const ID_SETTINGS: i32 = 1003;
const ID_QUIT: i32 = 1004;

thread_local! {
    static STATE: RefCell<Option<PopoverState>> = const { RefCell::new(None) };
}

struct PopoverState {
    hwnd: HWND,
    frames: Vec<Frame>,
    opened_at: f64,
    status: String,
    is_paused: bool,
    theme: Theme,
    font: Font,
    bold: Font,
    dpi: u32,
}

/// What the popover needs to render itself, snapshotted at open time.
pub struct PopoverModel {
    pub status: String,
    pub is_paused: bool,
    pub sheet_png: &'static [u8],
    pub sheet_json: &'static [u8],
}

/// Open the popover next to the tray icon, or close it if it is already up
/// (clicking the tray icon again should dismiss, like every other flyout).
pub fn toggle(owner: HWND, model: PopoverModel) {
    if is_open() {
        close();
        return;
    }
    let _ = open(owner, model);
}

pub fn is_open() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

pub fn close() {
    let hwnd = STATE.with(|s| s.borrow().as_ref().map(|p| p.hwnd));
    if let Some(hwnd) = hwnd {
        unsafe { let _ = DestroyWindow(hwnd); }
    }
}

/// Refresh the countdown without rebuilding the window.
pub fn set_status(status: String, is_paused: bool) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            let changed = state.status != status || state.is_paused != is_paused;
            state.status = status;
            state.is_paused = is_paused;
            if changed {
                unsafe { let _ = InvalidateRect(Some(state.hwnd), None, true); }
            }
        }
    });
}

fn open(owner: HWND, model: PopoverModel) -> Result<(), String> {
    let sheet = SpriteSheet::load(model.sheet_png, model.sheet_json)?;
    let class = ui::to_wide("ScootPopover");
    let title = ui::to_wide("Scoot");

    let instance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map_err(|e| format!("GetModuleHandle: {e}"))?
    };
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(popover_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }

    // WS_EX_TOOLWINDOW keeps it out of alt-tab; it is a flyout, not a document.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_BORDER,
            0, 0, 10, 10,
            Some(owner),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))?
    };

    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let theme = Theme::current();
    ui::round_corners(hwnd);
    ui::apply_titlebar_theme(hwnd, theme.is_dark);

    let idle = sheet.idle_frame_index();
    let frames = if sheet.frames.is_empty() { Vec::new() } else { sheet.frames.clone() };
    let _ = idle;

    let (w, h) = layout_size(dpi);
    let (x, y) = anchor(owner, w, h);
    unsafe {
        SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE).ok();
    }

    let font = Font::shell_ui(dpi);
    let bold = Font::shell_ui_bold(dpi);
    create_buttons(hwnd, dpi, &font, theme, model.is_paused);

    STATE.with(|s| {
        *s.borrow_mut() = Some(PopoverState {
            hwnd,
            frames,
            opened_at: crate::time::monotonic_seconds(),
            status: model.status,
            is_paused: model.is_paused,
            theme,
            font,
            bold,
            dpi,
        })
    });

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        // Foreground so that clicking away deactivates us and we self-dismiss.
        let _ = SetForegroundWindow(hwnd);
        SetTimer(Some(hwnd), TIMER_TICK, 250, None);
    }
    Ok(())
}

fn layout_size(dpi: u32) -> (i32, i32) {
    let s = |v: i32| ui::scaled(v, dpi);
    let height = s(PAD) + s(PORTRAIT_BOX) + s(GAP) + s(20) + s(GAP)
        + s(BUTTON_H) + s(GAP) + s(1) + s(GAP) + s(BUTTON_H) + s(PAD);
    (s(WIDTH), height)
}

/// Put the popover against the tray icon it belongs to, falling back to the
/// cursor if the shell will not say where the icon is.
fn anchor(owner: HWND, w: i32, h: i32) -> (i32, i32) {
    let id = NOTIFYICONIDENTIFIER {
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: owner,
        uID: 1,
        ..Default::default()
    };
    // Anchor to the icon itself when the shell will say where it is; the
    // cursor is only a fallback, because on a click the two coincide anyway
    // and on a keyboard-invoked open they would not.
    let anchor_point = match unsafe { Shell_NotifyIconGetRect(&id) } {
        Ok(icon) => POINT { x: (icon.left + icon.right) / 2, y: icon.top },
        Err(_) => {
            let mut cursor = POINT::default();
            unsafe { let _ = GetCursorPos(&mut cursor); }
            cursor
        }
    };

    let monitor = unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromPoint(
            anchor_point,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = windows::Win32::Graphics::Gdi::MONITORINFO {
        cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
        ..Default::default()
    };
    let work = if unsafe { windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
    {
        info.rcWork
    } else {
        RECT { left: 0, top: 0, right: 1920, bottom: 1040 }
    };

    // Above the icon, centred on it — the usual place for a tray flyout.
    ui::clamp_to_work_area(anchor_point.x - w / 2, anchor_point.y - h - 8, w, h, work)
}

fn create_buttons(hwnd: HWND, dpi: u32, font: &Font, theme: Theme, is_paused: bool) {
    let s = |v: i32| ui::scaled(v, dpi);
    let inner = s(WIDTH) - 2 * s(PAD);
    let half = (inner - s(GAP)) / 2;
    let verbs_y = s(PAD) + s(PORTRAIT_BOX) + s(GAP) + s(20) + s(GAP);
    let footer_y = verbs_y + s(BUTTON_H) + s(GAP) + s(1) + s(GAP);

    let mk = |text: &str, id: i32, x: i32, y: i32, w: i32, default: bool| {
        let label = ui::to_wide(text);
        // Owner-drawn because Win32 push buttons ignore the dark theme; see
        // ui::draw_push_button.
        let _ = default;
        let style = WINDOW_STYLE(
            (WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0) | BS_OWNERDRAW as u32,
        );
        let btn = unsafe {
            CreateWindowExW(
                Default::default(),
                PCWSTR(ui::to_wide("BUTTON").as_ptr()),
                PCWSTR(label.as_ptr()),
                style,
                x, y, w, s(BUTTON_H),
                Some(hwnd),
                Some(HMENU(id as *mut _)),
                None,
                None,
            )
        };
        if let Ok(btn) = btn {
            unsafe {
                SendMessageW(btn, WM_SETFONT, Some(WPARAM(font.0 .0 as usize)), Some(LPARAM(1)));
            }
            ui::apply_control_theme(btn, theme.is_dark);
        }
    };

    mk("Nudge Now", ID_NUDGE_NOW, s(PAD), verbs_y, half, true);
    mk(
        if is_paused { "Resume" } else { "Pause 1 Hour" },
        ID_PAUSE_RESUME,
        s(PAD) + half + s(GAP),
        verbs_y,
        half,
        false,
    );
    mk("Settings\u{2026}", ID_SETTINGS, s(PAD), footer_y, half, false);
    mk("Quit", ID_QUIT, s(PAD) + half + s(GAP), footer_y, half, false);
}

extern "system" fn popover_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_TIMER => {
            // Only the portrait animates; the buttons are static, so repaint
            // just the sprite box rather than the whole window.
            STATE.with(|s| {
                if let Some(state) = s.borrow().as_ref() {
                    let sp = ui::scaled(PAD, state.dpi);
                    let box_px = ui::scaled(PORTRAIT_BOX, state.dpi);
                    let width = ui::scaled(WIDTH, state.dpi);
                    let rect = RECT {
                        left: 0,
                        top: sp,
                        right: width,
                        bottom: sp + box_px + ui::scaled(GAP + 20, state.dpi),
                    };
                    unsafe { let _ = InvalidateRect(Some(hwnd), Some(&rect), false); }
                }
            });
            LRESULT(0)
        }
        // Dismiss when focus goes elsewhere — the flyout contract.
        WM_ACTIVATE if wparam.0 as u32 & 0xFFFF == WA_INACTIVE => {
            unsafe { let _ = DestroyWindow(hwnd); }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let command = match id {
                ID_NUDGE_NOW => Some(tray::CMD_NUDGE_NOW),
                ID_PAUSE_RESUME => Some(
                    STATE
                        .with(|s| s.borrow().as_ref().map(|p| p.is_paused))
                        .unwrap_or(false)
                        .then_some(tray::CMD_RESUME)
                        .unwrap_or(tray::CMD_PAUSE),
                ),
                ID_SETTINGS => Some(tray::CMD_SETTINGS),
                ID_QUIT => Some(tray::CMD_QUIT),
                _ => None,
            };
            if let Some(command) = command {
                // Close first: the command may open Settings, and a flyout
                // that lingers behind a window it spawned looks stuck.
                unsafe { let _ = DestroyWindow(hwnd); }
                crate::app::dispatch_command(command);
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let item = unsafe { &*(lparam.0 as *const windows::Win32::UI::Controls::DRAWITEMSTRUCT) };
            STATE.with(|s| {
                if let Some(state) = s.borrow().as_ref() {
                    ui::draw_push_button(item, state.theme, state.font.0);
                }
            });
            LRESULT(1)
        }
        WM_CTLCOLORBTN | WM_CTLCOLORSTATIC => {
            let theme = STATE.with(|s| s.borrow().as_ref().map(|p| p.theme)).unwrap_or_else(Theme::current);
            let dc = HDC(wparam.0 as *mut _);
            unsafe {
                SetTextColor(dc, theme.text);
                SetBkMode(dc, TRANSPARENT);
            }
            LRESULT(unsafe { CreateSolidBrush(theme.background) }.0 as isize)
        }
        WM_DESTROY => {
            unsafe { KillTimer(Some(hwnd), TIMER_TICK).ok() };
            STATE.with(|s| *s.borrow_mut() = None);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut ps) };
    STATE.with(|s| {
        let borrowed = s.borrow();
        let Some(state) = borrowed.as_ref() else { return };
        let sc = |v: i32| ui::scaled(v, state.dpi);
        let mut client = RECT::default();
        unsafe { let _ = GetClientRect(hwnd, &mut client); }

        let bg = unsafe { CreateSolidBrush(state.theme.background) };
        unsafe {
            FillRect(dc, &client, bg);
            let _ = DeleteObject(bg.into());
        }

        // Layer 2: the buddy, integer-scaled and nearest-neighbor.
        if !state.frames.is_empty() {
            let elapsed = crate::time::monotonic_seconds() - state.opened_at;
            let index = crate::nudge::overlay::frame_at(elapsed, PORTRAIT_FPS, state.frames.len());
            if let Some(frame) = state.frames.get(index) {
                let box_px = sc(PORTRAIT_BOX);
                let scale = (box_px / frame.height.max(1) as i32).max(1) as u32;
                let sprite = scale_nearest(frame, scale);
                let x = (client.right - sprite.width as i32) / 2;
                let y = sc(PAD) + (box_px - sprite.height as i32) / 2;
                blit_frame(dc, &sprite, x, y);
            }
        }

        // Layer 1: the status line, in the shell's own font.
        let text_top = sc(PAD) + sc(PORTRAIT_BOX) + sc(GAP);
        let mut text_rect = RECT {
            left: sc(PAD),
            top: text_top,
            right: client.right - sc(PAD),
            bottom: text_top + sc(20),
        };
        let old = unsafe { SelectObject(dc, state.bold.0.into()) };
        unsafe {
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, state.theme.text);
        }
        let mut line = ui::to_wide(&state.status);
        line.pop();
        unsafe {
            DrawTextW(dc, &mut line, &mut text_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
            SelectObject(dc, old);
        }

        // The hairline above the footer row.
        let divider_y = text_top + sc(20) + sc(GAP) + sc(BUTTON_H) + sc(GAP);
        let divider = RECT {
            left: sc(PAD),
            top: divider_y,
            right: client.right - sc(PAD),
            bottom: divider_y + sc(1),
        };
        let brush = unsafe { CreateSolidBrush(state.theme.divider) };
        unsafe {
            FillRect(dc, &divider, brush);
            let _ = DeleteObject(brush.into());
        }
    });
    unsafe { let _ = EndPaint(hwnd, &ps); }
}

/// Draw a decoded frame with no resampling whatsoever.
fn blit_frame(dc: HDC, frame: &Frame, x: i32, y: i32) {
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: frame.width as i32,
        biHeight: -(frame.height as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
    // BGRA for GDI, and pre-composited against the popover background so the
    // sprite's transparent margin does not paint black.
    let mut bgra = Vec::with_capacity(frame.rgba.len());
    let bg = crate::ui::Theme::current().background.0;
    let (br, bgc, bb) = ((bg & 0xFF) as u32, ((bg >> 8) & 0xFF) as u32, ((bg >> 16) & 0xFF) as u32);
    for px in frame.rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let mix = |c: u32, b: u32| (((c * a) + (b * (255 - a))) / 255) as u8;
        bgra.push(mix(px[2] as u32, bb));
        bgra.push(mix(px[1] as u32, bgc));
        bgra.push(mix(px[0] as u32, br));
        bgra.push(255);
    }
    unsafe {
        // COLORONCOLOR: drop pixels rather than average them. The scale is
        // already whole, so nothing is dropped — but if it ever were not,
        // this fails to a hard edge instead of to a blur (DESIGN.md §2).
        SetStretchBltMode(dc, COLORONCOLOR);
        StretchDIBits(
            dc, x, y, frame.width as i32, frame.height as i32,
            0, 0, frame.width as i32, frame.height as i32,
            Some(bgra.as_ptr() as *const _), &info, DIB_RGB_COLORS, SRCCOPY,
        );
    }
    let _ = SIZE::default();
    let _: COLORREF = COLORREF(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_popover_is_the_macos_width() {
        assert_eq!(WIDTH, 260, "DESIGN.md §5 specifies a 260pt popover");
    }

    #[test]
    fn layout_grows_with_dpi_and_stays_whole() {
        let (w96, h96) = layout_size(96);
        let (w144, h144) = layout_size(144);
        assert_eq!(w96, 260);
        assert_eq!(w144, 390, "260 at 150%");
        assert!(h144 > h96);
        assert!(h96 > 200, "must fit portrait, status and two button rows");
    }

    #[test]
    fn the_portrait_rests_slower_than_a_nudge_dance() {
        // A portrait at dance speed reads as agitated when you have only
        // opened the window to check the countdown.
        assert!(PORTRAIT_FPS < 8.0);
    }

    #[test]
    fn control_ids_cannot_be_mistaken_for_tray_commands() {
        for id in [ID_NUDGE_NOW, ID_PAUSE_RESUME, ID_SETTINGS, ID_QUIT] {
            assert!(id >= 1000, "control id {id} overlaps the tray command range");
        }
    }
}
