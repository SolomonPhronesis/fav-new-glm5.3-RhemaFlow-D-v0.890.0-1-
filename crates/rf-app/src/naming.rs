//! Local date/time, file and folder names, and the detailed transcript / sermon
//! notes text (v0.812.0).
//!
//! The old stamp used UTC, so a service at 01:27 in the UK in summer was filed an
//! hour early. Everything the operator sees now uses the Mac's LOCAL clock.
//!
//!   <save root>/Sunday 4th October 2026/                         (the folder = the full date)
//!       Sunday 4th October 2026 - 01h27m43s to 02h15m10s - recording.wav
//!       Sunday 4th October 2026 - 01h27m43s to 02h15m10s - transcript.txt  (.srt)
//!       Sunday 4th October 2026 - 01h27m43s to 02h15m10s - sermon notes.md
//!       Sunday 4th October 2026 - 01h27m43s to 02h15m10s - events.jsonl
//!
//! v0.833.0: EVERY service gets its own folder inside the day folder, named by its start time
//! (<save root>/Sunday 4th October 2026/01h27m43s - Faith/...), so three services on one day are three
//! folders, each holding only its own recording, transcript, notes and events. While a service is running the files say
//! "... - start 01h27m43s - ..." (the end is not known yet) and are renamed when it ends.

use crate::export::{hms, safe_name, Cite, Formats, Line};
use std::path::Path;

const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];

/// A wall-clock moment to the second, plus its offset from UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalStamp {
    pub y: i32,
    pub mo: u32,
    pub d: u32,
    pub h: u32,
    pub mi: u32,
    pub s: u32,
    pub utc_offset_min: i32,
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    // Howard Hinnant's algorithm
    let y = (if m <= 2 { y - 1 } else { y }) as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (if m > 2 { m - 3 } else { m + 9 }) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "31st".
pub fn ordinal(d: u32) -> String {
    let suffix = match d % 100 {
        11..=13 => "th",
        _ => match d % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{d}{suffix}")
}

impl LocalStamp {
    pub fn from_unix_utc(t: i64) -> Self {
        Self::from_local_secs(t, 0)
    }

    /// `local_secs` = seconds since 1970-01-01 00:00:00 on the LOCAL wall clock.
    pub fn from_local_secs(local_secs: i64, utc_offset_min: i32) -> Self {
        let days = local_secs.div_euclid(86_400);
        let tod = local_secs.rem_euclid(86_400);
        let (y, mo, d) = civil_from_days(days);
        LocalStamp { y, mo, d, h: (tod / 3600) as u32, mi: ((tod % 3600) / 60) as u32, s: (tod % 60) as u32, utc_offset_min }
    }

    pub fn local_secs(&self) -> i64 {
        days_from_civil(self.y, self.mo, self.d) * 86_400 + self.h as i64 * 3600 + self.mi as i64 * 60 + self.s as i64
    }

    /// This moment plus `ms` milliseconds (rolls over midnight correctly).
    pub fn plus_ms(&self, ms: u64) -> LocalStamp {
        Self::from_local_secs(self.local_secs() + (ms / 1000) as i64, self.utc_offset_min)
    }

    /// 0 = Monday .. 6 = Sunday
    pub fn weekday_index(&self) -> usize {
        (days_from_civil(self.y, self.mo, self.d) + 3).rem_euclid(7) as usize
    }
    pub fn weekday_name(&self) -> &'static str {
        WEEKDAYS[self.weekday_index()]
    }
    pub fn month_name(&self) -> &'static str {
        MONTHS[(self.mo.clamp(1, 12) - 1) as usize]
    }
    /// "Sunday 4th October 2026"
    pub fn long_date(&self) -> String {
        format!("{} {} {} {}", self.weekday_name(), ordinal(self.d), self.month_name(), self.y)
    }
    /// "01:27:43"
    pub fn time_hms(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.h, self.mi, self.s)
    }
    /// "1:27:43 AM"
    pub fn time_12h(&self) -> String {
        let (h12, ampm) = match self.h {
            0 => (12, "AM"),
            1..=11 => (self.h, "AM"),
            12 => (12, "PM"),
            _ => (self.h - 12, "PM"),
        };
        format!("{h12}:{:02}:{:02} {ampm}", self.mi, self.s)
    }
    /// file-name safe: "01h27m43s"
    pub fn time_file(&self) -> String {
        format!("{:02}h{:02}m{:02}s", self.h, self.mi, self.s)
    }
    /// "UTC+01:00"
    pub fn utc_label(&self) -> String {
        let o = self.utc_offset_min;
        format!("UTC{}{:02}:{:02}", if o < 0 { '-' } else { '+' }, o.abs() / 60, o.abs() % 60)
    }
    pub fn same_day(&self, other: &LocalStamp) -> bool {
        (self.y, self.mo, self.d) == (other.y, other.mo, other.d)
    }
}

/// Parse `date "+%Y %m %d %H %M %S %z %Z"` output, e.g. `2026 10 04 01 27 43 +0100 BST`.
pub fn parse_date_output(out: &str) -> Option<(LocalStamp, String)> {
    let p: Vec<&str> = out.split_whitespace().collect();
    if p.len() < 7 {
        return None;
    }
    let n = |i: usize| p[i].parse::<u32>().ok();
    let z = p[6];
    let sign = if z.starts_with('-') { -1 } else { 1 };
    let digits: String = z.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let off = sign * (digits[0..2].parse::<i32>().ok()? * 60 + digits[2..4].parse::<i32>().ok()?);
    let st = LocalStamp { y: p[0].parse().ok()?, mo: n(1)?, d: n(2)?, h: n(3)?, mi: n(4)?, s: n(5)?, utc_offset_min: off };
    if !(1..=12).contains(&st.mo) || !(1..=31).contains(&st.d) || st.h > 23 || st.mi > 59 || st.s > 60 {
        return None;
    }
    Some((st, p.get(7).map(|s| s.to_string()).unwrap_or_default()))
}

/// The Mac's local date and time (and its zone name, e.g. "BST"). Falls back to UTC.
pub fn now_local() -> (LocalStamp, String) {
    if let Ok(o) = std::process::Command::new("date").arg("+%Y %m %d %H %M %S %z %Z").output() {
        if let Some(r) = parse_date_output(&String::from_utf8_lossy(&o.stdout)) {
            return r;
        }
    }
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    (LocalStamp::from_unix_utc(t), "UTC".to_string())
}

// ---------------------------------------------------------------------------
// names
// ---------------------------------------------------------------------------

/// The folder for a day: just the full date.
pub fn day_folder_name(start: &LocalStamp) -> String {
    start.long_date()
}

/// One folder per service, inside the day folder: "01h27m43s - Faith" (the start time never changes, so the
/// files can be written into it while the service runs).
pub fn service_folder_name(start: &LocalStamp, title: &str) -> String {
    format!("{}{}", start.time_file(), title_part(title))
}

fn title_part(title: &str) -> String {
    let t = safe_name(title);
    if t.is_empty() {
        String::new()
    } else {
        format!(" - {t}")
    }
}

/// While the service is running: "Sunday 4th October 2026 - Faith - start 01h27m43s".
pub fn live_base(start: &LocalStamp, title: &str) -> String {
    format!("{}{} - start {}", start.long_date(), title_part(title), start.time_file())
}

/// When it is over: "Sunday 4th October 2026 - Faith - 01h27m43s to 02h15m10s".
/// If it ran past midnight the end carries its own date.
pub fn final_base(start: &LocalStamp, end: &LocalStamp, title: &str) -> String {
    let end_part = if start.same_day(end) {
        end.time_file()
    } else {
        format!("{} {}", end.long_date(), end.time_file())
    };
    format!("{}{} - {} to {}", start.long_date(), title_part(title), start.time_file(), end_part)
}

/// `recording.wav` for the first stretch of audio, `recording (part 2).wav` after a Stop/Record.
pub fn recording_name(base: &str, part: u32) -> String {
    if part <= 1 {
        format!("{base} - recording.wav")
    } else {
        format!("{base} - recording (part {part}).wav")
    }
}

pub fn file_name(base: &str, what: &str) -> String {
    format!("{base} - {what}")
}

/// "47 min 27 s", "1 h 05 min 09 s", "38 s"
pub fn human_duration(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, sec) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h} h {m:02} min {sec:02} s")
    } else if m > 0 {
        format!("{m} min {sec:02} s")
    } else {
        format!("{sec} s")
    }
}

// ---------------------------------------------------------------------------
// the detailed documents
// ---------------------------------------------------------------------------

/// Everything the transcript header and sermon notes say about the service.
#[derive(Clone, Debug)]
pub struct ServiceMeta {
    pub title: String,
    pub church: String,
    pub app_version: String,
    pub start: LocalStamp,
    pub end: LocalStamp,
    pub tz_name: String,
    pub duration_ms: u64,
    pub translation: String,
    pub engine: String,
    pub mic: String,
    pub folder: String,
    pub recording_files: Vec<String>,
    pub auto_staged: u32,
    pub approved: u32,
    pub dismissed: u32,
}

impl ServiceMeta {
    fn start_line(&self) -> String {
        format!("{} ({}) {} {}", self.start.time_hms(), self.start.time_12h(), self.start.utc_label(), self.tz_name).trim_end().to_string()
    }
    fn end_line(&self) -> String {
        let day = if self.start.same_day(&self.end) { String::new() } else { format!("{}, ", self.end.long_date()) };
        format!("{day}{} ({}) {} {}", self.end.time_hms(), self.end.time_12h(), self.end.utc_label(), self.tz_name).trim_end().to_string()
    }
}

/// How a scripture reached the stage, in words.
pub fn source_label(src: &str) -> &'static str {
    match src {
        "voice" | "voice:parallel" => "spoken reference",
        s if s.starts_with("quotation") => "quoted words",
        "confirm" => "review card",
        "nav" => "next/previous",
        "manual" | "browser" => "clicked by hand",
        "history" => "previous/next scripture",
        "plan" => "sermon plan",
        "again" => "shown again",
        _ => "other",
    }
}

fn count_by_source(cites: &[Cite]) -> Vec<(&'static str, usize)> {
    let mut v: Vec<(&'static str, usize)> = Vec::new();
    for c in cites {
        let l = source_label(&c.source);
        match v.iter_mut().find(|(k, _)| *k == l) {
            Some((_, n)) => *n += 1,
            None => v.push((l, 1)),
        }
    }
    v
}

fn word_count(lines: &[Line]) -> usize {
    lines.iter().map(|l| l.text.split_whitespace().count()).sum()
}

/// Largest silence between transcript lines: (gap ms, where it started ms).
fn longest_gap(lines: &[Line]) -> Option<(u64, u64)> {
    lines.windows(2).map(|w| (w[1].ms.saturating_sub(w[0].ms), w[0].ms)).max_by_key(|g| g.0)
}

/// Quiet stretches at least this long get a marker line in the transcript.
pub const GAP_MARK_MS: u64 = 30_000;

/// The first lines of a transcript file while the service is still running.
pub fn live_header(start: &LocalStamp, tz_name: &str, title: &str, church: &str, version: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("{}\n", start.long_date()));
    s.push_str("RHEMAFLOW-D SERVICE TRANSCRIPT (saved live while the service runs)\n");
    s.push_str("====================================================================\n");
    s.push_str(&format!("Day and date : {}\n", start.long_date()));
    s.push_str(&format!("Start time   : {} ({}) {} {}\n", start.time_hms(), start.time_12h(), start.utc_label(), tz_name));
    if !title.trim().is_empty() {
        s.push_str(&format!("Title        : {}\n", title.trim()));
    }
    if !church.trim().is_empty() {
        s.push_str(&format!("Church       : {}\n", church.trim()));
    }
    s.push_str(&format!("Software     : {version}\n"));
    s.push_str("Each line:  [time into the service | clock time]  words heard\n");
    s.push_str("--------------------------------------------------------------------\n");
    s
}

/// Line prefix used both live and in the final file: `[00:01:03 | 01:28:46]`.
pub fn line_prefix(start: &LocalStamp, ms: u64) -> String {
    format!("[{} | {}]", hms(ms), start.plus_ms(ms).time_hms())
}

/// The complete, final plain-text transcript with the full detailed header.
pub fn to_txt(meta: &ServiceMeta, lines: &[Line], cites: &[Cite]) -> String {
    let mut s = String::new();
    s.push_str(&format!("{}\n", meta.start.long_date()));
    s.push_str("RHEMAFLOW-D SERVICE TRANSCRIPT\n");
    s.push_str("====================================================================\n");
    s.push_str(&format!("Day and date  : {}\n", meta.start.long_date()));
    s.push_str(&format!("Start time    : {}\n", meta.start_line()));
    s.push_str(&format!("End time      : {}\n", meta.end_line()));
    s.push_str(&format!("Length        : {} ({})\n", human_duration(meta.duration_ms), hms(meta.duration_ms)));
    if !meta.title.trim().is_empty() {
        s.push_str(&format!("Title         : {}\n", meta.title.trim()));
    }
    if !meta.church.trim().is_empty() {
        s.push_str(&format!("Church        : {}\n", meta.church.trim()));
    }
    s.push_str(&format!("Software      : {}\n", meta.app_version));
    s.push_str(&format!("Speech engine : {}\n", meta.engine));
    s.push_str(&format!("Microphone    : {}\n", meta.mic));
    s.push_str(&format!("Bible shown   : {}\n", meta.translation));
    s.push_str(&format!("Saved in      : {}\n", meta.folder));
    for f in &meta.recording_files {
        s.push_str(&format!("Recording     : {f}\n"));
    }
    s.push_str("\nTOTALS\n------\n");
    s.push_str(&format!("Transcript lines : {}\n", lines.len()));
    s.push_str(&format!("Words heard      : {}\n", word_count(lines)));
    s.push_str(&format!("Scriptures shown : {}\n", cites.len()));
    for (k, n) in count_by_source(cites) {
        s.push_str(&format!("   {k:<18}: {n}\n"));
    }
    s.push_str(&format!(
        "Staged automatically: {}    Review cards approved: {}    dismissed: {}\n",
        meta.auto_staged, meta.approved, meta.dismissed
    ));
    if let Some((gap, at)) = longest_gap(lines) {
        if gap >= GAP_MARK_MS {
            s.push_str(&format!(
                "Longest quiet stretch: {} starting {} into the service ({})\n",
                human_duration(gap),
                hms(at),
                meta.start.plus_ms(at).time_hms()
            ));
        }
    }
    s.push_str("\nSCRIPTURE INDEX  [time into the service | clock time]\n----------------------------------------------------\n");
    if cites.is_empty() {
        s.push_str("(no Scripture was shown)\n");
    }
    for c in cites {
        s.push_str(&format!("{}  {}  ({})\n", line_prefix(&meta.start, c.ms), c.label, source_label(&c.source)));
        if !c.text.trim().is_empty() {
            s.push_str(&format!("      {}\n", c.text.trim().replace('\n', " ")));
        }
    }
    s.push_str("\nTRANSCRIPT  [time into the service | clock time]\n----------------------------------------------------\n");
    let mut prev: Option<u64> = None;
    for l in lines {
        if let Some(p) = prev {
            let gap = l.ms.saturating_sub(p);
            if gap >= GAP_MARK_MS {
                s.push_str(&format!(
                    "            ... quiet for {} ({} to {}) ...\n",
                    human_duration(gap),
                    meta.start.plus_ms(p).time_hms(),
                    meta.start.plus_ms(l.ms).time_hms()
                ));
            }
        }
        s.push_str(&format!("{} {}\n", line_prefix(&meta.start, l.ms), l.text.trim()));
        prev = Some(l.ms);
    }
    s.push_str(&format!("\n--- end of transcript: {} at {} ---\n", meta.end.long_date(), meta.end.time_hms()));
    s
}

/// Sermon notes in Markdown: a details table, the scripture index with verse text, the transcript.
pub fn to_markdown(meta: &ServiceMeta, lines: &[Line], cites: &[Cite]) -> String {
    let esc = |t: &str| t.replace('|', "/").replace('\n', " ");
    let mut m = String::new();
    let t = if meta.title.trim().is_empty() { "Service" } else { meta.title.trim() };
    m.push_str(&format!("# {} \u{2014} {}\n\n", meta.start.long_date(), t));
    if !meta.church.trim().is_empty() {
        m.push_str(&format!("**{}**\n\n", meta.church.trim()));
    }
    m.push_str("## Service details\n\n| | |\n|---|---|\n");
    m.push_str(&format!("| Day and date | {} |\n", meta.start.long_date()));
    m.push_str(&format!("| Started | {} |\n", meta.start_line()));
    m.push_str(&format!("| Ended | {} |\n", meta.end_line()));
    m.push_str(&format!("| Length | {} ({}) |\n", human_duration(meta.duration_ms), hms(meta.duration_ms)));
    m.push_str(&format!("| Speech engine | {} |\n", esc(&meta.engine)));
    m.push_str(&format!("| Microphone | {} |\n", esc(&meta.mic)));
    m.push_str(&format!("| Bible shown | {} |\n", meta.translation));
    m.push_str(&format!("| Software | {} |\n", meta.app_version));
    m.push_str(&format!("| Saved in | `{}` |\n", meta.folder));
    for f in &meta.recording_files {
        m.push_str(&format!("| Recording | `{}` |\n", esc(f)));
    }
    m.push_str(&format!("| Transcript lines / words | {} / {} |\n", lines.len(), word_count(lines)));
    m.push_str(&format!("| Scriptures shown | {} |\n", cites.len()));
    for (k, n) in count_by_source(cites) {
        m.push_str(&format!("| \u{00a0}\u{00a0}\u{00a0}{k} | {n} |\n"));
    }
    m.push_str(&format!(
        "| Staged automatically | {} |\n| Review cards approved / dismissed | {} / {} |\n\n",
        meta.auto_staged, meta.approved, meta.dismissed
    ));
    m.push_str("## Scripture index\n\n");
    if cites.is_empty() {
        m.push_str("_No Scripture was shown during this service._\n\n");
    } else {
        m.push_str("| # | Into service | Clock | Scripture | How | Text |\n|---|---|---|---|---|---|\n");
        for (i, c) in cites.iter().enumerate() {
            m.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                i + 1,
                hms(c.ms),
                meta.start.plus_ms(c.ms).time_hms(),
                esc(&c.label),
                source_label(&c.source),
                esc(c.text.trim())
            ));
        }
        m.push('\n');
    }
    m.push_str("## Transcript\n\n");
    let mut prev: Option<u64> = None;
    for l in lines {
        if let Some(p) = prev {
            let gap = l.ms.saturating_sub(p);
            if gap >= GAP_MARK_MS {
                m.push_str(&format!("_\u{2026} quiet for {} \u{2026}_\n\n", human_duration(gap)));
            }
        }
        m.push_str(&format!("**{}** {}\n\n", line_prefix(&meta.start, l.ms), l.text.trim()));
        prev = Some(l.ms);
    }
    m.push_str(&format!("---\n_End of transcript \u{2014} {} at {}._\n", meta.end.long_date(), meta.end.time_hms()));
    m
}

/// Write the transcript files for a finished service into `dir`, named from `base`.
/// Returns the file names written. `.txt` is always written (it replaces the live copy).
pub fn write_transcripts(dir: &Path, base: &str, formats: Formats, meta: &ServiceMeta, lines: &[Line], cites: &[Cite]) -> Vec<String> {
    let mut written = Vec::new();
    let mut put = |name: String, body: String| {
        if std::fs::write(dir.join(&name), body).is_ok() {
            written.push(name);
        }
    };
    put(file_name(base, "transcript.txt"), to_txt(meta, lines, cites));
    if formats.srt {
        put(file_name(base, "transcript.srt"), crate::export::to_srt(lines, meta.duration_ms));
    }
    if formats.md {
        put(file_name(base, "sermon notes.md"), to_markdown(meta, lines, cites));
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start() -> LocalStamp {
        LocalStamp { y: 2026, mo: 10, d: 4, h: 1, mi: 27, s: 43, utc_offset_min: 60 }
    }

    #[test]
    fn the_users_own_example_date_is_a_sunday() {
        assert_eq!(start().long_date(), "Sunday 4th October 2026");
        assert_eq!(start().time_hms(), "01:27:43");
        assert_eq!(start().time_12h(), "1:27:43 AM");
        assert_eq!(start().time_file(), "01h27m43s");
        assert_eq!(start().utc_label(), "UTC+01:00");
    }

    #[test]
    fn weekdays_across_known_dates() {
        let d = |y, mo, d| LocalStamp { y, mo, d, h: 0, mi: 0, s: 0, utc_offset_min: 0 }.weekday_name();
        assert_eq!(d(1970, 1, 1), "Thursday");
        assert_eq!(d(2000, 1, 1), "Saturday");
        assert_eq!(d(2024, 2, 29), "Thursday");
        assert_eq!(d(2026, 12, 25), "Friday");
        assert_eq!(d(2026, 10, 3), "Saturday");
    }

    #[test]
    fn ordinals_are_right_including_the_teens() {
        let want = [(1, "1st"), (2, "2nd"), (3, "3rd"), (4, "4th"), (11, "11th"), (12, "12th"), (13, "13th"), (21, "21st"), (22, "22nd"), (23, "23rd"), (30, "30th"), (31, "31st")];
        for (n, s) in want {
            assert_eq!(ordinal(n), s);
        }
    }

    #[test]
    fn adding_time_rolls_over_midnight_and_month_end() {
        let late = LocalStamp { y: 2026, mo: 10, d: 31, h: 23, mi: 59, s: 50, utc_offset_min: 60 };
        let next = late.plus_ms(15_000);
        assert_eq!(next.long_date(), "Sunday 1st November 2026");
        assert_eq!(next.time_hms(), "00:00:05");
        assert_eq!(start().plus_ms(3_600_000 + 17_000).time_hms(), "02:28:00");
    }

    #[test]
    fn date_command_output_is_parsed() {
        let (st, tz) = parse_date_output("2026 10 04 01 27 43 +0100 BST\n").unwrap();
        assert_eq!(st, start());
        assert_eq!(tz, "BST");
        let (st, _) = parse_date_output("2026 01 05 09 00 00 -0530 IST").unwrap();
        assert_eq!(st.utc_offset_min, -330);
        assert!(parse_date_output("garbage").is_none());
        assert!(parse_date_output("2026 13 04 01 27 43 +0100 BST").is_none());
    }

    #[test]
    fn folder_and_file_names_follow_the_users_wording() {
        let end = LocalStamp { h: 2, mi: 15, s: 10, ..start() };
        assert_eq!(day_folder_name(&start()), "Sunday 4th October 2026");
        let later = start().plus_ms(3 * 3_600_000);
        assert_ne!(service_folder_name(&start(), "Faith"), service_folder_name(&later, "Faith"), "two services on one day never share a folder");
        assert!(service_folder_name(&start(), "Faith").ends_with(" - Faith"));
        assert!(!service_folder_name(&start(), "").contains(" - "));
        assert_eq!(live_base(&start(), ""), "Sunday 4th October 2026 - start 01h27m43s");
        assert_eq!(final_base(&start(), &end, ""), "Sunday 4th October 2026 - 01h27m43s to 02h15m10s");
        assert_eq!(final_base(&start(), &end, "Faith: that moves"), "Sunday 4th October 2026 - Faith- that moves - 01h27m43s to 02h15m10s");
        assert_eq!(recording_name("B", 1), "B - recording.wav");
        assert_eq!(recording_name("B", 2), "B - recording (part 2).wav");
        // a service that crosses midnight says so in the end part
        let next = LocalStamp { d: 5, h: 0, mi: 10, s: 2, ..start() };
        assert_eq!(final_base(&start(), &next, ""), "Sunday 4th October 2026 - 01h27m43s to Monday 5th October 2026 00h10m02s");
        for n in [live_base(&start(), "x/y"), final_base(&start(), &end, "a:b")] {
            assert!(!n.contains('/') && !n.contains(':'), "{n}");
        }
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(human_duration(38_000), "38 s");
        assert_eq!(human_duration(47 * 60_000 + 27_000), "47 min 27 s");
        assert_eq!(human_duration(3_600_000 + 5 * 60_000 + 9_000), "1 h 05 min 09 s");
    }

    fn meta() -> ServiceMeta {
        ServiceMeta {
            title: "Faith that moves".into(),
            church: "Grace Church".into(),
            app_version: "RhemaFlow-D v0.812.0".into(),
            start: start(),
            end: LocalStamp { h: 2, mi: 15, s: 10, ..start() },
            tz_name: "BST".into(),
            duration_ms: 47 * 60_000 + 27_000,
            translation: "KJV".into(),
            engine: "whisper (offline)".into(),
            mic: "MacBook Pro Microphone".into(),
            folder: "/Users/x/Downloads/RhemaFlow/Sunday 4th October 2026".into(),
            recording_files: vec!["Sunday 4th October 2026 - recording.wav".into()],
            auto_staged: 3,
            approved: 2,
            dismissed: 1,
        }
    }

    fn lines() -> Vec<Line> {
        vec![
            Line { ms: 63_000, text: "Revelations chapter 1 verse 6.".into() },
            Line { ms: 66_000, text: "Read.".into() },
            Line { ms: 130_000, text: "And hath made us kings and priests.".into() },
        ]
    }

    fn cites() -> Vec<Cite> {
        vec![
            Cite { ms: 65_000, label: "Revelation 1:6".into(), source: "voice".into(), text: "And hath made us kings and priests unto God".into() },
            Cite { ms: 131_000, label: "Revelation 1:6".into(), source: "quotation (auto)".into(), text: "x | y".into() },
        ]
    }

    #[test]
    fn the_transcript_opens_with_the_day_and_start_time_to_the_second() {
        let t = to_txt(&meta(), &lines(), &cites());
        let mut it = t.lines();
        assert_eq!(it.next(), Some("Sunday 4th October 2026"));
        assert!(t.contains("Start time    : 01:27:43 (1:27:43 AM) UTC+01:00 BST"), "{t}");
        assert!(t.contains("End time      : 02:15:10 (2:15:10 AM)"));
        assert!(t.contains("Length        : 47 min 27 s (00:47:27)"));
        assert!(t.contains("Microphone    : MacBook Pro Microphone"));
        assert!(t.contains("Words heard      : 13"), "{t}");
        assert!(t.contains("spoken reference  : 1"), "{t}");
        assert!(t.contains("quoted words"));
        // every line carries both clocks: 63 s after 01:27:43 is 01:28:46
        assert!(t.contains("[00:01:03 | 01:28:46] Revelations chapter 1 verse 6."), "{t}");
        assert!(t.contains("quiet for 1 min 04 s"), "a 64 s silence is marked: {t}");
        assert!(t.trim_end().ends_with("at 02:15:10 ---"));
    }

    #[test]
    fn markdown_has_details_index_with_text_and_transcript_in_order() {
        let m = to_markdown(&meta(), &lines(), &cites());
        assert!(m.starts_with("# Sunday 4th October 2026 \u{2014} Faith that moves\n"));
        assert!(m.contains("| Started | 01:27:43 (1:27:43 AM) UTC+01:00 BST |"));
        assert!(m.contains("| 1 | 00:01:05 | 01:28:48 | Revelation 1:6 | spoken reference | And hath made us kings and priests unto God |"), "{m}");
        assert!(m.contains("x / y"), "table cells never contain a raw pipe");
        assert!(m.find("## Service details").unwrap() < m.find("## Scripture index").unwrap());
        assert!(m.find("## Scripture index").unwrap() < m.find("## Transcript").unwrap());
        let empty = to_markdown(&meta(), &[], &[]);
        assert!(empty.contains("No Scripture was shown"));
    }

    #[test]
    fn live_header_names_the_day_and_start_time() {
        let h = live_header(&start(), "BST", "", "Grace Church", "v0.812.0");
        assert!(h.starts_with("Sunday 4th October 2026\n"));
        assert!(h.contains("Start time   : 01:27:43 (1:27:43 AM) UTC+01:00 BST"));
        assert_eq!(line_prefix(&start(), 63_000), "[00:01:03 | 01:28:46]");
    }

    #[test]
    fn writes_files_named_from_the_base() {
        let dir = std::env::temp_dir().join(format!("rf-naming-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let base = final_base(&meta().start, &meta().end, "");
        let names = write_transcripts(&dir, &base, Formats::default(), &meta(), &lines(), &cites());
        assert_eq!(
            names,
            vec![
                "Sunday 4th October 2026 - 01h27m43s to 02h15m10s - transcript.txt",
                "Sunday 4th October 2026 - 01h27m43s to 02h15m10s - transcript.srt",
                "Sunday 4th October 2026 - 01h27m43s to 02h15m10s - sermon notes.md",
            ]
        );
        let only = write_transcripts(&dir, "B", Formats { txt: true, srt: false, md: false }, &meta(), &lines(), &cites());
        assert_eq!(only, vec!["B - transcript.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
