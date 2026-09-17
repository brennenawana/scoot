//! The hero nudge: a pixel buddy that hops into a corner and dances
//! (PRODUCT.md §3, PORTS.md §7).
//!
//! ## The window
//!
//! Borderless, topmost, layered, and — the part that matters — **not
//! activating**. `WS_EX_NOACTIVATE` plus `SWP_NOACTIVATE` means clicking the
//! buddy credits a scoot without taking keyboard focus from whatever the user
//! was typing in. A break reminder that eats a keystroke has failed.
//!
//! Click-through is solved by *geometry*, exactly as the macOS panel solves it:
//! the window is buddy-sized and lives in a corner, so everything outside it is
//! simply other apps' windows. There is no `WS_EX_TRANSPARENT` and no
//! full-screen overlay — that shape would put an invisible sheet of glass over
//! the whole desktop, and any bug in its hit-testing would eat real clicks.
//!
//! ## The pixels
//!
//! Constitutional (DESIGN.md §2): integer scale factors only, nearest-neighbor
//! only, integral origins. Fractional Windows display scaling is where that
//! rule gets tested — see `integer_scale_for`, which resolves 150% DPI by
//! picking the largest whole scale that fits rather than by scaling by 1.5.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GetMonitorInfoW,
    MonitorFromPoint, SelectObject, SetBkMode, SetTextColor, ANTIALIASED_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE,
    FW_SEMIBOLD, HDC, MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, RegisterClassW, SetWindowPos,
    ShowWindow, UpdateLayeredWindow, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA, SWP_NOACTIVATE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::render::dib::Dib;
use crate::render::sprite::{Frame, SpriteSheet};
use crate::storage::settings::OverlayCorner;

use super::{NudgeContext, NudgeOutcome, NudgeStyle, MAX_NUDGE_SECONDS};

/// Copy lives in PRODUCT.md §8 — warm, short, never guilt-adjacent.
const MSG_DANCING: &str = "Time to scoot.";
const MSG_ACKNOWLEDGED: &str = "Nice scoot.";
const MSG_AUTO_CREDIT: &str = "Saw you step away. +1 scoot.";

/// Post-outcome beats, matching the macOS timings exactly so the moment has
/// the same rhythm on both platforms.
const LINGER_ACKNOWLEDGED: f64 = 1.4;
const LINGER_MOVEMENT: f64 = 2.2;
const LINGER_TIMEOUT: f64 = 1.2;

/// Logical pixels between the buddy and the screen edge (design spec).
const CORNER_MARGIN: i32 = 24;
/// Logical gap between the speech bubble and the sprite.
const BUBBLE_GAP: i32 = 8;

/// Set by the overlay's window proc when the buddy is clicked. A plain flag
/// rather than a callback: the message loop and the poll both run on this one
/// thread, so there is no race to guard and no ownership cycle to untangle.
static CLICKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Hidden,
    /// The dance. Ends on a click, a credit, or `MAX_NUDGE_SECONDS`.
    Dancing { since: f64, fps: f64 },
    /// The outcome has already been reported; this is the visual goodbye.
    Lingering { until: f64, message: Option<&'static str>, fps: f64 },
}

pub struct BuddyOverlay {
    sheet: SpriteSheet,
    hwnd: Option<HWND>,
    stage: Stage,
    corner: OverlayCorner,
    logical_scale: i32,
    /// An outcome decided outside `poll` and owed to the dispatcher.
    ///
    /// `movement_credited` is called by the coordinator, not by the pump, so
    /// it cannot return an outcome directly. Parking it here lets the next
    /// `poll` deliver it — without which the style stays in the dispatcher's
    /// live set forever, the 66ms timer never stops, and the nudge window
    /// never gets its `nudge_outcome` line.
    pending: Option<NudgeOutcome>,
    /// The monitor rect chosen when the nudge fired. Resolved once so the
    /// buddy cannot teleport to another display if the mouse wanders
    /// mid-dance.
    stage_monitor: Option<(RECT, u32)>,
}

impl BuddyOverlay {
    pub fn new(sheet: SpriteSheet, corner: OverlayCorner, logical_scale: i32) -> Self {
        Self {
            sheet, hwnd: None, stage: Stage::Hidden, corner, logical_scale,
            pending: None, stage_monitor: None,
        }
    }

    /// Settings changed under us; the next nudge uses the new placement.
    pub fn apply_settings(&mut self, corner: OverlayCorner, logical_scale: i32) {
        self.corner = corner;
        self.logical_scale = logical_scale;
    }

    /// Pump the animation. Called every animation tick whether or not the
    /// dispatcher still considers the style live, because the linger beat
    /// outlives the outcome — the user is told "Nice scoot." *after* the
    /// credit has already been recorded.
    pub fn animate(&mut self, now: f64) {
        match self.stage {
            Stage::Hidden => {}
            Stage::Dancing { since, fps } => {
                self.draw(frame_at(now - since, fps, self.sheet.frames.len()), Some(MSG_DANCING));
            }
            Stage::Lingering { until, message, fps } => {
                if now >= until {
                    self.hide();
                    return;
                }
                self.draw(frame_at(now, fps, self.sheet.frames.len()), message);
            }
        }
    }

    fn begin_linger(&mut self, now: f64, outcome: NudgeOutcome) {
        let (seconds, message, fps) = match outcome {
            NudgeOutcome::Acknowledged => (LINGER_ACKNOWLEDGED, Some(MSG_ACKNOWLEDGED), 12.0),
            NudgeOutcome::MovementDetected => (LINGER_MOVEMENT, Some(MSG_AUTO_CREDIT), 12.0),
            // Ignored nudges slow to a gentle sway and leave. No sad state, no
            // frown — constitutionally (VISION.md §2).
            _ => (LINGER_TIMEOUT, None, 2.0),
        };
        self.stage = Stage::Lingering { until: now + seconds, message, fps };
    }

    fn hide(&mut self) {
        self.stage = Stage::Hidden;
        if let Some(hwnd) = self.hwnd.take() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    fn draw(&mut self, frame_index: usize, message: Option<&str>) {
        let Some(frame) = self.sheet.frames.get(frame_index) else { return };
        let hwnd = match self.hwnd {
            Some(hwnd) => hwnd,
            None => match create_window() {
                Ok(hwnd) => {
                    self.hwnd = Some(hwnd);
                    hwnd
                }
                Err(_) => return,
            },
        };

        // Both the scale and the placement come from the monitor resolved at
        // fire time. Asking the window itself is wrong on the first frame: it
        // was just created at (0,0) with size 0x0, so GetDpiForWindow reports
        // whichever monitor owns that point, not the one the buddy will
        // appear on.
        let (work, dpi) = self.stage_monitor.unwrap_or_else(monitor_under_cursor);
        let scale = integer_scale_for(frame.width, self.logical_scale as u32, dpi);
        let sprite = scale_nearest(frame, scale);

        let canvas = match compose(&sprite, message, dpi) {
            Ok(canvas) => canvas,
            Err(_) => return,
        };
        let origin = corner_origin(canvas.width as i32, canvas.height as i32, self.corner, dpi, work);
        let _ = present(hwnd, &canvas, origin, message);
    }

    /// True while anything is still on screen — including the goodbye beat,
    /// which runs after the dispatcher has already collected the outcome.
    pub fn is_visible(&self) -> bool {
        !matches!(self.stage, Stage::Hidden)
    }

    /// Tear the window down immediately, skipping any linger. Used when the
    /// session locks or the app quits: there is nobody to say goodbye to.
    pub fn cancel_now(&mut self) {
        self.hide();
    }
}

/// Lets the coordinator keep a handle on the overlay while the dispatcher owns
/// it as a style.
///
/// The overlay is the one style with a life outside the seam: its linger beat
/// keeps drawing after `poll` has reported the outcome and the dispatcher has
/// dropped it from the live set. So the coordinator pumps `animate` directly
/// and the dispatcher drives the same object through this delegate.
pub struct SharedOverlay(pub std::rc::Rc<std::cell::RefCell<BuddyOverlay>>);

impl NudgeStyle for SharedOverlay {
    fn id(&self) -> &'static str {
        crate::storage::settings::STYLE_BUDDY_OVERLAY
    }

    fn display_name(&self) -> &'static str {
        "Buddy drop-in"
    }

    fn fire(&mut self, context: &NudgeContext) -> Option<NudgeOutcome> {
        self.0.borrow_mut().fire(context)
    }

    fn poll(&mut self, now: f64) -> Option<NudgeOutcome> {
        self.0.borrow_mut().poll(now)
    }

    fn cancel(&mut self) {
        self.0.borrow_mut().cancel()
    }

    fn movement_credited(&mut self) {
        self.0.borrow_mut().movement_credited()
    }
}

impl NudgeStyle for BuddyOverlay {
    fn id(&self) -> &'static str {
        crate::storage::settings::STYLE_BUDDY_OVERLAY
    }

    fn display_name(&self) -> &'static str {
        "Buddy drop-in"
    }

    fn fire(&mut self, context: &NudgeContext) -> Option<NudgeOutcome> {
        CLICKED.store(false, std::sync::atomic::Ordering::Relaxed);
        self.pending = None;
        // Pick the display once, here, from where the mouse is now.
        self.stage_monitor = Some(monitor_under_cursor());
        self.stage = Stage::Dancing { since: context.fired_at, fps: context.dance_fps() };
        self.animate(context.fired_at);
        None
    }

    fn poll(&mut self, now: f64) -> Option<NudgeOutcome> {
        // An auto-credit already moved us to the goodbye beat; hand the
        // dispatcher the outcome it is still waiting for.
        if let Some(outcome) = self.pending.take() {
            return Some(outcome);
        }
        let Stage::Dancing { since, .. } = self.stage else { return None };
        if CLICKED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.begin_linger(now, NudgeOutcome::Acknowledged);
            return Some(NudgeOutcome::Acknowledged);
        }
        if now - since >= MAX_NUDGE_SECONDS {
            self.begin_linger(now, NudgeOutcome::TimedOut);
            return Some(NudgeOutcome::TimedOut);
        }
        None
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.hide();
    }

    fn movement_credited(&mut self) {
        if let Stage::Dancing { .. } = self.stage {
            // The coordinator has already credited; this is the "Saw you step
            // away" beat the user comes back to.
            let now = crate::time::monotonic_seconds();
            self.begin_linger(now, NudgeOutcome::MovementDetected);
            self.pending = Some(NudgeOutcome::MovementDetected);
        }
    }
}

/// The largest whole scale whose result still fits the size the user asked
/// for at this monitor's DPI.
///
/// This is PORTS.md §13's "Windows DPI" risk, resolved. At 150% scaling a
/// naive renderer multiplies by 1.5 and interpolates; here a ×3 buddy at 150%
/// asks for 144px, and the largest whole multiple of a 32px sprite that fits
/// is ×4 (128px). Slightly smaller than requested, perfectly crisp — which is
/// the trade DESIGN.md §2 makes on purpose.
pub fn integer_scale_for(sprite_px: u32, logical_scale: u32, dpi: u32) -> u32 {
    let sprite_px = sprite_px.max(1);
    let logical_scale = logical_scale.max(1);
    let desired = sprite_px * logical_scale * dpi / 96;
    (desired / sprite_px).max(1)
}

/// Which frame is showing, from wall-clock time rather than a counter — a
/// dropped timer tick then skips a frame instead of slowing the whole dance.
pub fn frame_at(elapsed: f64, fps: f64, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let fps = if fps > 0.0 { fps } else { 8.0 };
    let index = (elapsed.max(0.0) * fps) as usize;
    index % count
}

/// Nearest-neighbor integer scale. No averaging, no filtering — a scaled frame
/// contains only colours that were in the source.
pub fn scale_nearest(frame: &Frame, scale: u32) -> Frame {
    let scale = scale.max(1);
    if scale == 1 {
        return frame.clone();
    }
    let (w, h) = (frame.width * scale, frame.height * scale);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let src_row = (y / scale) as usize * frame.width as usize;
        for x in 0..w {
            let src = (src_row + (x / scale) as usize) * 4;
            rgba.extend_from_slice(&frame.rgba[src..src + 4]);
        }
    }
    Frame { width: w, height: h, rgba }
}

/// Where the speech bubble sits inside the composed canvas, so the text pass
/// knows which pixels to make opaque.
struct Composed {
    frame: Frame,
    bubble: Option<RECT>,
    /// The corner radius the pill was filled with. Needed again at repair
    /// time so the alpha fix follows the same shape the fill did.
    bubble_radius: i32,
    font_px: i32,
}

impl std::ops::Deref for Composed {
    type Target = Frame;
    fn deref(&self) -> &Frame {
        &self.frame
    }
}

/// Lay out bubble-above-sprite and rasterize everything except the text, which
/// GDI draws later straight into the DIB.
fn compose(sprite: &Frame, message: Option<&str>, dpi: u32) -> Result<Composed, String> {
    let scaled = |logical: i32| (logical * dpi as i32 / 96).max(1);
    let font_px = scaled(12);

    let (bubble_w, bubble_h) = match message {
        // Width is estimated from the glyph count rather than measured: a
        // measuring pass needs a DC and a selected font before the canvas
        // exists. Segoe UI at this weight averages a little over half its em
        // in width, and the bubble is a rounded pill, so a few pixels of slack
        // reads as padding rather than as a layout bug.
        Some(text) => {
            let w = (text.chars().count() as i32 * font_px * 58 / 100) + scaled(22);
            (w, font_px + scaled(12))
        }
        None => (0, 0),
    };

    let gap = if message.is_some() { scaled(BUBBLE_GAP) } else { 0 };
    let width = (sprite.width as i32).max(bubble_w);
    let height = bubble_h + gap + sprite.height as i32;
    if width <= 0 || height <= 0 {
        return Err("degenerate overlay canvas".to_string());
    }

    let mut canvas = Frame {
        width: width as u32,
        height: height as u32,
        rgba: vec![0; (width * height * 4) as usize],
    };

    let radius = bubble_h / 2;
    let bubble = message.map(|_| {
        // Centred horizontally; integral origin, so the pill never lands on a
        // half pixel.
        let left = (width - bubble_w) / 2;
        let rect = RECT { left, top: 0, right: left + bubble_w, bottom: bubble_h };
        fill_rounded_rect(&mut canvas, rect, radius, bubble_colors(dpi).0);
        rect
    });

    // Sprite centred under the bubble, again on an integral origin.
    let sprite_x = (width - sprite.width as i32) / 2;
    blit(&mut canvas, sprite, sprite_x, bubble_h + gap);

    Ok(Composed { frame: canvas, bubble, bubble_radius: radius, font_px })
}

/// Light and dark bubble colours, following the system theme so the chrome
/// layer reads as native (DESIGN.md §1). Returns (background, text).
fn bubble_colors(_dpi: u32) -> ([u8; 4], COLORREF) {
    if system_prefers_dark() {
        ([44, 44, 48, 235], COLORREF(0x00F7F5F5))
    } else {
        ([255, 255, 255, 235], COLORREF(0x001F1D1D))
    }
}

fn system_prefers_dark() -> bool {
    crate::theme::prefers_dark()
}

fn fill_rounded_rect(canvas: &mut Frame, rect: RECT, radius: i32, color: [u8; 4]) {
    let radius = radius.max(0);
    for y in rect.top.max(0)..rect.bottom.min(canvas.height as i32) {
        for x in rect.left.max(0)..rect.right.min(canvas.width as i32) {
            if !inside_rounded(rect, radius, x, y) {
                continue;
            }
            let at = ((y as u32 * canvas.width + x as u32) * 4) as usize;
            canvas.rgba[at..at + 4].copy_from_slice(&color);
        }
    }
}

fn inside_rounded(rect: RECT, radius: i32, x: i32, y: i32) -> bool {
    let corners = [
        (rect.left + radius, rect.top + radius, x < rect.left + radius && y < rect.top + radius),
        (rect.right - radius - 1, rect.top + radius, x > rect.right - radius - 1 && y < rect.top + radius),
        (rect.left + radius, rect.bottom - radius - 1, x < rect.left + radius && y > rect.bottom - radius - 1),
        (rect.right - radius - 1, rect.bottom - radius - 1, x > rect.right - radius - 1 && y > rect.bottom - radius - 1),
    ];
    for (cx, cy, in_quadrant) in corners {
        if in_quadrant {
            let (dx, dy) = ((x - cx) as f64, (y - cy) as f64);
            return dx * dx + dy * dy <= (radius as f64) * (radius as f64);
        }
    }
    true
}

fn blit(canvas: &mut Frame, src: &Frame, ox: i32, oy: i32) {
    for y in 0..src.height as i32 {
        let dy = oy + y;
        if dy < 0 || dy >= canvas.height as i32 {
            continue;
        }
        for x in 0..src.width as i32 {
            let dx = ox + x;
            if dx < 0 || dx >= canvas.width as i32 {
                continue;
            }
            let s = ((y as u32 * src.width + x as u32) * 4) as usize;
            if src.rgba[s + 3] == 0 {
                continue;
            }
            let d = ((dy as u32 * canvas.width + dx as u32) * 4) as usize;
            canvas.rgba[d..d + 4].copy_from_slice(&src.rgba[s..s + 4]);
        }
    }
}

/// Place the window in the chosen corner of the monitor the mouse is on,
/// inside the work area so it never sits under the taskbar.
/// The work area and DPI of the monitor the mouse is on. Resolved once per
/// nudge; see `BuddyOverlay::stage_monitor`.
fn monitor_under_cursor() -> (RECT, u32) {
    let mut cursor = POINT::default();
    unsafe { let _ = GetCursorPos(&mut cursor); }
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work = if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        info.rcWork
    } else {
        RECT { left: 0, top: 0, right: 1920, bottom: 1080 }
    };
    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    let dpi = if unsafe {
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
    }
    .is_ok()
    {
        dpi_x.max(96)
    } else {
        96
    };
    (work, dpi)
}

fn corner_origin(
    width: i32,
    height: i32,
    corner: OverlayCorner,
    dpi: u32,
    work: RECT,
) -> POINT {
    let margin = (CORNER_MARGIN * dpi as i32 / 96).max(1);
    let (x, y) = match corner {
        OverlayCorner::BottomRight => (work.right - width - margin, work.bottom - height - margin),
        OverlayCorner::BottomLeft => (work.left + margin, work.bottom - height - margin),
        OverlayCorner::TopRight => (work.right - width - margin, work.top + margin),
        OverlayCorner::TopLeft => (work.left + margin, work.top + margin),
    };
    POINT { x, y }
}

/// Push the composed canvas to the layered window, drawing the text as the
/// last step so GDI composites it over an already-opaque bubble.
fn present(
    hwnd: HWND,
    canvas: &Composed,
    origin: POINT,
    message: Option<&str>,
) -> Result<(), String> {
    let dib = Dib::premultiplied_from(&canvas.frame)?;
    let screen_dc = unsafe { windows::Win32::Graphics::Gdi::GetDC(None) };
    let mem_dc: HDC = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    let old = unsafe { SelectObject(mem_dc, dib.bitmap.into()) };

    if let (Some(rect), Some(text)) = (canvas.bubble, message) {
        draw_text_into(mem_dc, rect, canvas.font_px, text);
        // GDI text writes RGB but leaves the alpha byte at whatever it found,
        // so in a premultiplied layered window the glyphs would come out
        // invisible. The bubble is opaque by design, so restoring full alpha
        // across the pill is both correct and the simplest repair. It has to
        // happen after the text lands, and after any GDI call that touched
        // these pixels — hence here rather than in `compose`.
        let radius = canvas.bubble_radius;
        dib.force_opaque_where(rect, |x, y| inside_rounded(rect, radius, x, y));
    }

    let size = SIZE { cx: canvas.width as i32, cy: canvas.height as i32 };
    let mut src = POINT { x: 0, y: 0 };
    let mut dst = origin;
    let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: windows::Win32::Graphics::Gdi::AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: windows::Win32::Graphics::Gdi::AC_SRC_ALPHA as u8,
    };
    let result = unsafe {
        UpdateLayeredWindow(
            hwnd,
            Some(screen_dc),
            Some(&mut dst),
            Some(&size),
            Some(mem_dc),
            Some(&mut src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
    };

    unsafe {
        SelectObject(mem_dc, old);
        let _ = DeleteDC(mem_dc);
        windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            dst.x,
            dst.y,
            size.cx,
            size.cy,
            SWP_NOACTIVATE,
        );
    }
    result.map_err(|e| format!("UpdateLayeredWindow: {e}"))
}

/// Segoe UI is the Windows chrome font, the counterpart of the macOS shell's
/// SF Pro: the bubble belongs to DESIGN.md's "invisible-native" layer, not to
/// the pixel-art layer, so it must not be drawn in a pixel font.
fn draw_text_into(dc: HDC, rect: RECT, font_px: i32, text: &str) {
    let (_, text_color) = bubble_colors(96);
    // Negative height asks for a cell of that many pixels rather than that
    // many points, which is what keeps the glyphs the size the layout budgeted.
    let face = wide("Segoe UI");
    let font = unsafe {
        CreateFontW(
            -font_px, 0, 0, 0, FW_SEMIBOLD.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            // Antialiased rather than ClearType: the bubble is composited into
            // a layered window, and subpixel-rendered glyphs carry colour
            // fringes that assume a known opaque background.
            ANTIALIASED_QUALITY,
            FF_DONTCARE.0 as u32,
            PCWSTR(face.as_ptr()),
        )
    };
    let old = unsafe { SelectObject(dc, font.into()) };
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, text_color);
    }
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut r = rect;
    if !wide.is_empty() {
        unsafe { DrawTextW(dc, &mut wide, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE) };
    }
    unsafe {
        SelectObject(dc, old);
        let _ = DeleteObject(font.into());
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create_window() -> Result<HWND, String> {
    // Both stay alive across the CreateWindowEx call below; Win32 copies the
    // class name at registration but reads the pointers during the call.
    let class_name = wide("ScootBuddyOverlay");
    let title = wide("Scoot");
    unsafe {
        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map_err(|e| format!("GetModuleHandle: {e}"))?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(overlay_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(None, IDC_ARROW)
                .unwrap_or_default(),
            ..Default::default()
        };
        // Re-registering an existing class fails harmlessly; the buddy is
        // created and destroyed once per nudge.
        RegisterClassW(&class);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0, 0, 0, 0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))?;
        Ok(hwnd)
    }
}

extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP;
    if msg == WM_LBUTTONUP {
        CLICKED.store(true, std::sync::atomic::Ordering::Relaxed);
        return windows::Win32::Foundation::LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// Referenced so the unused-import lint stays quiet about the alpha constant we
// document but do not call directly.
#[allow(dead_code)]
const _: u32 = LWA_ALPHA.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32) -> Frame {
        Frame { width: w, height: h, rgba: vec![7, 8, 9, 255].repeat((w * h) as usize) }
    }

    #[test]
    fn integer_scale_never_goes_fractional_at_awkward_dpi() {
        // 100%: a x3 buddy is exactly x3.
        assert_eq!(integer_scale_for(32, 3, 96), 3);
        // 125%: 120px wanted, x4 would overshoot at 128 — stay at x3.
        assert_eq!(integer_scale_for(32, 3, 120), 3);
        // 150%: 144px wanted, x4 fits at 128. The PORTS.md §13 case.
        assert_eq!(integer_scale_for(32, 3, 144), 4);
        // 200%: exactly x6.
        assert_eq!(integer_scale_for(32, 3, 192), 6);
    }

    #[test]
    fn scale_is_always_at_least_one() {
        assert_eq!(integer_scale_for(32, 0, 96), 1);
        assert_eq!(integer_scale_for(32, 1, 48), 1, "a sub-96 dpi must not scale to zero");
        assert_eq!(integer_scale_for(0, 3, 96), 3);
    }

    #[test]
    fn nearest_neighbour_invents_no_colours() {
        // The constitutional test: an interpolating scaler produces
        // intermediate values, and this asserts none appear.
        let mut src = frame(2, 2);
        src.rgba[0..4].copy_from_slice(&[255, 0, 0, 255]);
        src.rgba[4..8].copy_from_slice(&[0, 0, 255, 255]);
        let out = scale_nearest(&src, 4);
        assert_eq!(out.width, 8);
        assert_eq!(out.height, 8);
        let source_colors: std::collections::HashSet<[u8; 4]> =
            src.rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
        for px in out.rgba.chunks_exact(4) {
            let c = [px[0], px[1], px[2], px[3]];
            assert!(source_colors.contains(&c), "scaler invented {c:?}");
        }
    }

    #[test]
    fn scaling_by_one_is_identity() {
        let src = frame(3, 5);
        assert_eq!(scale_nearest(&src, 1), src);
    }

    #[test]
    fn scaled_output_has_the_exact_pixel_count() {
        let src = frame(40, 32);
        let out = scale_nearest(&src, 3);
        assert_eq!(out.width, 120);
        assert_eq!(out.height, 96);
        assert_eq!(out.rgba.len(), (120 * 96 * 4) as usize);
    }

    #[test]
    fn frames_advance_at_the_requested_rate_and_wrap() {
        assert_eq!(frame_at(0.0, 8.0, 4), 0);
        assert_eq!(frame_at(0.125, 8.0, 4), 1);
        assert_eq!(frame_at(0.5, 8.0, 4), 0, "wraps after four frames");
        assert_eq!(frame_at(1.0 / 12.0, 12.0, 4), 1);
    }

    #[test]
    fn frame_selection_survives_nonsense_inputs() {
        assert_eq!(frame_at(-5.0, 8.0, 4), 0);
        assert_eq!(frame_at(1.0, 0.0, 4), 0, "a zero fps falls back, never divides by zero");
        assert_eq!(frame_at(1.0, 8.0, 0), 0, "an empty sheet must not index");
    }

    #[test]
    fn a_rounded_rect_clips_its_corners_but_keeps_its_middle() {
        let mut canvas = Frame { width: 20, height: 10, rgba: vec![0; 20 * 10 * 4] };
        let rect = RECT { left: 0, top: 0, right: 20, bottom: 10 };
        fill_rounded_rect(&mut canvas, rect, 5, [255, 255, 255, 255]);
        let at = |x: u32, y: u32| canvas.rgba[((y * 20 + x) * 4 + 3) as usize];
        assert_eq!(at(0, 0), 0, "top-left corner should be cut away");
        assert_eq!(at(19, 0), 0, "top-right corner should be cut away");
        assert_eq!(at(10, 5), 255, "the middle must be filled");
        assert_eq!(at(10, 0), 255, "the top edge between the corners is filled");
    }

    #[test]
    fn blit_skips_transparent_source_pixels() {
        let mut canvas = Frame { width: 4, height: 4, rgba: vec![9; 4 * 4 * 4] };
        let mut src = frame(2, 2);
        src.rgba[0..4].copy_from_slice(&[0, 0, 0, 0]);
        blit(&mut canvas, &src, 0, 0);
        assert_eq!(&canvas.rgba[0..4], &[9, 9, 9, 9], "transparent pixel must not overwrite");
        assert_eq!(&canvas.rgba[4..8], &[7, 8, 9, 255], "opaque pixel must land");
    }

    #[test]
    fn blit_clips_rather_than_panicking_off_canvas() {
        let mut canvas = Frame { width: 4, height: 4, rgba: vec![0; 4 * 4 * 4] };
        let src = frame(8, 8);
        blit(&mut canvas, &src, -2, -2);
        blit(&mut canvas, &src, 3, 3);
    }

    #[test]
    fn an_auto_credit_surfaces_an_outcome_on_the_next_poll() {
        // The bug this guards: movement_credited() moved the stage to
        // Lingering, but poll() only reported from Dancing — so the style was
        // never removed from the dispatcher's live set. The 66ms animation
        // timer then ran until the next nudge, and the window never got its
        // nudge_outcome line.
        let sheet = SpriteSheet::load(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../Sources/Scoot/Resources/Sprites/buddy-classic.png")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../Sources/Scoot/Resources/Sprites/buddy-classic.json")),
        )
        .expect("classic sheet");
        let mut overlay = BuddyOverlay::new(sheet, OverlayCorner::BottomRight, 3);
        overlay.stage = Stage::Dancing { since: 0.0, fps: 8.0 };

        overlay.movement_credited();
        assert_eq!(
            overlay.poll(1.0),
            Some(NudgeOutcome::MovementDetected),
            "the credit must reach the dispatcher"
        );
        assert_eq!(overlay.poll(2.0), None, "and only once");
    }

    #[test]
    fn a_cancel_drops_any_owed_outcome() {
        let sheet = SpriteSheet::load(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../Sources/Scoot/Resources/Sprites/buddy-classic.png")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../Sources/Scoot/Resources/Sprites/buddy-classic.json")),
        )
        .expect("classic sheet");
        let mut overlay = BuddyOverlay::new(sheet, OverlayCorner::BottomRight, 3);
        overlay.stage = Stage::Dancing { since: 0.0, fps: 8.0 };
        overlay.movement_credited();
        overlay.cancel();
        assert_eq!(overlay.poll(1.0), None, "a cancelled nudge owes nothing");
    }

    #[test]
    fn the_pill_corners_stay_transparent() {
        // Same bug as dib's mask test, asserted through the predicate the
        // overlay actually uses: the wedges outside the rounded corners must
        // not be considered part of the pill.
        let rect = RECT { left: 0, top: 0, right: 40, bottom: 20 };
        let radius = 10;
        assert!(!inside_rounded(rect, radius, 0, 0), "top-left wedge");
        assert!(!inside_rounded(rect, radius, 39, 0), "top-right wedge");
        assert!(!inside_rounded(rect, radius, 0, 19), "bottom-left wedge");
        assert!(!inside_rounded(rect, radius, 39, 19), "bottom-right wedge");
        assert!(inside_rounded(rect, radius, 20, 10), "the middle is the pill");
        assert!(inside_rounded(rect, radius, 20, 0), "the top edge is the pill");
    }

    #[test]
    fn the_dance_ends_at_the_house_limit() {
        assert_eq!(MAX_NUDGE_SECONDS, 12.0, "PRODUCT.md §3 and the house rules");
    }

    #[test]
    fn linger_beats_match_the_macos_timings() {
        assert_eq!(LINGER_ACKNOWLEDGED, 1.4);
        assert_eq!(LINGER_MOVEMENT, 2.2);
        assert_eq!(LINGER_TIMEOUT, 1.2);
    }

    #[test]
    fn copy_is_the_approved_wording() {
        // PRODUCT.md §8 — warm, short, and none of the banned words.
        assert_eq!(MSG_DANCING, "Time to scoot.");
        assert_eq!(MSG_ACKNOWLEDGED, "Nice scoot.");
        assert_eq!(MSG_AUTO_CREDIT, "Saw you step away. +1 scoot.");
        for banned in ["streak freeze", "don't lose", "you failed", "last chance"] {
            for copy in [MSG_DANCING, MSG_ACKNOWLEDGED, MSG_AUTO_CREDIT] {
                assert!(!copy.to_lowercase().contains(banned), "{copy} contains {banned}");
            }
        }
    }
}
