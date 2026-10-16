//! Cut the sound of an MP4/M4A from a given time, with no other program (v0.835.0).
//!
//! Jumping in a video used to need `ffmpeg` to re-make the whole rest of the sound before anything could be
//! heard (and nothing was heard at all if ffmpeg was missing or slow). Almost every video has AAC sound, which
//! can be copied as it is: each sample gets a 7-byte ADTS header and the result is a `.aac` file the Mac's
//! `afplay` plays. Starting from the right sample takes milliseconds, whatever the length.

use mp4::{Mp4Reader, TrackType};
use std::io::Write;
use std::path::Path;

/// 7-byte ADTS header for one AAC frame of `payload` bytes.
pub fn adts_header(object_type: u8, freq_index: u8, channels: u8, payload: usize) -> [u8; 7] {
    let len = payload + 7;
    let profile = object_type.saturating_sub(1) & 0x3; // ADTS stores object type - 1
    [
        0xFF,
        0xF1, // MPEG-4, layer 0, no CRC
        (profile << 6) | ((freq_index & 0xF) << 2) | ((channels >> 2) & 1),
        ((channels & 3) << 6) | ((len >> 11) & 3) as u8,
        ((len >> 3) & 0xFF) as u8,
        (((len & 7) << 5) as u8) | 0x1F,
        0xFC,
    ]
}

/// Writes the AAC sound of `input` from `start_ms` to the end into `out` (ADTS `.aac`).
/// `Err` when there is no AAC-LC/Main sound track (the caller then falls back to ffmpeg).
pub fn cut_aac_adts(input: &Path, start_ms: u64, out: &Path) -> Result<(), String> {
    let f = std::fs::File::open(input).map_err(|e| e.to_string())?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    let mut mp4 = Mp4Reader::read_header(std::io::BufReader::new(f), size).map_err(|e| e.to_string())?;
    let mut ids: Vec<u32> = mp4.tracks().iter().filter(|(_, t)| matches!(t.track_type(), Ok(TrackType::Audio))).map(|(id, _)| *id).collect();
    ids.sort();
    let Some(&id) = ids.first() else { return Err("no sound track".into()) };
    let (obj, freq, chan, first, count) = {
        let t = &mp4.tracks()[&id];
        let obj = t.audio_profile().map_err(|e| e.to_string())? as u8;
        if !(1..=2).contains(&obj) {
            return Err("sound is not plain AAC".into());
        }
        let freq = t.sample_freq_index().map_err(|e| e.to_string())? as u8;
        let chan = t.channel_config().map_err(|e| e.to_string())? as u8;
        let ts = t.timescale().max(1) as u64;
        let target = start_ms * ts / 1000;
        // first sample whose start time is at or before the target and ends after it
        let (mut idx, mut time) = (0u32, 0u64);
        'find: for e in &t.trak.mdia.minf.stbl.stts.entries {
            let delta = (e.sample_delta as u64).max(1);
            let span = delta * e.sample_count as u64;
            if time + span <= target {
                time += span;
                idx += e.sample_count;
            } else {
                let skip = (target - time) / delta;
                idx += skip as u32;
                break 'find;
            }
        }
        (obj, freq, chan, (idx + 1).min(t.sample_count().max(1)), t.sample_count())
    };
    let mut w = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| e.to_string())?);
    let mut wrote = 0u32;
    for n in first..=count {
        let Some(s) = mp4.read_sample(id, n).map_err(|e| e.to_string())? else { break };
        w.write_all(&adts_header(obj, freq, chan, s.bytes.len())).map_err(|e| e.to_string())?;
        w.write_all(&s.bytes).map_err(|e| e.to_string())?;
        wrote += 1;
    }
    w.flush().map_err(|e| e.to_string())?;
    if wrote == 0 {
        return Err("nothing to play after that time".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adts_header_has_sync_and_length() {
        let h = adts_header(2, 4, 2, 100); // AAC-LC, 44100 Hz, stereo
        assert_eq!(h[0], 0xFF);
        assert_eq!(h[1] & 0xF0, 0xF0);
        let len = (((h[3] & 3) as usize) << 11) | ((h[4] as usize) << 3) | ((h[5] >> 5) as usize);
        assert_eq!(len, 107);
        assert_eq!((h[2] >> 2) & 0xF, 4);
        assert_eq!(((h[2] & 1) << 2) | (h[3] >> 6), 2);
        assert_eq!(h[2] >> 6, 1, "LC is profile 1 in ADTS");
    }
}
