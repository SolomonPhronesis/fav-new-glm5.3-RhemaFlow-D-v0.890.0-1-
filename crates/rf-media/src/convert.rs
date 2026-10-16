//! Make a file RhemaFlow can play out of one it cannot (v0.825.0).
//!
//! Why: phones record HEVC video, cameras make .mov, and some sound files are .ogg. The app only
//! plays H.264 MP4 and common sound files, so such a file used to stop with an error. If the free
//! program `ffmpeg` is on the Mac, RhemaFlow now runs it in the background to make a good copy.
//! RhemaFlow does not include ffmpeg and does not link to it: it only starts it as a separate
//! program (so its licence never mixes with ours). If it is missing, the person is told how to get it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const INSTALL_HINT: &str = "To let RhemaFlow fix this by itself, install the free ffmpeg program once (in Terminal: brew install ffmpeg), then add the file again. Or convert it with HandBrake.";

/// Where ffmpeg might be. A Finder-launched app does not see the shell's PATH, so Homebrew's folders are tried by name.
pub fn find_ffmpeg() -> Option<PathBuf> {
    if std::env::var_os("RHEMAFLOW_NO_FFMPEG").is_some() {
        return None; // a switch for testing, and for anyone who wants the converter off
    }
    let mut places: Vec<PathBuf> = vec!["/opt/homebrew/bin/ffmpeg".into(), "/usr/local/bin/ffmpeg".into(), "/usr/bin/ffmpeg".into()];
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            places.push(d.join("ffmpeg"));
        }
    }
    if let Ok(p) = std::env::var("RHEMAFLOW_FFMPEG") {
        places.insert(0, p.into());
    }
    places.into_iter().find(|p| p.is_file())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// H.264 video + AAC sound in an MP4, at most 1920 wide, plain 4:2:0 colour.
    Video,
    /// AAC sound in an M4A.
    Audio,
}

impl Target {
    pub fn extension(self) -> &'static str {
        match self {
            Target::Video => "mp4",
            Target::Audio => "m4a",
        }
    }
}

/// The ffmpeg command line (each piece is ONE argument, so spaces in names are safe).
pub fn args(input: &Path, output: &Path, target: Target) -> Vec<String> {
    let s = |p: &Path| p.to_string_lossy().to_string();
    let mut a: Vec<String> = ["-y", "-nostdin", "-hide_banner", "-i"].iter().map(|x| x.to_string()).collect();
    a.push(s(input));
    match target {
        Target::Video => {
            for x in [
                "-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn",
                "-vf", "scale='min(1920,iw)':-2,format=yuv420p",
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-profile:v", "high",
                "-c:a", "aac", "-b:a", "160k", "-ac", "2",
                "-movflags", "+faststart",
            ] {
                a.push(x.to_string());
            }
        }
        Target::Audio => {
            for x in ["-map", "0:a:0", "-vn", "-sn", "-dn", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"] {
                a.push(x.to_string());
            }
        }
    }
    a.push("-f".into());
    a.push(match target { Target::Video => "mp4", Target::Audio => "ipod" }.into());
    a.push(s(output));
    a
}

/// "Duration: 00:01:23.45," or "time=00:00:12.34" -> seconds.
pub fn parse_clock(line: &str, key: &str) -> Option<f64> {
    let at = line.find(key)? + key.len();
    let t: String = line[at..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.').collect();
    let mut secs = 0.0;
    for part in t.split(':') {
        secs = secs * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(secs)
}

/// The last useful line of ffmpeg's complaint, in a form a person can read.
pub fn explain_failure(stderr_tail: &str) -> String {
    let line = stderr_tail.lines().rev().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with("frame=") && !l.starts_with("size=")).unwrap_or("no details");
    let low = line.to_lowercase();
    let why = if low.contains("no such file") { "the file could not be found".to_string() }
        else if low.contains("invalid data") || low.contains("moov atom not found") { "the file looks damaged or cut short".to_string() }
        else if low.contains("does not contain any stream") || low.contains("matches no streams") { "the file has no picture or sound in it that can be used".to_string() }
        else if low.contains("permission denied") { "the computer would not let RhemaFlow write there".to_string() }
        else if low.contains("no space") { "the disk is full".to_string() }
        else { format!("ffmpeg said: {line}") };
    format!("The conversion did not work: {why}.")
}

/// Convert `input` into `out_dir/<name>.mp4|m4a`. `progress` gets 0.0..=1.0 as it goes.
/// The result appears under its final name only when it is complete.
pub fn convert(input: &Path, out_dir: &Path, target: Target, progress: &mut dyn FnMut(f32)) -> Result<PathBuf, String> {
    let ff = find_ffmpeg().ok_or_else(|| INSTALL_HINT.to_string())?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Cannot make the folder for the converted file ({e})."))?;
    let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "converted".into());
    let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let dest = out_dir.join(format!("{safe}.{}", target.extension()));
    if dest.is_file() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        progress(1.0);
        return Ok(dest); // converted before
    }
    let part = out_dir.join(format!("{safe}.part"));
    let mut child = Command::new(ff).args(args(input, &part, target)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Could not start ffmpeg ({e}). {INSTALL_HINT}"))?;
    let mut err = child.stderr.take().ok_or("no output from ffmpeg")?;
    let (mut buf, mut tail, mut total) = ([0u8; 4096], String::new(), None::<f64>);
    let mut line = String::new();
    loop {
        let n = err.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                if total.is_none() {
                    total = parse_clock(&line, "Duration:").filter(|d| *d > 0.0);
                }
                if let (Some(t), Some(now)) = (total, parse_clock(&line, "time=")) {
                    progress(((now / t) as f32).clamp(0.0, 0.99));
                }
                tail.push_str(&line);
                tail.push('\n');
                if tail.len() > 3000 {
                    tail = tail[tail.len() - 2000..].to_string();
                }
                line.clear();
            } else {
                line.push(b as char);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() || !part.is_file() {
        let _ = std::fs::remove_file(&part);
        return Err(explain_failure(&tail));
    }
    std::fs::rename(&part, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("Could not save the converted file ({e}).")
    })?;
    progress(1.0);
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_keeps_paths_whole_and_makes_plain_h264() {
        let a = args(Path::new("/My Films/Baptism 2026.mov"), Path::new("/out dir/x.part"), Target::Video);
        assert!(a.contains(&"/My Films/Baptism 2026.mov".to_string()), "one argument, spaces and all");
        assert!(a.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(a.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert!(a.contains(&"0:a:0?".to_string()), "a video with no sound still converts");
        assert!(a.iter().any(|x| x.contains("yuv420p")), "the decoder needs plain 4:2:0 colour");
        assert!(a.contains(&"+faststart".to_string()));
        assert_eq!(a.last().unwrap(), "/out dir/x.part");
        let s = args(Path::new("a.ogg"), Path::new("b.part"), Target::Audio);
        assert!(s.contains(&"-vn".to_string()) && !s.contains(&"libx264".to_string()));
    }

    #[test]
    fn clock_text_from_ffmpeg_is_read() {
        assert_eq!(parse_clock("  Duration: 00:01:23.50, start: 0.0", "Duration:"), Some(83.5));
        assert_eq!(parse_clock("frame= 10 fps=0 time=00:00:02.00 bitrate=", "time="), Some(2.0));
        assert_eq!(parse_clock("no clock here", "time="), None);
    }

    #[test]
    fn failures_are_explained_not_dumped() {
        assert!(explain_failure("x.mov: Invalid data found when processing input").contains("damaged"));
        assert!(explain_failure("").contains("did not work"));
        assert!(explain_failure("frame= 1\nsize= 0\n").contains("no details"));
    }

    #[test]
    fn the_extensions_are_right() {
        assert_eq!(Target::Video.extension(), "mp4");
        assert_eq!(Target::Audio.extension(), "m4a");
    }
}
