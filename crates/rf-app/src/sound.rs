//! Sound for video (Stage 2, spec S3.9).
//!
//! The H.264 decoder only makes pictures. A Mac already has a program called `afplay` that plays
//! the sound of a media file, so we use that instead of adding a sound-decoding library (the usual
//! Rust one is MPL-licensed, which is not on our allowed list; see docs/STAGE2-SPEC.md section 8).
//!
//! Pausing is done by freezing and unfreezing that program (`kill -STOP` / `kill -CONT`).
//! `afplay` cannot start part-way in, so "restart" means stop and start again. To JUMP to a time
//! (rewind / fast-forward / drag the bar) the free `ffmpeg` program, if it is installed, cuts a copy
//! of the sound that starts at that time and `afplay` plays that copy. Without ffmpeg, jumping says so.
//! The length comes from the Mac's own `afinfo` program, and the position from a clock we keep.
//!
//! NOT VERIFIED on a real Mac in this version: only the command lines are tested.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use crate::macaudio::NativePlayer;

static SEEK_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Said when a jump is asked for but ffmpeg is not installed.
pub const NO_SEEK_HINT: &str = "Jumping to a time in a sound needs the free ffmpeg program (in Terminal: brew install ffmpeg). Pause and Play still work.";

/// The length in milliseconds from `afinfo`'s text ("estimated duration: 215.032000 sec").
pub fn parse_afinfo_duration(text: &str) -> Option<u64> {
    for line in text.lines() {
        let low = line.trim().to_lowercase();
        if let Some(rest) = low.strip_prefix("estimated duration:") {
            let secs: f64 = rest.trim().trim_end_matches("sec").trim().parse().ok()?;
            if secs.is_finite() && secs >= 0.0 {
                return Some((secs * 1000.0).round() as u64);
            }
        }
    }
    None
}

/// The ffmpeg command that writes a copy of `input` starting at `start_ms` (video dropped).
/// MP3 is copied as it is (instant, any length); anything else becomes AAC in an M4A.
pub fn seek_command(ffmpeg: &str, input: &str, out: &str, start_ms: u64) -> (String, Vec<String>) {
    let is_mp3 = input.to_lowercase().ends_with(".mp3");
    let mut a: Vec<String> = vec!["-y".into(), "-v".into(), "error".into(), "-ss".into(), format!("{}.{:03}", start_ms / 1000, start_ms % 1000), "-i".into(), input.into(), "-vn".into()];
    if is_mp3 {
        a.extend(["-c:a", "copy", "-f", "mp3"].map(String::from));
    } else {
        a.extend(["-c:a", "aac", "-b:a", "160k", "-f", "ipod"].map(String::from));
    }
    a.push(out.into());
    (ffmpeg.to_string(), a)
}

/// Where a jump goes: `from` plus `delta`, kept between 0 and the end (a little before it, so something still plays).
pub fn jump_target(from_ms: u64, delta_ms: i64, duration_ms: u64) -> u64 {
    let t = (from_ms as i64 + delta_ms).max(0) as u64;
    if duration_ms > 1000 {
        t.min(duration_ms - 500)
    } else {
        t
    }
}

fn seek_extension(input: &str) -> &'static str {
    if input.to_lowercase().ends_with(".mp3") { "mp3" } else { "m4a" }
}

/// How a jump's copy of the sound is being made: by RhemaFlow itself (AAC in an MP4, instant, no other program),
/// or by ffmpeg (anything else).
enum Cutter {
    Native(std::thread::JoinHandle<Result<(), String>>),
    Ffmpeg(Child),
}

struct SeekJob {
    cutter: Cutter,
    out: PathBuf,
    resume: bool,
    input: String,
    target: u64,
}

fn native_cuttable(input: &str) -> bool {
    let l = input.to_lowercase();
    [".mp4", ".m4v", ".mov", ".m4a", ".m4b"].iter().any(|e| l.ends_with(e))
}

fn drop_job(mut j: SeekJob) {
    if let Cutter::Ffmpeg(c) = &mut j.cutter {
        let _ = c.kill();
        let _ = c.wait();
    }
    let _ = std::fs::remove_file(&j.out);
}

/// The program and arguments that play `path` at `volume` (0.0 to 1.0).
pub fn afplay_command(path: &str, volume: f32) -> (String, Vec<String>) {
    ("afplay".to_string(), vec!["-v".to_string(), format!("{:.2}", volume.clamp(0.0, 1.0)), path.to_string()])
}

/// `kill -STOP <pid>` / `kill -CONT <pid>`.
pub fn signal_command(pid: u32, signal: &str) -> (String, Vec<String>) {
    ("kill".to_string(), vec![format!("-{signal}"), pid.to_string()])
}

#[derive(Default)]
pub struct Sound {
    child: Option<Child>,
    paused: bool,
    /// Set once when the player program quit with a failure (so the operator is told, not left in silence).
    failure_told: bool,
    path: String,
    volume: f32,
    duration_ms: u64,
    /// Where in the sound we are: `base_ms` plus the time since `since` (if it is going).
    base_ms: u64,
    since: Option<Instant>,
    seeking: Option<SeekJob>,
    /// A cut copy that is ready but waiting for Play (a jump made while paused).
    ready: Option<PathBuf>,
    temp: Option<PathBuf>,
    /// v0.836.0: the picture of a video is held still while a jumped-to copy of the sound is made, then released
    /// together with the sound (before, the picture ran on during the cut and the sound came in late).
    video_hold: bool,
    /// v0.837.0: afplay takes a moment to open the file and start sound; the picture waits this long after it is launched.
    pub latency_ms: u64,
    spawned: Option<Instant>,
    /// v0.839.0: the Mac's in-process audio player (instant start, instant jump, true position). None = afplay.
    native: Option<NativePlayer>,
    native_paused: bool,
    native_started: bool,
    last_drift_check: Option<Instant>,
}

/// Plain-words reason for a failed `afplay`, from what it printed (or its exit code).
pub fn failure_text(file: &str, stderr: &str, code: Option<i32>) -> String {
    let low = stderr.to_lowercase();
    let why = if low.contains("audiofiletypeid") || low.contains("typ?") || low.contains("fmt?") || low.contains("not supported") || low.contains("unsupported") {
        "this Mac cannot read that sound format; convert it to MP3 or M4A and add the new file".to_string()
    } else if low.contains("no such file") || low.contains("can't open") || low.contains("cannot open") || low.contains("open failed") {
        "the file could not be opened (moved, deleted, or no permission)".to_string()
    } else if !stderr.trim().is_empty() {
        format!("the Mac said: {}", stderr.trim().lines().next().unwrap_or(""))
    } else {
        format!("the sound player stopped with code {}", code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into()))
    };
    format!("The sound of \"{file}\" could not be played: {why}.")
}

impl Sound {
    pub fn none() -> Sound {
        Sound::default()
    }

    /// Start (or restart) the sound from the beginning. False if it could not start (not a Mac, or no `afplay`).
    pub fn start(&mut self, path: &str, volume: f32) -> bool {
        self.stop();
        self.path = path.to_string();
        self.volume = volume;
        self.duration_ms = probe_duration(path);
        if let Some(n) = NativePlayer::open(path) {
            n.set_volume(volume);
            if n.play() {
                let d = (n.duration_secs() * 1000.0) as u64;
                if d > 0 {
                    self.duration_ms = d;
                }
                self.native = Some(n);
                self.native_paused = false;
                self.native_started = true;
                self.base_ms = 0;
                self.since = Some(Instant::now());
                self.spawned = Some(Instant::now());
                return true;
            }
        }
        if self.spawn_player(path) {
            self.base_ms = 0;
            self.since = Some(Instant::now());
            return true;
        }
        false
    }

    fn spawn_player(&mut self, file: &str) -> bool {
        #[cfg(target_os = "macos")]
        {
            let (prog, args) = afplay_command(file, self.volume);
            if let Ok(c) = Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn() {
                remember_pid(c.id());
                self.child = Some(c);
                self.paused = false;
                self.failure_told = false;
                self.spawned = Some(Instant::now());
                return true;
            }
        }
        let _ = (file, Stdio::null());
        false
    }

    fn signal(&self, sig: &str) {
        if let Some(c) = &self.child {
            let (prog, args) = signal_command(c.id(), sig);
            let _ = Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
    }

    /// True until a sound has been started.
    pub fn path_is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Length of the sound in milliseconds (0 when the Mac could not tell us).
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Where we are in the sound, in milliseconds.
    pub fn position_ms(&self) -> u64 {
        if let Some(n) = &self.native {
            let ms = (n.time_secs() * 1000.0).max(0.0) as u64;
            return if self.duration_ms > 0 { ms.min(self.duration_ms) } else { ms };
        }
        let raw = self.base_ms + self.since.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);
        if self.duration_ms > 0 { raw.min(self.duration_ms) } else { raw }
    }

    /// True when the Mac's in-process player is in use (no copy to cut, no late start, true position).
    pub fn is_native(&self) -> bool {
        self.native.is_some()
    }

    /// True about four times a second: time to compare the picture's position with the sound's.
    pub fn drift_check_due(&mut self) -> bool {
        let due = self.last_drift_check.map(|t| t.elapsed().as_millis() >= 250).unwrap_or(true);
        if due {
            self.last_drift_check = Some(Instant::now());
        }
        due
    }

    /// True when a jump has been asked for and the new copy is still being cut.
    pub fn is_seeking(&self) -> bool {
        self.seeking.is_some()
    }

    /// Ask for the picture to be held until the jump's sound is ready (see `take_video_release`).
    pub fn hold_video(&mut self) {
        self.video_hold = true;
    }

    /// True while a held picture is waiting (jump being prepared, or the sound player warming up).
    pub fn is_holding(&self) -> bool {
        self.video_hold
    }

    /// True once, when a held picture can run again: the jumped-to sound has been cut (or the jump failed) and nobody paused meanwhile.
    pub fn take_video_release(&mut self) -> bool {
        let waited = self.spawned.map(|t| t.elapsed().as_millis() as u64 >= self.latency_ms).unwrap_or(true);
        if self.video_hold && self.seeking.is_none() && waited {
            self.video_hold = false;
            return true;
        }
        false
    }

    /// Jump to `ms`. Playing stays playing, paused stays paused. AAC sound in an MP4 is cut by RhemaFlow itself;
    /// other sound needs ffmpeg (a message says so if it is missing).
    pub fn seek(&mut self, ms: u64) -> Result<(), String> {
        if self.path.is_empty() {
            return Err("There is no sound to jump in.".into());
        }
        if let Some(n) = &self.native {
            // instant: no copy to cut, no other program to start
            let target = jump_target(ms, 0, self.duration_ms);
            n.set_time_secs(target as f64 / 1000.0);
            self.base_ms = target;
            if self.since.is_some() {
                self.since = Some(Instant::now());
            }
            return Ok(());
        }
        let native = native_cuttable(&self.path);
        if !native && rf_media::convert::find_ffmpeg().is_none() {
            return Err(NO_SEEK_HINT.to_string());
        }
        let was_playing = self.is_playing();
        let target = jump_target(ms, 0, self.duration_ms);
        // stop what is playing (a frozen program must be woken before it can be killed)
        if let Some(mut c) = self.child.take() {
            if self.paused {
                self.signal_to(&c, "CONT");
            }
            let _ = c.kill();
            let _ = c.wait();
        }
        self.paused = false;
        self.spawned = None;
        if let Some(j) = self.seeking.take() {
            drop_job(j);
        }
        self.ready = None;
        let n = SEEK_COUNTER.fetch_add(1, Ordering::Relaxed);
        let input = self.path.clone();
        let cutter;
        let out;
        if native {
            out = std::env::temp_dir().join(format!("rhemaflow-seek-{}-{}.aac", std::process::id(), n));
            let (i, o) = (PathBuf::from(&input), out.clone());
            cutter = Cutter::Native(std::thread::spawn(move || rf_media::audiocut::cut_aac_adts(&i, target, &o)));
        } else {
            let (c, o) = self.start_ffmpeg(&input, target, n)?;
            cutter = Cutter::Ffmpeg(c);
            out = o;
        }
        self.base_ms = target;
        self.since = None;
        self.seeking = Some(SeekJob { cutter, out, resume: was_playing, input, target });
        Ok(())
    }

    fn start_ffmpeg(&self, input: &str, target: u64, n: u32) -> Result<(Child, PathBuf), String> {
        let ff = rf_media::convert::find_ffmpeg().ok_or_else(|| NO_SEEK_HINT.to_string())?;
        let out = std::env::temp_dir().join(format!("rhemaflow-seek-{}-{}f.{}", std::process::id(), n, seek_extension(input)));
        let (prog, args) = seek_command(&ff.to_string_lossy(), input, &out.to_string_lossy(), target);
        let child = Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not start ffmpeg to jump in the sound ({e})."))?;
        remember_pid(child.id());
        Ok((child, out))
    }

    fn signal_to(&self, c: &Child, sig: &str) {
        let (prog, args) = signal_command(c.id(), sig);
        let _ = Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }

    /// Call every frame. Starts the new copy once it has been cut. `Some(message)` if the jump failed.
    pub fn poll_seek(&mut self) -> Option<String> {
        let finished = match &mut self.seeking.as_mut()?.cutter {
            Cutter::Ffmpeg(c) => c.try_wait().ok().flatten().is_some(),
            Cutter::Native(h) => h.is_finished(),
        };
        if !finished {
            return None;
        }
        let job = self.seeking.take()?;
        let SeekJob { cutter, out, resume, input, target } = job;
        let ok = match cutter {
            Cutter::Ffmpeg(mut c) => c.wait().map(|s| s.success()).unwrap_or(false) && out.exists(),
            Cutter::Native(h) => {
                let good = matches!(h.join(), Ok(Ok(()))) && out.exists();
                if !good {
                    // the built-in cut could not read this sound: try ffmpeg
                    let _ = std::fs::remove_file(&out);
                    return match self.start_ffmpeg(&input, target, SEEK_COUNTER.fetch_add(1, Ordering::Relaxed)) {
                        Ok((c, o)) => {
                            self.seeking = Some(SeekJob { cutter: Cutter::Ffmpeg(c), out: o, resume, input, target });
                            None
                        }
                        Err(e) => Some(e),
                    };
                }
                true
            }
        };
        if !ok {
            let _ = std::fs::remove_file(&out);
            return Some("The sound could not jump to that time (the sound could not be read). Play still works from the start.".into());
        }
        if let Some(old) = self.temp.replace(out.clone()) {
            let _ = std::fs::remove_file(old);
        }
        if resume {
            let file = out.to_string_lossy().to_string();
            if self.spawn_player(&file) {
                self.since = Some(Instant::now());
            }
        } else {
            self.ready = Some(out);
        }
        None
    }

    pub fn pause(&mut self) {
        self.video_hold = false; // a pause during the jump keeps the picture still too
        if let Some(n) = &self.native {
            if !self.native_paused {
                self.base_ms = self.position_ms();
                self.since = None;
                n.pause();
                self.native_paused = true;
            }
            return;
        }
        if let Some(j) = self.seeking.as_mut() {
            j.resume = false; // the copy will be made, then wait for Play
        }
        if self.child.is_some() && !self.paused {
            self.base_ms = self.position_ms();
            self.since = None;
            self.signal("STOP");
            self.paused = true;
        }
    }

    pub fn resume(&mut self) {
        self.video_hold = false;
        if let Some(n) = &self.native {
            if self.native_paused {
                n.play();
                self.native_paused = false;
                self.since = Some(Instant::now());
            }
            return;
        }
        if let Some(j) = self.seeking.as_mut() {
            j.resume = true;
            return;
        }
        if let Some(r) = self.ready.take() {
            let file = r.to_string_lossy().to_string();
            if self.spawn_player(&file) {
                self.since = Some(Instant::now());
            }
            return;
        }
        if self.child.is_some() && self.paused {
            self.signal("CONT");
            self.paused = false;
            self.since = Some(Instant::now());
        }
    }

    pub fn stop(&mut self) {
        self.native = None; // dropping it stops it
        self.native_paused = false;
        self.native_started = false;
        if let Some(mut c) = self.child.take() {
            if self.paused {
                let (prog, args) = signal_command(c.id(), "CONT");
                let _ = Command::new(prog).args(args).status(); // a frozen program cannot be killed cleanly
            }
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(j) = self.seeking.take() {
            drop_job(j);
        }
        if let Some(t) = self.temp.take() {
            let _ = std::fs::remove_file(t);
        }
        self.ready = None;
        self.video_hold = false;
        self.paused = false;
        self.base_ms = 0;
        self.since = None;
    }

    /// `Some(message)` once, if the player program quit with an error (bad format, unreadable file).
    /// Call this every frame; it never blocks.
    pub fn poll_failure(&mut self, file: &str) -> Option<String> {
        if self.failure_told {
            return None;
        }
        let c = self.child.as_mut()?;
        match c.try_wait() {
            Ok(Some(status)) if !status.success() => {
                self.failure_told = true;
                let mut err = String::new();
                if let Some(mut e) = c.stderr.take() {
                    use std::io::Read;
                    let _ = e.by_ref().take(2048).read_to_string(&mut err);
                }
                let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
                Some(failure_text(name, &err, status.code()))
            }
            _ => None,
        }
    }

    /// True once the sound played to its end by itself.
    pub fn finished_ok(&mut self) -> bool {
        if let Some(n) = &self.native {
            return self.native_started && !self.native_paused && !n.is_playing();
        }
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(Some(s)) if s.success()),
            None => false,
        }
    }

    /// True while a sound is actually coming out (started, not finished, not paused).
    pub fn is_playing(&mut self) -> bool {
        if let Some(n) = &self.native {
            return !self.native_paused && n.is_playing();
        }
        if let Some(j) = &self.seeking {
            return j.resume;
        }
        match &mut self.child {
            Some(c) => match c.try_wait() {
                Ok(None) => !self.paused,
                _ => false,
            },
            None => false,
        }
    }
}

/// Asks the Mac's `afinfo` for the length (0 if it cannot say, or this is not a Mac).
fn probe_duration(path: &str) -> u64 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = Command::new("afinfo").arg(path).stdin(Stdio::null()).stderr(Stdio::null()).output() {
            return parse_afinfo_duration(&String::from_utf8_lossy(&o.stdout)).unwrap_or(0);
        }
    }
    let _ = path;
    0
}

// ---- v0.832.0: a force-quit (or crash) cannot run Drop, so the sound player (afplay / ffmpeg) it started kept
// playing: a 754 MB video's sound went on after the app was gone. Every player we start is written to a small
// file; the next start of RhemaFlow stops any of those that are still running.
fn pid_file() -> std::path::PathBuf {
    std::env::temp_dir().join("rhemaflow-players.pid")
}

fn remember_pid(pid: u32) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(pid_file()) {
        let _ = writeln!(f, "{pid}");
    }
}

/// Only ever stops a process whose program name is afplay or ffmpeg (so a reused number is never a different app).
pub fn kill_strays() {
    let Ok(text) = std::fs::read_to_string(pid_file()) else { return };
    for pid in text.lines().filter_map(|l| l.trim().parse::<u32>().ok()) {
        if pid == std::process::id() {
            continue;
        }
        if let Ok(o) = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).stdin(Stdio::null()).stderr(Stdio::null()).output() {
            let name = String::from_utf8_lossy(&o.stdout).to_lowercase();
            if name.contains("afplay") || name.contains("ffmpeg") {
                let _ = Command::new("kill").arg(pid.to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
            }
        }
    }
    let _ = std::fs::remove_file(pid_file());
}

impl Drop for Sound {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afplay_gets_the_file_and_a_safe_volume() {
        let (p, a) = afplay_command("/Users/me/My Films/baptism.mp4", 1.0);
        assert_eq!(p, "afplay");
        assert_eq!(a, vec!["-v", "1.00", "/Users/me/My Films/baptism.mp4"], "the path is ONE argument, so spaces are safe (no shell involved)");
        assert_eq!(afplay_command("x", 7.0).1[1], "1.00");
        assert_eq!(afplay_command("x", -1.0).1[1], "0.00");
    }

    #[test]
    fn a_failed_sound_is_explained_in_plain_words() {
        let m = failure_text("song.ogg", "Error: AudioFileOpen failed ('typ?')", Some(1));
        assert!(m.contains("song.ogg") && m.contains("convert"), "{m}");
        let m = failure_text("a.mp3", "", Some(9));
        assert!(m.contains("code 9"), "{m}");
        let m = failure_text("a.mp3", "No such file or directory", Some(1));
        assert!(m.contains("moved"), "{m}");
        // a player that never started has nothing to report
        let mut s = Sound::none();
        assert_eq!(s.poll_failure("x.mp3"), None);
        assert!(!s.finished_ok());
    }

    #[test]
    fn pausing_and_resuming_are_stop_and_continue_signals() {
        assert_eq!(signal_command(4242, "STOP"), ("kill".to_string(), vec!["-STOP".to_string(), "4242".to_string()]));
        assert_eq!(signal_command(7, "CONT").1, vec!["-CONT", "7"]);
    }

    #[test]
    fn a_sound_that_never_started_is_quiet_and_every_call_is_harmless() {
        let mut s = Sound::none();
        assert!(!s.is_playing());
        s.pause();
        s.resume();
        s.stop();
        // not a Mac (or no afplay): start says so instead of crashing
        #[cfg(not(target_os = "macos"))]
        assert!(!s.start("/nowhere.mp4", 1.0));
        assert!(!s.is_playing());
    }

    #[test]
    fn afinfo_length_is_read() {
        let t = "File:  a.mp3\nData format:  2 ch\nestimated duration: 215.032000 sec\nbit rate: 128000 bits per second\n";
        assert_eq!(parse_afinfo_duration(t), Some(215_032));
        assert_eq!(parse_afinfo_duration("nothing here"), None);
    }

    #[test]
    fn jumping_cuts_a_copy_from_the_right_time() {
        let (p, a) = seek_command("/opt/homebrew/bin/ffmpeg", "/m/My Song.mp3", "/tmp/o.mp3", 83_250);
        assert_eq!(p, "/opt/homebrew/bin/ffmpeg");
        let j = a.join(" ");
        assert!(j.contains("-ss 83.250 -i /m/My Song.mp3"), "{j}");
        assert!(j.contains("-c:a copy"), "mp3 is copied, not re-made: {j}");
        let (_, a) = seek_command("ffmpeg", "/m/talk.mp4", "/tmp/o.m4a", 5_000);
        assert!(a.join(" ").contains("-vn") && a.join(" ").contains("aac"));
    }

    #[test]
    fn a_jump_stays_inside_the_sound() {
        assert_eq!(jump_target(30_000, -10_000, 200_000), 20_000);
        assert_eq!(jump_target(5_000, -10_000, 200_000), 0);
        assert_eq!(jump_target(195_000, 10_000, 200_000), 199_500);
        assert_eq!(jump_target(1_000, 10_000, 0), 11_000, "length unknown: no end to stop at");
    }

    #[test]
    fn jumping_without_a_sound_is_a_plain_message_not_a_crash() {
        let mut s = Sound::none();
        assert!(s.seek(5_000).is_err());
        assert_eq!(s.poll_seek(), None);
        assert_eq!(s.position_ms(), 0);
        assert!(!s.is_seeking());
    }
}
