//! Sermon clips (v0.835.0): mark a moment while the preacher speaks (or just after) and get a short audio clip,
//! a caption file and the words, ready for social media or a "listen again" page.
//!
//! Marks are kept in service time. A clip is cut from the service recording with ffmpeg when it is installed;
//! the words (`.txt`) and captions (`.srt`) and a timecode list (`clips.csv`) are always written, so a video editor can cut
//! the OBS recording from the same times even without ffmpeg.

use crate::export::{hms, Line};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: u64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
    /// the words said during the clip
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct ClipBook {
    pub clips: Vec<Clip>,
    /// a start has been marked and the end has not
    pub open_start: Option<u64>,
    next_id: u64,
}

pub const MIN_CLIP_MS: u64 = 3_000;
pub const MAX_CLIP_MS: u64 = 10 * 60 * 1000;
pub const QUICK_BACK_MS: u64 = 30_000;

/// The words of every transcript line that starts inside [a, b].
pub fn text_between(lines: &[Line], a: u64, b: u64) -> String {
    lines.iter().filter(|l| l.ms >= a && l.ms <= b).map(|l| l.text.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A working title from the first words of the clip.
pub fn auto_title(text: &str) -> String {
    let w: Vec<&str> = text.split_whitespace().take(8).collect();
    if w.is_empty() {
        return "Clip".to_string();
    }
    let mut t = w.join(" ");
    t = t.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
    if text.split_whitespace().count() > 8 {
        t.push_str("...");
    }
    t
}

impl ClipBook {
    /// "Start clip": remembers the time. A second press moves the start.
    pub fn mark_in(&mut self, now_ms: u64) {
        self.open_start = Some(now_ms);
    }

    /// "End clip": makes the clip from the marked start to now.
    pub fn mark_out(&mut self, now_ms: u64, lines: &[Line]) -> Result<u64, String> {
        let Some(start) = self.open_start else { return Err("Press \"Start clip\" first.".into()) };
        self.add(start, now_ms, lines)
    }

    /// One press: the last `back_ms` up to now (for "that was good, keep it").
    pub fn quick(&mut self, now_ms: u64, back_ms: u64, lines: &[Line]) -> Result<u64, String> {
        self.add(now_ms.saturating_sub(back_ms), now_ms, lines)
    }

    pub fn add(&mut self, start_ms: u64, end_ms: u64, lines: &[Line]) -> Result<u64, String> {
        if end_ms <= start_ms || end_ms - start_ms < MIN_CLIP_MS {
            return Err("That clip is under 3 seconds. Mark a longer moment.".into());
        }
        let end_ms = end_ms.min(start_ms + MAX_CLIP_MS);
        self.next_id += 1;
        let text = text_between(lines, start_ms, end_ms);
        let title = auto_title(&text);
        self.clips.push(Clip { id: self.next_id, start_ms, end_ms, title, text });
        self.open_start = None;
        Ok(self.next_id)
    }

    pub fn remove(&mut self, id: u64) {
        self.clips.retain(|c| c.id != id);
    }

    pub fn rename(&mut self, id: u64, title: &str) {
        if let Some(c) = self.clips.iter_mut().find(|c| c.id == id) {
            c.title = title.trim().to_string();
        }
    }

    /// Nudge a clip's edges (in milliseconds, either direction).
    pub fn trim(&mut self, id: u64, start_delta: i64, end_delta: i64, lines: &[Line]) {
        if let Some(c) = self.clips.iter_mut().find(|c| c.id == id) {
            let s = (c.start_ms as i64 + start_delta).max(0) as u64;
            let e = (c.end_ms as i64 + end_delta).max(0) as u64;
            if e > s && e - s >= MIN_CLIP_MS && e - s <= MAX_CLIP_MS {
                c.start_ms = s;
                c.end_ms = e;
                c.text = text_between(lines, s, e);
            }
        }
    }
}

/// SubRip captions for one clip, with times counted from the start of the clip.
pub fn srt_for(lines: &[Line], c: &Clip) -> String {
    let inside: Vec<Line> = lines.iter().filter(|l| l.ms >= c.start_ms && l.ms <= c.end_ms).map(|l| Line { ms: l.ms - c.start_ms, text: l.text.clone() }).collect();
    crate::export::to_srt(&inside, c.end_ms - c.start_ms)
}

/// ffmpeg arguments that cut [start, start+duration] from the recording. Audio clips are AAC in an .m4a; with
/// `video` the picture is kept too. Times are put before `-i` (fast, accurate to the frame for re-encoded output).
pub fn ffmpeg_args(input: &Path, start_ms: u64, dur_ms: u64, out: &Path, video: bool) -> Vec<String> {
    let t = |ms: u64| format!("{}.{:03}", ms / 1000, ms % 1000);
    let mut a: Vec<String> = ["-y", "-v", "error", "-ss"].map(String::from).to_vec();
    a.push(t(start_ms));
    a.push("-i".into());
    a.push(input.to_string_lossy().to_string());
    a.push("-t".into());
    a.push(t(dur_ms));
    if video {
        a.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k"].map(String::from));
    } else {
        a.extend(["-vn", "-c:a", "aac", "-b:a", "160k"].map(String::from));
    }
    a.push(out.to_string_lossy().to_string());
    a
}

/// "03 Grace that finds us" safe for any file system.
pub fn file_stem(n: usize, title: &str) -> String {
    let s = crate::export::safe_name(title);
    let s = if s.is_empty() { "clip".to_string() } else { s };
    format!("{n:02} {s}")
}

/// A timecode list a video editor can use: `n,start,end,length,title`.
pub fn csv(clips: &[Clip]) -> String {
    let mut s = String::from("number,start,end,length_seconds,title\n");
    for (i, c) in clips.iter().enumerate() {
        let title = c.title.replace('"', "\"\"");
        s.push_str(&format!("{},{},{},{},\"{}\"\n", i + 1, hms(c.start_ms), hms(c.end_ms), (c.end_ms - c.start_ms) / 1000, title));
    }
    s
}

#[derive(Debug, Default)]
pub struct ExportReport {
    pub written: Vec<String>,
    pub audio_made: usize,
    pub problems: Vec<String>,
}

/// Write everything for the clips into `dir`. `recording` is the service audio file (None = not recorded);
/// `rec_offset_ms` is how long after service time 0 the recording began (so a clip at 10:00 is at 10:00 - offset in the file).
pub fn export(dir: &Path, clips: &[Clip], lines: &[Line], recording: Option<&Path>, rec_offset_ms: u64, ffmpeg: Option<&Path>) -> ExportReport {
    let mut r = ExportReport::default();
    if clips.is_empty() {
        r.problems.push("There are no clips to save.".into());
        return r;
    }
    if let Err(e) = std::fs::create_dir_all(dir) {
        r.problems.push(format!("Could not make the folder ({e})."));
        return r;
    }
    let mut put = |name: String, body: String, r: &mut ExportReport| match std::fs::write(dir.join(&name), body) {
        Ok(()) => r.written.push(name),
        Err(e) => r.problems.push(format!("Could not write {name} ({e}).")),
    };
    put("clips.csv".into(), csv(clips), &mut r);
    for (i, c) in clips.iter().enumerate() {
        let stem = file_stem(i + 1, &c.title);
        put(format!("{stem}.txt"), format!("{}\n\n{}\n", c.title, c.text), &mut r);
        put(format!("{stem}.srt"), srt_for(lines, c), &mut r);
        match (recording, ffmpeg) {
            (Some(rec), Some(ff)) => {
                if c.start_ms < rec_offset_ms {
                    r.problems.push(format!("\"{}\" starts before the recording did, so no audio was cut.", c.title));
                    continue;
                }
                let out = dir.join(format!("{stem}.m4a"));
                let args = ffmpeg_args(rec, c.start_ms - rec_offset_ms, c.end_ms - c.start_ms, &out, false);
                match std::process::Command::new(ff).args(args).stdin(std::process::Stdio::null()).output() {
                    Ok(o) if o.status.success() && out.exists() => {
                        r.audio_made += 1;
                        r.written.push(format!("{stem}.m4a"));
                    }
                    _ => r.problems.push(format!("ffmpeg could not cut \"{}\".", c.title)),
                }
            }
            (None, _) => {}
            (Some(_), None) => {}
        }
    }
    if recording.is_none() {
        r.problems.push("This service was not recorded, so there is no audio to cut. The words, captions and times were saved.".into());
    } else if ffmpeg.is_none() {
        r.problems.push("ffmpeg is not installed (brew install ffmpeg), so no audio was cut. The words, captions and times were saved.".into());
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<Line> {
        vec![
            Line { ms: 5_000, text: "Before the clip".into() },
            Line { ms: 62_000, text: "Grace is not earned".into() },
            Line { ms: 70_000, text: "it is given freely".into() },
            Line { ms: 80_000, text: "to all who ask".into() },
            Line { ms: 200_000, text: "after".into() },
        ]
    }

    #[test]
    fn start_then_end_makes_a_clip_with_its_words_and_a_title() {
        let mut b = ClipBook::default();
        assert!(b.mark_out(10_000, &lines()).is_err(), "no start yet");
        b.mark_in(60_000);
        let id = b.mark_out(85_000, &lines()).unwrap();
        let c = &b.clips[0];
        assert_eq!(c.id, id);
        assert_eq!(c.text, "Grace is not earned it is given freely to all who ask");
        assert_eq!(c.title, "Grace is not earned it is given freely...");
        assert!(b.open_start.is_none());
    }

    #[test]
    fn one_press_takes_the_last_half_minute_and_tiny_or_huge_clips_are_handled() {
        let mut b = ClipBook::default();
        b.quick(90_000, QUICK_BACK_MS, &lines()).unwrap();
        assert_eq!((b.clips[0].start_ms, b.clips[0].end_ms), (60_000, 90_000));
        assert!(b.add(0, 2_000, &lines()).is_err());
        assert!(b.add(5_000, 5_000, &lines()).is_err());
        b.add(0, 3_600_000, &lines()).unwrap();
        assert_eq!(b.clips[1].end_ms, MAX_CLIP_MS, "capped at ten minutes");
        b.quick(10_000, QUICK_BACK_MS, &lines()).unwrap();
        assert_eq!(b.clips[2].start_ms, 0, "near the start it cannot go before zero");
    }

    #[test]
    fn trimming_changes_the_words_and_refuses_nonsense() {
        let mut b = ClipBook::default();
        let id = b.add(60_000, 85_000, &lines()).unwrap();
        b.trim(id, 5_000, 0, &lines()); // start at 65 s: drops "Grace is not earned"
        assert_eq!(b.clips[0].text, "it is given freely to all who ask");
        b.trim(id, 0, -100_000, &lines()); // would end before it starts
        assert_eq!(b.clips[0].end_ms, 85_000);
        b.rename(id, "  Grace  ");
        assert_eq!(b.clips[0].title, "Grace");
        b.remove(id);
        assert!(b.clips.is_empty());
    }

    #[test]
    fn captions_count_from_the_start_of_the_clip() {
        let c = Clip { id: 1, start_ms: 60_000, end_ms: 85_000, title: "t".into(), text: String::new() };
        let srt = srt_for(&lines(), &c);
        assert!(srt.starts_with("1\n00:00:02,000 -->"), "{srt}");
        assert!(srt.contains("it is given freely"));
        assert!(!srt.contains("Before the clip") && !srt.contains("after"));
    }

    #[test]
    fn the_ffmpeg_command_cuts_the_right_stretch_and_keeps_paths_whole() {
        let a = ffmpeg_args(Path::new("/s/My Service.wav"), 62_500, 20_000, Path::new("/o/01 Grace.m4a"), false);
        let j = a.join("|");
        assert!(j.contains("-ss|62.500|-i|/s/My Service.wav|-t|20.000|-vn|-c:a|aac"), "{j}");
        assert_eq!(a.last().unwrap(), "/o/01 Grace.m4a");
        assert!(ffmpeg_args(Path::new("a.mp4"), 0, 1000, Path::new("o.mp4"), true).join(" ").contains("libx264"));
    }

    #[test]
    fn the_timecode_list_and_file_names_are_clean() {
        let c = Clip { id: 1, start_ms: 3_725_000, end_ms: 3_760_000, title: "He said \"grace\"".into(), text: String::new() };
        let s = csv(&[c]);
        assert!(s.starts_with("number,start,end,length_seconds,title\n1,01:02:05,"), "{s}");
        assert!(s.contains("\"He said \"\"grace\"\"\""));
        assert_eq!(file_stem(3, "Grace: that/finds us?"), "03 Grace- that-finds us");
        assert_eq!(file_stem(1, "???"), "01 clip");
    }

    #[test]
    fn exporting_without_a_recording_still_saves_words_captions_and_times() {
        let d = std::env::temp_dir().join(format!("rf-clips-{}", std::process::id()));
        let c = Clip { id: 1, start_ms: 60_000, end_ms: 85_000, title: "Grace".into(), text: "Grace is not earned".into() };
        let r = export(&d, &[c], &lines(), None, 0, None);
        assert!(r.written.contains(&"clips.csv".to_string()) && r.written.contains(&"01 Grace.txt".to_string()) && r.written.contains(&"01 Grace.srt".to_string()));
        assert_eq!(r.audio_made, 0);
        assert!(r.problems.iter().any(|p| p.contains("not recorded")));
        assert!(export(&d, &[], &lines(), None, 0, None).problems[0].contains("no clips"));
    }

    #[test]
    fn exporting_with_real_ffmpeg_cuts_real_audio() {
        let Some(ff) = rf_media::convert::find_ffmpeg() else { eprintln!("SKIPPED: no ffmpeg"); return };
        let d = std::env::temp_dir().join(format!("rf-clips-real-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let rec = d.join("service.wav");
        assert!(std::process::Command::new(&ff).args(["-y", "-v", "error", "-f", "lavfi", "-i", "sine=duration=100", "-ar", "16000"]).arg(&rec).status().unwrap().success());
        let clip = Clip { id: 1, start_ms: 70_000, end_ms: 80_000, title: "Ten seconds".into(), text: "x".into() };
        let r = export(&d.join("clips"), &[clip], &lines(), Some(&rec), 10_000, Some(&ff));
        assert_eq!(r.audio_made, 1, "{:?}", r.problems);
        let m4a = d.join("clips").join("01 Ten seconds.m4a");
        assert!(m4a.exists() && std::fs::metadata(&m4a).unwrap().len() > 1000);
    }
}
