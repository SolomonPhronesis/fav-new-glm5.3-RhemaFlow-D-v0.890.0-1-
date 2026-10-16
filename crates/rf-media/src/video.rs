//! H.264 in an MP4 -> RGBA frames in display order (spec S3.1, S3.7, S3.8, S3.10).

use crate::Rgba;
use mp4::{MediaType, Mp4Reader, TrackType};
use openh264::decoder::Decoder;
use rf_service::media::CONVERT_VIDEO;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::Path;

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_count: u32,
    pub fps: f32,
}

pub struct Frame {
    pub pts_ms: u64,
    pub image: Rgba,
}

pub struct VideoReader {
    mp4: Mp4Reader<Box<dyn ReadSeek>>,
    track_id: u32,
    nal_len: usize,
    header: Vec<u8>,
    /// display time of each output frame, in order (milliseconds, first = 0)
    pts: Vec<u64>,
    info: VideoInfo,
    decoder: Decoder,
    next_sample: u32,
    /// how many frames have been handed out, in display order
    out_index: usize,
    /// for the n-th frame the decoder makes (file order): its place in display order
    rank_of_decoded: Vec<usize>,
    /// how many pictures the decoder has made so far
    decoded_count: usize,
    /// decoded but not yet due, by display place (holds back frames that came out early)
    held: HashMap<usize, Rgba>,
    header_sent: bool,
    /// samples (1-based, file order) a decode can start from cleanly (keyframes); empty = every sample
    sync: Vec<u32>,
}

const START: [u8; 4] = [0, 0, 0, 1];

fn new_decoder() -> Result<Decoder, String> {
    Decoder::new(openh264::OpenH264API::from_source()).map_err(|e| format!("The video decoder could not start ({e})."))
}

/// When a file will not open, leave a short report in `~/.rhemaflow/last-video-problem.txt` that can be sent for a fix.
fn write_report(path: &Path, why: &str) {
    let Some(home) = std::env::var_os("HOME") else { return };
    let dir = std::path::PathBuf::from(home).join(".rhemaflow");
    let _ = std::fs::create_dir_all(&dir);
    let outline = std::fs::File::open(path).map(|mut f| crate::mp4fix::describe(&mut f)).unwrap_or_else(|e| format!("could not open: {e}"));
    let _ = std::fs::write(dir.join("last-video-problem.txt"), format!("file: {}\nproblem: {why}\n\n{outline}", path.display()));
}

impl VideoReader {
    pub fn open(path: &Path) -> Result<VideoReader, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("Cannot find or open this video ({e})."))?;
        let size = f.metadata().map(|m| m.len()).map_err(|e| e.to_string())?;
        match Self::from_reader(Box::new(std::io::BufReader::new(f)), size) {
            Ok(v) => Ok(v),
            Err(first) => {
                // the picture may be fine while the sound track's header is not: ignore non-video tracks and retry
                let patches = std::fs::File::open(path).ok().and_then(|mut f| crate::mp4fix::non_video_track_patches(&mut f).ok()).unwrap_or_default();
                if patches.is_empty() {
                    write_report(path, &first);
                    return Err(first);
                }
                let f = std::fs::File::open(path).map_err(|_| first.clone())?;
                Self::from_reader(Box::new(std::io::BufReader::new(crate::mp4fix::PatchedReader::new(f, patches))), size).map_err(|_| {
                    write_report(path, &first);
                    first
                })
            }
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<VideoReader, String> {
        let size = bytes.len() as u64;
        match Self::from_reader(Box::new(std::io::Cursor::new(bytes.clone())), size) {
            Ok(v) => Ok(v),
            Err(first) => {
                let patches = crate::mp4fix::non_video_track_patches(&mut std::io::Cursor::new(&bytes)).unwrap_or_default();
                if patches.is_empty() {
                    return Err(first);
                }
                let mut fixed = bytes;
                crate::mp4fix::apply_to_bytes(&mut fixed, &patches);
                Self::from_reader(Box::new(std::io::Cursor::new(fixed)), size).map_err(|_| first)
            }
        }
    }

    fn from_reader(reader: Box<dyn ReadSeek>, size: u64) -> Result<VideoReader, String> {
        let not_mp4 = |e: String| format!("This video could not be opened ({e}). It may be damaged or cut short, or not an MP4 file. {CONVERT_VIDEO}");
        // a damaged file must never take the app down, even if the parser has a bug
        let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Mp4Reader::read_header(reader, size)));
        let mp4 = match parsed {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => return Err(not_mp4(e.to_string())),
            Err(_) => return Err(not_mp4("unreadable file".into())),
        };
        // S3.8: pick the first VIDEO track; the sound track is simply not touched
        let mut video_ids: Vec<u32> = mp4.tracks().iter().filter(|(_, t)| matches!(t.track_type(), Ok(TrackType::Video))).map(|(id, _)| *id).collect();
        video_ids.sort();
        let Some(&track_id) = video_ids.first() else {
            return Err("This file has no video picture in it. (Is it a sound-only file?)".to_string());
        };
        let track = &mp4.tracks()[&track_id];
        match track.media_type() {
            Ok(MediaType::H264) => {}
            Ok(other) => {
                let name = match other {
                    MediaType::H265 => "H.265 (HEVC)".to_string(),
                    MediaType::VP9 => "VP9".to_string(),
                    o => format!("{o}"),
                };
                return Err(format!("This video uses {name}, which RhemaFlow cannot play. {CONVERT_VIDEO}"));
            }
            Err(_) => return Err(format!("RhemaFlow does not recognise this video's format (phone videos are often HEVC, which it cannot play). {CONVERT_VIDEO}")),
        }
        let stbl = &track.trak.mdia.minf.stbl;
        let avc1 = stbl.stsd.avc1.as_ref().ok_or_else(|| format!("RhemaFlow does not recognise this video's format. {CONVERT_VIDEO}"))?;
        let nal_len = avc1.avcc.length_size_minus_one as usize + 1;
        let sps = track.sequence_parameter_set().map_err(|_| "This video is missing its H.264 set-up information.".to_string())?;
        let pps = track.picture_parameter_set().map_err(|_| "This video is missing its H.264 set-up information.".to_string())?;
        let mut header = Vec::new();
        header.extend_from_slice(&START);
        header.extend_from_slice(sps);
        header.extend_from_slice(&START);
        header.extend_from_slice(pps);

        let count = track.sample_count();
        if count == 0 {
            return Err("This video has no pictures in it.".to_string());
        }
        let timescale = track.timescale().max(1) as u64;
        // display time of every frame from the two time tables (no need to read the whole file)
        let mut dts: Vec<u64> = Vec::with_capacity(count as usize);
        let mut t = 0u64;
        for e in &stbl.stts.entries {
            for _ in 0..e.sample_count {
                dts.push(t);
                t += e.sample_delta as u64;
            }
        }
        if dts.len() < count as usize {
            return Err("This video is a streaming-style (fragmented) MP4, which RhemaFlow cannot play yet. Re-save it as a normal MP4 (HandBrake can).".to_string());
        }
        dts.truncate(count as usize);
        let mut shown: Vec<i64> = dts.iter().map(|&d| d as i64).collect();
        if let Some(ctts) = &stbl.ctts {
            let mut i = 0usize;
            for e in &ctts.entries {
                for _ in 0..e.sample_count {
                    if i < shown.len() {
                        shown[i] += e.sample_offset as i64;
                    }
                    i += 1;
                }
            }
        }
        let min = shown.iter().copied().min().unwrap_or(0);
        let file_order_pts: Vec<u64> = shown.iter().map(|&p| ((p - min) as u64 * 1000) / timescale).collect();
        // S3.7: the decoder gives pictures in FILE order; B-frames mean that is not display order.
        // rank_of_decoded[n] = where the n-th decoded picture belongs when shown.
        let mut by_time: Vec<usize> = (0..file_order_pts.len()).collect();
        by_time.sort_by_key(|&i| (shown[i], i));
        let mut rank_of_decoded = vec![0usize; by_time.len()];
        for (rank, &i) in by_time.iter().enumerate() {
            rank_of_decoded[i] = rank;
        }
        let mut pts = file_order_pts;
        pts.sort_unstable();
        let duration_ms = (track.duration().as_millis() as u64).max(pts.last().copied().unwrap_or(0));
        let fps = if duration_ms > 0 { count as f32 * 1000.0 / duration_ms as f32 } else { track.frame_rate() as f32 };
        let info = VideoInfo { width: track.width() as u32, height: track.height() as u32, duration_ms, frame_count: count, fps };
        let sync: Vec<u32> = stbl.stss.as_ref().map(|s| s.entries.clone()).unwrap_or_default();
        Ok(VideoReader { mp4, track_id, nal_len, header, pts, info, decoder: new_decoder()?, next_sample: 1, out_index: 0, rank_of_decoded, decoded_count: 0, held: HashMap::new(), header_sent: false, sync })
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    /// Display time of the frame `next_frame` will give next (None at the end).
    pub fn peek_pts(&self) -> Option<u64> {
        self.pts.get(self.out_index).copied()
    }

    /// Display time of the very last frame.
    pub fn last_pts(&self) -> u64 {
        self.pts.last().copied().unwrap_or(0)
    }

    /// Back to the first frame.
    pub fn rewind(&mut self) -> Result<(), String> {
        self.decoder = new_decoder()?;
        self.next_sample = 1;
        self.out_index = 0;
        self.decoded_count = 0;
        self.held.clear();
        self.header_sent = false;
        Ok(())
    }

    /// Jump to the keyframe at or before `target_ms` (v0.833.0). Frames from there to the target still have to be
    /// decoded (`peek_pts` / `next_frame(false)` do that), but that is one short stretch (a second or two),
    /// not everything from the start of the file: jumping to minute 13 of a 22 minute video used to decode 13 minutes first.
    pub fn seek_to(&mut self, target_ms: u64) -> Result<(), String> {
        if self.pts.is_empty() {
            return self.rewind();
        }
        // display place of the wanted picture, then where it sits in the file
        let rank = self.pts.partition_point(|&p| p < target_ms).min(self.pts.len() - 1);
        let file_idx = self.rank_of_decoded.iter().position(|&r| r == rank).unwrap_or(rank);
        let want = file_idx as u32 + 1; // 1-based sample number
        // the latest keyframe at or before it
        let start = if self.sync.is_empty() { want } else { self.sync.iter().copied().filter(|&k| k <= want).max().unwrap_or(1) };
        if start <= 1 {
            return self.rewind();
        }
        self.decoder = new_decoder()?;
        self.next_sample = start;
        self.decoded_count = (start - 1) as usize;
        // the keyframe is the first picture the decoder will make: it is shown at its own place
        self.out_index = self.rank_of_decoded.get((start - 1) as usize).copied().unwrap_or(0);
        self.held.clear();
        self.header_sent = false;
        Ok(())
    }

    /// The next frame in display order. With `convert = false` the picture is decoded (it has to
    /// be, later frames depend on it) but not turned into RGBA, which is how a slow computer
    /// skips frames cheaply. `Ok(None)` = end of video.
    pub fn next_frame(&mut self, convert: bool) -> Result<Option<Frame>, String> {
        loop {
            // the frame whose turn it is may already have been decoded (it came out early)
            if let Some(image) = self.held.remove(&self.out_index) {
                return Ok(Some(self.hand_out(image)));
            }
            if self.next_sample > self.info.frame_count {
                // end of file: anything still held is shown in order (a frame the decoder never made leaves a gap)
                return Ok(match self.held.keys().min().copied() {
                    Some(k) => {
                        self.out_index = k;
                        let image = self.held.remove(&k).unwrap_or(Rgba { width: 0, height: 0, pixels: Vec::new() });
                        Some(self.hand_out(image))
                    }
                    None => None,
                });
            }
            if !self.header_sent {
                self.header_sent = true;
                let h = self.header.clone();
                // the set-up data alone produces no picture
                let _ = self.decoder.decode(&h);
            }
            let id = self.next_sample;
            self.next_sample += 1;
            let sample = self
                .mp4
                .read_sample(self.track_id, id)
                .map_err(|e| format!("The video file is damaged or cut short ({e})."))?
                .ok_or_else(|| "The video file is cut short.".to_string())?;
            let annexb = avcc_to_annexb(&sample.bytes, self.nal_len)?;
            let decoded = self.decoder.decode(&annexb).map_err(|e| format!("A video frame could not be decoded ({e}). The file may be damaged."))?;
            if let Some(yuv) = decoded {
                let (w, h) = yuv.dimension_rgb();
                let image = if convert {
                    let mut pixels = vec![0u8; w * h * 4];
                    yuv.write_rgba8(&mut pixels);
                    Rgba { width: w as u32, height: h as u32, pixels }
                } else {
                    Rgba { width: w as u32, height: h as u32, pixels: Vec::new() }
                };
                let n = self.decoded_count;
                self.decoded_count += 1;
                let rank = self.rank_of_decoded.get(n).copied().unwrap_or(n);
                if rank >= self.out_index {
                    self.held.insert(rank, image);
                }
            }
        }
    }

    fn hand_out(&mut self, image: Rgba) -> Frame {
        let pts_ms = self.pts.get(self.out_index).copied().unwrap_or(0);
        self.out_index += 1;
        Frame { pts_ms, image }
    }
}

/// MP4 stores each NAL unit with a length in front; the decoder wants a start code `00 00 00 01`.
pub(crate) fn avcc_to_annexb(data: &[u8], nal_len: usize) -> Result<Vec<u8>, String> {
    if !(1..=4).contains(&nal_len) {
        return Err("The video's set-up information is not valid.".to_string());
    }
    let mut out = Vec::with_capacity(data.len() + 16);
    let mut i = 0;
    while i < data.len() {
        if i + nal_len > data.len() {
            return Err("A video frame is cut short.".to_string());
        }
        let len = data[i..i + nal_len].iter().fold(0usize, |a, &b| (a << 8) | b as usize);
        i += nal_len;
        if len == 0 {
            continue;
        }
        if i + len > data.len() {
            return Err("A video frame is cut short.".to_string());
        }
        out.extend_from_slice(&START);
        out.extend_from_slice(&data[i..i + len]);
        i += len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avcc_conversion_handles_each_length_size_and_refuses_cut_data() {
        // two NALs, 4-byte lengths
        let d = [0, 0, 0, 2, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC];
        assert_eq!(avcc_to_annexb(&d, 4).unwrap(), vec![0, 0, 0, 1, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC]);
        // 2-byte lengths
        assert_eq!(avcc_to_annexb(&[0, 1, 0xDD], 2).unwrap(), vec![0, 0, 0, 1, 0xDD]);
        // 1-byte lengths
        assert_eq!(avcc_to_annexb(&[2, 7, 8], 1).unwrap(), vec![0, 0, 0, 1, 7, 8]);
        // length runs past the end
        assert!(avcc_to_annexb(&[0, 0, 0, 9, 1, 2], 4).is_err());
        // not even a whole length field
        assert!(avcc_to_annexb(&[0, 0], 4).is_err());
        assert!(avcc_to_annexb(&[1], 7).is_err());
        assert!(avcc_to_annexb(&[], 4).unwrap().is_empty());
    }
}
