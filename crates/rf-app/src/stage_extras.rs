//! v0.833.0: the text rules for live captions and the preacher's monitor (the drawing is in `ui.rs` / `app.rs`).

/// A caption stays up for this long after the words were heard.
pub const CAPTION_LIFE_MS: u64 = 8_000;
/// Long speech is cut from the front so the newest words are what people read.
const CAPTION_MAX_CHARS: usize = 120;

/// The words to show now, or "" when they are too old.
pub fn caption_text(text: &str, said_at_ms: u64, now_ms: u64) -> String {
    if now_ms.saturating_sub(said_at_ms) > CAPTION_LIFE_MS {
        return String::new();
    }
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = t.chars().count();
    if n <= CAPTION_MAX_CHARS {
        return t;
    }
    // keep the end, starting on a word
    let tail: String = t.chars().skip(n - CAPTION_MAX_CHARS).collect();
    match tail.split_once(' ') {
        Some((_, rest)) => format!("\u{2026}{rest}"),
        None => format!("\u{2026}{tail}"),
    }
}

pub struct MonitorLines {
    pub clock: String,
    pub elapsed: String,
    pub now_label: String,
    pub now_text: String,
    pub next: String,
}

pub fn monitor_lines(clock_hms: &str, service_ms: Option<u64>, label: &str, text: &str, next: Option<&str>) -> MonitorLines {
    let clock = clock_hms.get(..5).unwrap_or(clock_hms).to_string(); // hh:mm
    let elapsed = match service_ms {
        Some(ms) => {
            let s = ms / 1000;
            format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
        }
        None => "not started".to_string(),
    };
    let short: String = {
        let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.chars().count() > 160 {
            format!("{}\u{2026}", t.chars().take(160).collect::<String>())
        } else {
            t
        }
    };
    MonitorLines {
        clock,
        elapsed,
        now_label: if label.trim().is_empty() { "(nothing)".into() } else { label.to_string() },
        now_text: short,
        next: next.map(str::to_string).unwrap_or_else(|| "(nothing planned)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caption_fades_out_after_a_few_seconds() {
        assert_eq!(caption_text("  turn to  John three  ", 10_000, 12_000), "turn to John three");
        assert_eq!(caption_text("old words", 10_000, 10_000 + CAPTION_LIFE_MS + 1), "");
    }

    #[test]
    fn long_speech_keeps_the_newest_words_and_starts_on_a_word() {
        let long = (1..=60).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
        let c = caption_text(&long, 0, 0);
        assert!(c.starts_with('\u{2026}'));
        assert!(c.ends_with("word60"));
        assert!(c.chars().count() <= CAPTION_MAX_CHARS + 1);
        assert!(!c.contains("word1 "), "the oldest words are dropped");
    }

    #[test]
    fn the_monitor_shows_clock_service_time_and_what_is_next() {
        let m = monitor_lines("10:31:07", Some(3_725_000), "John 3:16", "For God so loved the world", Some("Romans 8:28-30"));
        assert_eq!((m.clock.as_str(), m.elapsed.as_str()), ("10:31", "01:02:05"));
        assert_eq!(m.next, "Romans 8:28-30");
        let n = monitor_lines("09:00:00", None, "", "", None);
        assert_eq!((n.elapsed.as_str(), n.now_label.as_str(), n.next.as_str()), ("not started", "(nothing)", "(nothing planned)"));
    }
}
