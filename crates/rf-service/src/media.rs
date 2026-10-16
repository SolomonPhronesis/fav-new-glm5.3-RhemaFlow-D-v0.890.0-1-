//! Pictures and video: the rules and the maths (Stage 2, spec sections 3 and 4).
//! Decoding itself lives in the `rf-media` crate; nothing here touches a file or a screen.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    /// Sound only (mp3, wav, m4a ...): played by the Mac's own sound player, nothing is drawn.
    Audio,
    /// A PDF or an HTML page (v0.840.0): shown one page at a time, with zoom and a reading line.
    Document,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Classified {
    Ok(MediaKind),
    /// A file type we know about but cannot show, with advice in plain words.
    Unsupported(String),
    /// Not a media file at all.
    Unknown,
}

pub const CONVERT_VIDEO: &str = "RhemaFlow plays MP4 files with H.264 video. Convert it to an MP4 with H.264 video first (the free HandBrake app does this) and add that file instead.";
pub const CONVERT_AUDIO: &str = "RhemaFlow plays MP3, WAV, M4A, AAC, AIFF and FLAC sound files. Convert this one to MP3 or M4A first (the free VLC or Music app can do it) and add that file instead.";
pub const CONVERT_IMAGE: &str = "RhemaFlow shows PNG, JPEG, GIF and BMP pictures. Save or export this one as PNG or JPEG first (Preview on a Mac can do it) and add that file instead.";

/// S3.1: by file extension (the real check happens when the file is opened).
pub fn classify(path: &str) -> Classified {
    let ext = match path.rsplit_once('.') {
        Some((_, e)) if !e.contains('/') && !e.contains('\\') => e.to_ascii_lowercase(),
        _ => return Classified::Unknown,
    };
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" => Classified::Ok(MediaKind::Image),
        "mp4" | "m4v" => Classified::Ok(MediaKind::Video),
        "pdf" | "html" | "htm" | "xhtml" => Classified::Ok(MediaKind::Document),
        "mp3" | "wav" | "m4a" | "aac" | "aif" | "aiff" | "caf" | "flac" => Classified::Ok(MediaKind::Audio),
        "ogg" | "oga" | "opus" | "wma" | "mid" | "midi" | "amr" => Classified::Unsupported(CONVERT_AUDIO.to_string()),
        "mov" | "mkv" | "avi" | "webm" | "wmv" | "flv" | "mpg" | "mpeg" | "hevc" | "h265" | "ts" => Classified::Unsupported(CONVERT_VIDEO.to_string()),
        "webp" | "heic" | "heif" | "tif" | "tiff" | "svg" | "avif" => Classified::Unsupported(CONVERT_IMAGE.to_string()),
        _ => Classified::Unknown,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitMode {
    /// Whole picture visible, bands in the stage colour.
    Fit,
    /// Screen fully covered, edges cropped.
    Fill,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// S3.3: where to draw a `src_w` x `src_h` picture inside a `dst_w` x `dst_h` screen (origin = screen's top-left).
/// For `Fill` the rectangle is bigger than the screen; the painter clips it.
pub fn fit_rect(src_w: f32, src_h: f32, dst_w: f32, dst_h: f32, mode: FitMode) -> Rect {
    let (dw, dh) = (dst_w.max(0.0), dst_h.max(0.0));
    if !(src_w > 0.0 && src_h > 0.0 && dw > 0.0 && dh > 0.0) || mode == FitMode::Stretch {
        return Rect { x: 0.0, y: 0.0, w: dw, h: dh };
    }
    let (sx, sy) = (dw / src_w, dh / src_h);
    let scale = if mode == FitMode::Fit { sx.min(sy) } else { sx.max(sy) };
    let (w, h) = (src_w * scale, src_h * scale);
    Rect { x: (dw - w) / 2.0, y: (dh - h) / 2.0, w, h }
}

/// A video's own clock (S3.4 to S3.6). It is told the time; it never reads one.
#[derive(Clone, Debug)]
pub struct PlaybackClock {
    duration_ms: u64,
    looping: bool,
    base_ms: u64,
    since: Option<u64>,
}

impl PlaybackClock {
    pub fn new(duration_ms: u64, looping: bool) -> PlaybackClock {
        PlaybackClock { duration_ms, looping, base_ms: 0, since: None }
    }
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    pub fn is_looping(&self) -> bool {
        self.looping
    }
    pub fn is_playing(&self) -> bool {
        self.since.is_some()
    }

    pub fn position_ms(&self, now_ms: u64) -> u64 {
        let raw = self.base_ms + self.since.map(|s| now_ms.saturating_sub(s)).unwrap_or(0);
        if self.looping {
            if self.duration_ms == 0 {
                0
            } else {
                raw % self.duration_ms
            }
        } else {
            raw.min(self.duration_ms)
        }
    }

    pub fn ended(&self, now_ms: u64) -> bool {
        !self.looping && self.duration_ms > 0 && self.position_ms(now_ms) >= self.duration_ms
    }

    pub fn play(&mut self, now_ms: u64) {
        if self.since.is_some() {
            return;
        }
        if !self.looping && self.duration_ms > 0 && self.base_ms >= self.duration_ms {
            self.base_ms = 0; // pressing play at the very end starts again
        }
        self.since = Some(now_ms);
    }
    pub fn pause(&mut self, now_ms: u64) {
        self.base_ms = self.position_ms(now_ms);
        self.since = None;
    }
    pub fn restart(&mut self, now_ms: u64) {
        self.base_ms = 0;
        self.since = Some(now_ms);
    }
    /// Jump to `ms` (kept inside the video) and carry on playing or stay paused as before.
    pub fn seek(&mut self, ms: u64, now_ms: u64) {
        let playing = self.since.is_some();
        self.base_ms = if self.duration_ms == 0 { 0 } else { ms.min(self.duration_ms) };
        self.since = if playing { Some(now_ms) } else { None };
    }
    pub fn set_looping(&mut self, on: bool, now_ms: u64) {
        let pos = self.position_ms(now_ms);
        let playing = self.since.is_some();
        self.looping = on;
        self.base_ms = pos;
        self.since = if playing { Some(now_ms) } else { None };
    }
}

/// S4.3
pub const DEFAULT_DIM_PCT: u8 = 45;
pub const MAX_DIM_PCT: u8 = 85;

/// Opacity (0..=255) of the black layer between a background and the words.
pub fn dim_alpha(pct: u8) -> u8 {
    let p = pct.min(MAX_DIM_PCT) as u32;
    ((p * 255 + 50) / 100) as u8
}
