//! The bridge from decoded pixels to things GDI will draw: a tray `HICON` and
//! a premultiplied DIB section for `UpdateLayeredWindow`.
//!
//! Windows wants two *different* alpha conventions and mixing them up is the
//! classic layered-window bug, so they are separated here by name:
//!
//! * **Icons take straight alpha.** `CreateIconIndirect` composites using the
//!   raw channel values.
//! * **`UpdateLayeredWindow` takes premultiplied alpha.** Feed it straight
//!   alpha and semi-transparent pixels come out too bright, with a pale halo
//!   around every antialiased edge. Scoot's art is hard-edged so the halo
//!   would be subtle — which is worse, because it survives review.
//!
//! Both paths are top-down DIBs (negative `biHeight`). Bottom-up is the GDI
//! default and would render the buddy upside down.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO};

use super::sprite::Frame;

/// A 32-bit top-down DIB section that owns its bitmap and its pixel memory.
pub struct Dib {
    pub bitmap: HBITMAP,
    bits: *mut u8,
    pub width: i32,
    pub height: i32,
}

impl Dib {
    /// Allocate a DIB and fill it with `frame`, premultiplying as it copies.
    /// Intended for `UpdateLayeredWindow` — see the module note on why this
    /// path premultiplies and the icon path does not.
    pub fn premultiplied_from(frame: &Frame) -> Result<Dib, String> {
        let dib = Dib::new(frame.width as i32, frame.height as i32)?;
        let pixels = (frame.width as usize) * (frame.height as usize);
        if frame.rgba.len() < pixels * 4 {
            return Err(format!(
                "frame {}x{} carries {} bytes, needs {}",
                frame.width,
                frame.height,
                frame.rgba.len(),
                pixels * 4
            ));
        }
        // SAFETY: `bits` points at exactly pixels*4 bytes owned by the DIB
        // section we just created, and nothing else aliases it.
        let out = unsafe { std::slice::from_raw_parts_mut(dib.bits, pixels * 4) };
        for (src, dst) in frame.rgba.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
            let (r, g, b, a) = (src[0] as u32, src[1] as u32, src[2] as u32, src[3] as u32);
            // Rounded rather than truncated: truncation darkens every
            // semi-transparent pixel by up to one level, which reads as a
            // grubby outline once the sprite is scaled 4x.
            let mul = |c: u32| ((c * a + 127) / 255) as u8;
            dst[0] = mul(b);
            dst[1] = mul(g);
            dst[2] = mul(r);
            dst[3] = a as u8;
        }
        Ok(dib)
    }

    /// Allocate a DIB and fill it with straight (non-premultiplied) BGRA.
    /// This is what icons want.
    fn straight_from(frame: &Frame) -> Result<Dib, String> {
        let dib = Dib::new(frame.width as i32, frame.height as i32)?;
        let pixels = (frame.width as usize) * (frame.height as usize);
        if frame.rgba.len() < pixels * 4 {
            return Err("frame buffer too small".to_string());
        }
        let out = unsafe { std::slice::from_raw_parts_mut(dib.bits, pixels * 4) };
        for (src, dst) in frame.rgba.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
        Ok(dib)
    }

    /// Force full alpha across a rectangle.
    ///
    /// GDI's text and shape calls write the colour channels but leave the
    /// alpha byte untouched, which in a premultiplied layered window renders
    /// as nothing at all. Any region that GDI drew into and that is meant to
    /// be opaque has to be repaired afterwards; the speech bubble is exactly
    /// that region. Clipped to the bitmap so a mis-sized rect cannot walk off
    /// the buffer.
    pub fn force_opaque(&self, rect: windows::Win32::Foundation::RECT) {
        let left = rect.left.clamp(0, self.width);
        let right = rect.right.clamp(0, self.width);
        let top = rect.top.clamp(0, self.height);
        let bottom = rect.bottom.clamp(0, self.height);
        if left >= right || top >= bottom {
            return;
        }
        let stride = self.width as usize * 4;
        // SAFETY: the DIB owns height*stride bytes and the loop bounds are
        // clamped to its dimensions above.
        let bits = unsafe { std::slice::from_raw_parts_mut(self.bits, self.height as usize * stride) };
        for y in top..bottom {
            let row = y as usize * stride;
            for x in left..right {
                bits[row + x as usize * 4 + 3] = 255;
            }
        }
    }

    fn new(width: i32, height: i32) -> Result<Dib, String> {
        if width <= 0 || height <= 0 {
            return Err(format!("degenerate bitmap {width}x{height}"));
        }
        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative: top-down, matching the row order of decoded PNGs.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO { bmiHeader: header, ..Default::default() };

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let screen: HDC = unsafe { GetDC(None) };
        let bitmap = unsafe {
            CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        };
        if !screen.is_invalid() {
            unsafe { ReleaseDC(None, screen) };
        }
        let bitmap = bitmap.map_err(|e| format!("CreateDIBSection failed: {e}"))?;
        if bits.is_null() {
            unsafe { let _ = DeleteObject(bitmap.into()); }
            return Err("CreateDIBSection returned no pixel pointer".to_string());
        }
        Ok(Dib { bitmap, bits: bits as *mut u8, width, height })
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe { let _ = DeleteObject(self.bitmap.into()); }
    }
}

/// An owned tray icon. `Shell_NotifyIcon` copies what it needs, but the icon
/// must stay alive as long as it is the icon currently shown, and it must be
/// destroyed afterwards — leaking one per animation frame would burn a GDI
/// handle every 60ms during a bounce.
pub struct Icon(pub HICON);

impl Icon {
    pub fn from_frame(frame: &Frame) -> Result<Icon, String> {
        let color = Dib::straight_from(frame)?;

        // A 32-bit colour bitmap carries its own alpha, so the mask is
        // vestigial — but ICONINFO still requires one, and it must be the
        // same size or the icon silently renders blank on some shells.
        let mask_bits = vec![0u8; mask_stride(frame.width) * frame.height as usize];
        let mask: HBITMAP = unsafe {
            CreateBitmap(
                frame.width as i32,
                frame.height as i32,
                1,
                1,
                Some(mask_bits.as_ptr() as *const _),
            )
        };
        if mask.is_invalid() {
            return Err("CreateBitmap failed for the icon mask".to_string());
        }

        let mut info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color.bitmap,
        };
        let handle = unsafe { CreateIconIndirect(&mut info) };
        // CreateIconIndirect copies both bitmaps, so ours are ours to clean up
        // whether or not it succeeded. `color` drops itself.
        unsafe { let _ = DeleteObject(mask.into()); }
        let handle = handle.map_err(|e| format!("CreateIconIndirect failed: {e}"))?;
        Ok(Icon(handle))
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        unsafe { let _ = DestroyIcon(self.0); }
    }
}

/// 1-bpp scanlines are padded to a 2-byte boundary (`WORD`), not 4.
fn mask_stride(width: u32) -> usize {
    (((width as usize) + 15) / 16) * 2
}

// Kept so the overlay can hand its window handle in without another import.
#[allow(dead_code)]
pub type WindowHandle = HWND;

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, px: [u8; 4]) -> Frame {
        Frame { width, height, rgba: px.repeat((width * height) as usize) }
    }

    #[test]
    fn mask_scanlines_are_word_aligned() {
        assert_eq!(mask_stride(1), 2);
        assert_eq!(mask_stride(16), 2);
        assert_eq!(mask_stride(17), 4);
        assert_eq!(mask_stride(32), 4);
        assert_eq!(mask_stride(40), 6);
    }

    #[test]
    fn premultiply_scales_colour_by_alpha_and_swaps_to_bgra() {
        // Half-transparent pure red.
        let frame = solid(2, 2, [255, 0, 0, 128]);
        let dib = Dib::premultiplied_from(&frame).expect("dib");
        let out = unsafe { std::slice::from_raw_parts(dib.bits, 2 * 2 * 4) };
        // BGRA order, red premultiplied by 128/255 and rounded.
        assert_eq!(out[0], 0, "blue");
        assert_eq!(out[1], 0, "green");
        assert_eq!(out[2], 128, "red premultiplied");
        assert_eq!(out[3], 128, "alpha preserved");
    }

    #[test]
    fn a_fully_opaque_pixel_survives_premultiplication_unchanged() {
        let frame = solid(1, 1, [10, 20, 30, 255]);
        let dib = Dib::premultiplied_from(&frame).expect("dib");
        let out = unsafe { std::slice::from_raw_parts(dib.bits, 4) };
        assert_eq!(out, &[30, 20, 10, 255], "opaque pixels must not shift");
    }

    #[test]
    fn a_fully_transparent_pixel_premultiplies_to_zero() {
        let frame = solid(1, 1, [255, 255, 255, 0]);
        let dib = Dib::premultiplied_from(&frame).expect("dib");
        let out = unsafe { std::slice::from_raw_parts(dib.bits, 4) };
        assert_eq!(out, &[0, 0, 0, 0], "transparent pixels must not leak colour");
    }

    #[test]
    fn straight_conversion_leaves_colour_alone() {
        let frame = solid(1, 1, [255, 0, 0, 128]);
        let dib = Dib::straight_from(&frame).expect("dib");
        let out = unsafe { std::slice::from_raw_parts(dib.bits, 4) };
        assert_eq!(out, &[0, 0, 255, 128], "icons take straight alpha");
    }

    #[test]
    fn degenerate_sizes_are_refused_rather_than_allocating() {
        assert!(Dib::new(0, 16).is_err());
        assert!(Dib::new(16, 0).is_err());
        assert!(Dib::new(-4, 16).is_err());
    }

    #[test]
    fn a_short_pixel_buffer_is_refused_not_read_past() {
        let bad = Frame { width: 4, height: 4, rgba: vec![0; 8] };
        assert!(Dib::premultiplied_from(&bad).is_err());
    }

    #[test]
    fn an_icon_can_be_built_at_every_tray_size_we_ship() {
        for size in [16u32, 20, 24, 32, 40, 48] {
            let frame = solid(size, size, [200, 100, 50, 255]);
            let icon = Icon::from_frame(&frame).unwrap_or_else(|e| panic!("{size}px: {e}"));
            assert!(!icon.0.is_invalid(), "{size}px produced an invalid HICON");
            // Drop runs DestroyIcon; a leak here would show up as GDI handle
            // growth in the long-run test rather than a failure.
        }
    }
}
