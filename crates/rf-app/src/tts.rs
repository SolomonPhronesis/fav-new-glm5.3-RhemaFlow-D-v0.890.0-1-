//! Natural text-to-speech for read-aloud (v0.801.2).
//!
//! Backend choice (senior-engineer decision record):
//! - **macOS `say`** is the production backend. Modern macOS ships with
//!   *Premium* and *Enhanced* neural voices (Ava, Zoe, Tom, Daniel, ...)
//!   that are natural, NOT robotic, work fully offline after a one-time
//!   free download (System Settings → Accessibility → Spoken Content →
//!   System Voice → Manage Voices…), and cost zero latency to integrate.
//!   This answers "natural voice, not a robot" without shipping 300 MB of
//!   weights or reverse-engineering anyone's cloud API.
//! - **Azure/Edge neural voices**: Microsoft's Edge "Read Aloud" voices are
//!   Azure Speech voices served under Microsoft's terms; scraping them via
//!   the browser endpoint violates those terms. The *legitimate* path is a
//!   free Azure Speech API key (500k chars/month) — planned as an optional
//!   v0.9 backend behind a user-supplied key. Documented, not implemented,
//!   on purpose.
//! - **espeak-ng** is the dev/Linux fallback so the module is testable off
//!   macOS; quality is robotic, which is exactly why it is not the target.

use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Gender {
    Female,
    Male,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Voice {
    /// The exact string `say -v` expects (e.g. "Ava (Premium)").
    pub id: String,
    /// Display name without the quality suffix (e.g. "Ava").
    pub name: String,
    /// "Premium" / "Enhanced" / "" — Premium/Enhanced are the natural ones.
    pub quality: String,
    pub locale: String,
    pub gender: Gender,
}

impl Voice {
    pub fn is_natural(&self) -> bool {
        self.quality == "Premium" || self.quality == "Enhanced"
    }
    pub fn label(&self) -> String {
        let q = if self.quality.is_empty() {
            String::new()
        } else {
            format!(" · {}", self.quality)
        };
        let g = match self.gender {
            Gender::Female => " (female)",
            Gender::Male => " (male)",
            Gender::Unknown => "",
        };
        format!("{}{}{}", self.name, q, g)
    }
}

#[allow(dead_code)] // macOS-only classification path; still exercised on macOS builds
/// Voices known to be female/male on macOS. `say -v ?` does not expose
/// gender, so a curated list is the honest, zero-dependency answer; any
/// unknown voice is classified Unknown and still selectable.
const FEMALE: &[&str] = &[
    "ava", "zoe", "samantha", "fiona", "victoria", "allison", "susan", "kathy",
    "nicky", "joana", "amelie", "anna", "carola", "alice", "yuna", "kyoko",
    "mei-jia", "sin-ji", "ting-ting", "milena", "paula", "luciana", "zosia",
    "alva", "sara", "nora", "kanya", "damayanti", "ellen", "ioana",
    "katya", "tunde", "martha", "laila", "marie", "lezane", "sandy",
    "shelley", "linda",
];
const MALE: &[&str] = &[
    "tom", "daniel", "alex", "fred", "oliver", "gordon", "ralph", "jorge",
    "diego", "xander", "rishi", "aaron", "arthur", "nathan", "reed", "rocko",
    "grandpa", "eddy", "jacques", "thomas", "yannick", "luca", "otoya",
    "hattori", "yu-shu", "william", "maged", "tarik", "nicolas",
];

fn classify_gender(name: &str) -> Gender {
    let n = name.to_ascii_lowercase();
    // strip quality suffix for matching ("ava (premium)" -> "ava")
    let base: String = n
        .split(|c: char| c == '(' || c == ')' || c == ' ' || c == '-')
        .next()
        .unwrap_or(&n)
        .to_string();
    if FEMALE.contains(&base.as_str()) {
        return Gender::Female;
    }
    if MALE.contains(&base.as_str()) {
        return Gender::Male;
    }
    Gender::Unknown
}

#[allow(dead_code)]
/// Parse one line of `say -v ?` output:
/// `Ava (Premium)              en_US    # Hello, my name is Ava.`
pub fn parse_say_line(line: &str) -> Option<Voice> {
    let (left, _sample) = line.split_once('#')?;
    let left = left.trim_end();
    let mut parts = left.split_whitespace().rev();
    let locale = parts.next()?.to_string();
    // everything before the locale token is the voice id (may contain spaces)
    let id_left = left.len() - locale.len();
    let id = left[..id_left].trim_end().to_string();
    if id.is_empty() {
        return None;
    }
    let (name, quality) = match id.split_once(" (") {
        Some((n, q)) => (n.to_string(), q.trim_end_matches(')').to_string()),
        None => (id.clone(), String::new()),
    };
    Some(Voice {
        gender: classify_gender(&name),
        id,
        name,
        quality,
        locale,
    })
}

/// Enumerate installed voices. macOS: `say -v ?`. Linux dev boxes:
/// `espeak-ng --voices` (quality Unknown, gender Unknown).
pub fn list_voices() -> Vec<Voice> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("say").arg("-v").arg("?").output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut voices: Vec<Voice> = text.lines().filter_map(parse_say_line).collect();
            voices.sort_by(|a, b| {
                b.is_natural()
                    .cmp(&a.is_natural())
                    .then_with(|| a.name.cmp(&b.name))
            });
            return voices;
        }
        Vec::new()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut out = Vec::new();
        for cmd in [["espeak-ng", "--voices"], ["espeak", "--voices"]] {
            if let Ok(o) = Command::new(cmd[0]).arg(cmd[1]).output() {
                let text = String::from_utf8_lossy(&o.stdout);
                for line in text.lines().skip(1) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    if cols.len() >= 4 {
                        out.push(Voice {
                            id: cols[1].to_string(),
                            name: cols[3].to_string(),
                            quality: String::new(),
                            locale: cols[0].trim_matches(|c| c == '(' || c == ')').to_string(),
                            gender: Gender::Unknown,
                        });
                    }
                }
                if !out.is_empty() {
                    break;
                }
            }
        }
        out
    }
}

/// A preferred default: the most natural female voice, else the most
/// natural male voice, else whatever exists.
pub fn default_voice(voices: &[Voice]) -> Option<String> {
    voices
        .iter()
        .find(|v| v.is_natural() && v.gender == Gender::Female)
        .or_else(|| voices.iter().find(|v| v.is_natural()))
        .map(|v| v.id.clone())
}

/// Speak control: spawns the platform TTS process and can stop it.
pub struct Speaker {
    voice: Option<String>,
    rate: u32,
    child: Option<Child>,
    speaking: Arc<AtomicBool>,
    /// when the last utterance ended (finished or was stopped)
    ended_at: Option<Instant>,
}

/// How long after the voice stops the microphone may still be hearing its echo.
pub const ECHO_TAIL: Duration = Duration::from_millis(1500);

impl Speaker {
    pub fn new(voice: Option<String>, rate: u32) -> Self {
        Speaker {
            voice,
            rate: rate.clamp(120, 260),
            child: None,
            speaking: Arc::new(AtomicBool::new(false)),
            ended_at: None,
        }
    }

    pub fn set_voice(&mut self, voice: Option<String>) {
        self.voice = voice;
    }

    pub fn set_rate(&mut self, rate: u32) {
        self.rate = rate.clamp(120, 260);
    }

    pub fn speaking(&self) -> bool {
        self.speaking.load(Ordering::Relaxed)
    }

    /// Call every frame. BUG (fixed): `speaking` used to be set when the voice
    /// started and only cleared by `stop()`, so after a verse finished on its
    /// own the app believed it was still reading forever (the Stop button never
    /// went away and "is it reading?" was always yes). This reaps the finished
    /// `say` process and records when it ended.
    pub fn poll(&mut self) {
        let finished = match self.child.as_mut() {
            Some(c) => !matches!(c.try_wait(), Ok(None)),
            None => false,
        };
        if finished {
            self.child = None;
            self.speaking.store(false, Ordering::Relaxed);
            self.ended_at = Some(Instant::now());
        }
    }

    /// True while the voice is talking and for a moment after: the microphone can
    /// hear the speakers, so text heard now may be our own voice echoing back.
    pub fn echo_active(&self) -> bool {
        self.speaking() || self.ended_at.map_or(false, |t| t.elapsed() < ECHO_TAIL)
    }

    /// Speak `text` (non-blocking; replaces any current utterance).
    pub fn speak(&mut self, text: &str) -> Result<(), String> {
        self.stop();
        if text.trim().is_empty() {
            return Ok(());
        }
        let speaking = self.speaking.clone();
        #[cfg(target_os = "macos")]
        {
            let mut cmd = Command::new("say");
            if let Some(v) = &self.voice {
                cmd.arg("-v").arg(v);
            }
            cmd.arg("-r").arg(self.rate.to_string()).arg(text);
            match cmd.spawn() {
                Ok(child) => {
                    speaking.store(true, Ordering::Relaxed);
                    self.child = Some(child);
                    Ok(())
                }
                Err(e) => Err(format!("cannot start 'say': {e}")),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut ok = false;
            for cmd_name in ["espeak-ng", "espeak"] {
                let mut cmd = Command::new(cmd_name);
                if let Some(v) = &self.voice {
                    cmd.arg("-v").arg(v);
                }
                cmd.arg("-s").arg(self.rate.to_string()).arg(text);
                if let Ok(child) = cmd.spawn() {
                    speaking.store(true, Ordering::Relaxed);
                    self.child = Some(child);
                    ok = true;
                    break;
                }
            }
            if ok {
                Ok(())
            } else {
                Err("no TTS backend found (tried espeak-ng/espeak)".into())
            }
        }
    }

    /// Stop the current utterance. The `say` process is killed; macOS
    /// reaps it automatically.
    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.speaking.store(false, Ordering::Relaxed);
        self.ended_at = Some(Instant::now());
    }

    #[cfg(test)]
    fn attach_for_test(&mut self, child: Child) {
        self.speaking.store(true, Ordering::Relaxed);
        self.child = Some(child);
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_say_line_premium() {
        let v = parse_say_line(
            "Ava (Premium)              en_US    # Hello, my name is Ava.",
        )
        .expect("parse");
        assert_eq!(v.id, "Ava (Premium)");
        assert_eq!(v.name, "Ava");
        assert_eq!(v.quality, "Premium");
        assert_eq!(v.locale, "en_US");
        assert_eq!(v.gender, Gender::Female);
        assert!(v.is_natural());
    }

    #[test]
    fn parses_say_line_plain() {
        let v = parse_say_line("Tom                 en_US    # Hello, my name is Tom.")
            .expect("parse");
        assert_eq!(v.id, "Tom");
        assert_eq!(v.name, "Tom");
        assert_eq!(v.quality, "");
        assert_eq!(v.gender, Gender::Male);
    }

    #[cfg(unix)]
    #[test]
    fn speaking_clears_by_itself_when_the_voice_finishes() {
        let mut sp = Speaker::new(None, 175);
        sp.attach_for_test(Command::new("true").spawn().unwrap());
        assert!(sp.speaking());
        for _ in 0..50 {
            sp.poll();
            if !sp.speaking() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!sp.speaking(), "a finished voice must not stay 'speaking'");
        assert!(sp.echo_active(), "but its echo may still be in the mic for a moment");
    }

    #[cfg(unix)]
    #[test]
    fn stop_kills_a_running_voice_at_once() {
        let mut sp = Speaker::new(None, 175);
        sp.attach_for_test(Command::new("sleep").arg("30").spawn().unwrap());
        assert!(sp.speaking());
        let t0 = Instant::now();
        sp.stop();
        assert!(!sp.speaking());
        assert!(t0.elapsed() < Duration::from_secs(2), "stop must be instant");
        sp.poll(); // nothing left to reap, must not panic
    }

    #[test]
    fn garbage_rejected() {
        assert!(parse_say_line("no hash here").is_none());
        assert!(parse_say_line("").is_none());
    }

    #[test]
    fn default_prefers_natural_female() {
        let voices = vec![
            Voice { id: "Fred".into(), name: "Fred".into(), quality: String::new(), locale: "en_US".into(), gender: Gender::Male },
            Voice { id: "Zoe (Premium)".into(), name: "Zoe".into(), quality: "Premium".into(), locale: "en_US".into(), gender: Gender::Female },
        ];
        assert_eq!(default_voice(&voices).as_deref(), Some("Zoe (Premium)"));
    }
}
