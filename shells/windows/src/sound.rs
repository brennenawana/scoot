//! The chime nudge style's voice: the committed `nudge-chime.wav`
//! (PORTS.md §7).
//!
//! The file rides inside the exe rather than beside it. PORTS.md §7 ships this
//! shell as a single static exe, and a nudge that goes quiet because somebody
//! moved a folder is a failure the user cannot see, let alone diagnose.

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_FLAGS, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

/// RIFF/WAVE, PCM, mono, 44100 Hz, 16-bit — synthesized by the stdlib Python
/// pipeline and committed (PHILOSOPHY.md §3). Byte-for-byte the file macOS
/// plays, which is the whole point of sharing content across shells.
const CHIME_WAV: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../Sources/Scoot/Resources/Sounds/nudge-chime.wav"
));

/// Start the chime. Returns immediately; the sound plays on winmm's thread.
#[allow(dead_code)]
pub fn play_chime() {
    // SAFETY: SND_MEMORY says the pointer is a buffer, and SND_ASYNC says
    // playback outlives this call — so the buffer has to outlive it too.
    // `include_bytes!` puts these bytes in the exe's read-only data with a
    // `&'static` lifetime, so there is nothing here that can be freed, moved,
    // or dropped out from under winmm. The `*const u16` is only PCWSTR's
    // shape; under SND_MEMORY the pointer is opaque and never read as text.
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(CHIME_WAV.as_ptr().cast::<u16>()),
            None,
            // SND_ASYNC: this thread pumps the tray and the overlay message
            // loop. Blocking it for the length of the chime would freeze the
            // buddy mid-nudge.
            // SND_NODEFAULT: when the sound cannot play, the failure is
            // silence. A break reminder that instead fires the Windows alert
            // ding is precisely the wrong kind of surprise the product exists
            // to avoid (PRODUCT.md §4).
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

/// Cut playback short. The coordinator calls this when the session locks
/// mid-nudge: the user has walked away, and a chime still ringing at a locked
/// screen is noise aimed at nobody.
#[allow(dead_code)]
pub fn stop_chime() {
    // A null sound with no flags is the documented way to stop an async
    // waveform this process started. `SND_PURGE` reads like the right flag and
    // happens to work, but MSDN lists it as "Not supported." for PlaySound —
    // it belongs to sndPlaySound — so relying on it means relying on winmm
    // continuing to ignore an unsupported flag. The stop actually comes from
    // the null pszSound either way.
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(offset: usize) -> u16 {
        u16::from_le_bytes([CHIME_WAV[offset], CHIME_WAV[offset + 1]])
    }

    fn u32_at(offset: usize) -> u32 {
        u32::from_le_bytes(CHIME_WAV[offset..offset + 4].try_into().unwrap())
    }

    /// Offset of a chunk's payload and its declared size. Walks rather than
    /// assuming offset 36, so an extra `LIST` or `fact` chunk from a future
    /// generator run does not make these tests lie.
    fn chunk(id: &[u8; 4]) -> Option<(usize, usize)> {
        let mut at = 12; // past "RIFF", its size field, and "WAVE"
        while at + 8 <= CHIME_WAV.len() {
            let size = u32_at(at + 4) as usize;
            if &CHIME_WAV[at..at + 4] == id {
                return Some((at + 8, size));
            }
            at += 8 + size + (size & 1); // chunks are word-aligned
        }
        None
    }

    #[test]
    fn the_embedded_bytes_are_a_riff_wave_file() {
        assert_eq!(&CHIME_WAV[0..4], b"RIFF");
        assert_eq!(&CHIME_WAV[8..12], b"WAVE");
        assert_eq!(
            u32_at(4) as usize,
            CHIME_WAV.len() - 8,
            "the RIFF size must cover everything after it"
        );
    }

    #[test]
    fn the_format_is_the_one_the_chime_was_authored_in() {
        let (at, size) = chunk(b"fmt ").expect("fmt chunk");
        assert_eq!(size, 16, "plain PCM fmt chunk");
        assert_eq!(u16_at(at), 1, "PCM, uncompressed");
        assert_eq!(u16_at(at + 2), 1, "mono");
        assert_eq!(u32_at(at + 4), 44_100, "sample rate");
        assert_eq!(u32_at(at + 8), 88_200, "byte rate = 44100 x 1ch x 2 bytes");
        assert_eq!(u16_at(at + 12), 2, "block align");
        assert_eq!(u16_at(at + 14), 16, "bit depth");
    }

    #[test]
    fn the_data_chunk_runs_to_the_end_of_the_file() {
        let (at, size) = chunk(b"data").expect("data chunk");
        assert_eq!(at + size, CHIME_WAV.len(), "declared size must match what shipped");
        // A chime is a chime. Empty means the generator wrote a header and no
        // samples; minutes long means it is not a chime any more.
        let seconds = size as f64 / 88_200.0;
        assert!((0.1..3.0).contains(&seconds), "implausible chime length: {seconds}s");
    }
}
