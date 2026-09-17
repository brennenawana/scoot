//! Turning committed pixel art into things Windows can draw.
//!
//! The constitutional rule (DESIGN.md §2, PORTS.md §9) governs this whole
//! module: **integer scale factors only, nearest-neighbor only, integral
//! origins.** One blurry sprite breaks the spell, so there is no code path
//! here that can produce a fractional scale or an interpolated pixel.

pub mod dib;
pub mod sprite;
