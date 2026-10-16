//! Feature 7: words that fill themselves in at show time: `{date}`, `{time:12h}`, `{countdown:Sermon}`, `{speaker}`.
//! `{{` and `}}` write a real brace. Anything unknown is left as typed and reported, never dropped.
//!
//! Also the calendar helpers the rest of the crate uses (no clock and no time library: the time is passed in).

use std::collections::HashMap;

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
pub const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday ... 6 = Saturday
pub fn weekday_of_days(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

pub fn ordinal(d: i64) -> String {
    let suffix = match (d % 100, d % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{d}{suffix}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Local {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    /// 0 = Sunday
    pub weekday: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
}

/// A moment (seconds since 1970 in UTC) as the wall clock reads where the church is. `utc_offset_min` is e.g. 60 for UK summer time.
pub fn local(unix: i64, utc_offset_min: i32) -> Local {
    let t = unix + utc_offset_min as i64 * 60;
    let days = t.div_euclid(86_400);
    let sod = t.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Local { year, month, day, weekday: weekday_of_days(days), hour: sod / 3600, minute: (sod % 3600) / 60, second: sod % 60 }
}

/// The reverse: wall-clock time to seconds since 1970.
pub fn from_local(year: i64, month: i64, day: i64, hour: i64, minute: i64, utc_offset_min: i32) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 - utc_offset_min as i64 * 60
}

pub fn long_date(l: &Local) -> String {
    format!("{} {} {} {}", WEEKDAYS[l.weekday as usize], ordinal(l.day), MONTHS[(l.month - 1) as usize], l.year)
}

#[derive(Clone, Debug, Default)]
pub struct Ctx {
    vars: HashMap<String, String>,
    pub now_unix: i64,
    pub utc_offset_min: i32,
    timers: HashMap<String, i64>,
}

impl Ctx {
    pub fn new(now_unix: i64, utc_offset_min: i32) -> Ctx {
        Ctx { now_unix, utc_offset_min, ..Default::default() }
    }
    /// Names are not case sensitive.
    pub fn set(&mut self, name: &str, value: &str) -> &mut Self {
        self.vars.insert(name.trim().to_lowercase(), value.to_string());
        self
    }
    /// Seconds left on a named timer, if it is set.
    pub fn timer(&self, name: &str) -> Option<i64> {
        self.timers.get(&name.trim().to_lowercase()).copied()
    }
    /// Seconds left on a named timer (negative = over time).
    pub fn set_timer(&mut self, name: &str, secs_left: i64) -> &mut Self {
        self.timers.insert(name.trim().to_lowercase(), secs_left);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// Names that were not known, in order, without repeats.
    pub unknown: Vec<String>,
}

pub fn clock_text(secs: i64) -> String {
    let neg = secs < 0;
    let s = secs.abs();
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    let body = if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m:02}:{sec:02}") };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

fn builtin(name: &str, arg: &str, ctx: &Ctx) -> Option<String> {
    let l = local(ctx.now_unix, ctx.utc_offset_min);
    let arg_l = arg.trim().to_lowercase();
    match name {
        "date" => Some(match arg_l.as_str() {
            "" | "long" => long_date(&l),
            "short" | "uk" => format!("{:02}/{:02}/{}", l.day, l.month, l.year),
            "us" => format!("{:02}/{:02}/{}", l.month, l.day, l.year),
            "iso" => format!("{}-{:02}-{:02}", l.year, l.month, l.day),
            "day" => format!("{} {}", ordinal(l.day), MONTHS[(l.month - 1) as usize]),
            _ => return None,
        }),
        "time" => Some(match arg_l.as_str() {
            "" | "24h" => format!("{:02}:{:02}", l.hour, l.minute),
            "12h" => {
                let h12 = if l.hour % 12 == 0 { 12 } else { l.hour % 12 };
                format!("{}:{:02} {}", h12, l.minute, if l.hour < 12 { "am" } else { "pm" })
            }
            "hms" => format!("{:02}:{:02}:{:02}", l.hour, l.minute, l.second),
            _ => return None,
        }),
        "weekday" => Some(WEEKDAYS[l.weekday as usize].to_string()),
        "day" => Some(l.day.to_string()),
        "month" => Some(MONTHS[(l.month - 1) as usize].to_string()),
        "year" => Some(l.year.to_string()),
        "countdown" => ctx.timers.get(&arg_l).map(|s| clock_text(*s)),
        "countdown_to" => {
            let (h, m) = arg_l.split_once(':')?;
            let (h, m): (i64, i64) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
            if !(0..24).contains(&h) || !(0..60).contains(&m) {
                return None;
            }
            let target = from_local(l.year, l.month, l.day, h, m, ctx.utc_offset_min);
            Some(clock_text((target - ctx.now_unix).max(0)))
        }
        _ => None,
    }
}

pub fn render(template: &str, ctx: &Ctx) -> Rendered {
    let mut out = String::with_capacity(template.len());
    let mut unknown: Vec<String> = Vec::new();
    let cs: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '{' && cs.get(i + 1) == Some(&'{') {
            out.push('{');
            i += 2;
        } else if c == '}' && cs.get(i + 1) == Some(&'}') {
            out.push('}');
            i += 2;
        } else if c == '{' {
            let mut j = i + 1;
            while j < cs.len() && cs[j] != '}' && cs[j] != '{' && cs[j] != '\n' {
                j += 1;
            }
            if j < cs.len() && cs[j] == '}' {
                let inner: String = cs[i + 1..j].iter().collect();
                let (name, arg) = match inner.split_once(':') {
                    Some((n, a)) => (n.trim().to_lowercase(), a.to_string()),
                    None => (inner.trim().to_lowercase(), String::new()),
                };
                let value = builtin(&name, &arg, ctx).or_else(|| if arg.is_empty() { ctx.vars.get(&name).cloned() } else { None });
                match value {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.extend(cs[i..=j].iter());
                        let shown = inner.trim().to_string();
                        if !unknown.contains(&shown) {
                            unknown.push(shown);
                        }
                    }
                }
                i = j + 1;
            } else {
                out.push('{');
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    Rendered { text: out, unknown }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Wednesday 14 October 2026, 13:05:09 UTC
    fn now() -> i64 {
        from_local(2026, 10, 14, 13, 5, 0) + 9
    }

    #[test]
    fn calendar_round_trips_and_knows_the_weekday() {
        assert_eq!(civil_from_days(days_from_civil(2026, 10, 14)), (2026, 10, 14));
        assert_eq!(weekday_of_days(days_from_civil(2026, 10, 14)), 3); // Wednesday
        assert_eq!(weekday_of_days(days_from_civil(1970, 1, 1)), 4); // Thursday
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29)); // leap day
        assert_eq!(civil_from_days(days_from_civil(1969, 12, 31)), (1969, 12, 31));
        assert_eq!(days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28), 2);
    }

    #[test]
    fn ordinals() {
        let s: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 30, 31].iter().map(|d| ordinal(*d)).collect();
        assert_eq!(s, vec!["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "30th", "31st"]);
    }

    #[test]
    fn local_time_follows_the_offset_across_midnight() {
        let l = local(from_local(2026, 6, 30, 23, 30, 0), 60); // 23:30 UTC is 00:30 next day in UK summer
        assert_eq!((l.month, l.day, l.hour, l.minute), (7, 1, 0, 30));
        assert_eq!(from_local(2026, 7, 1, 0, 30, 60), from_local(2026, 6, 30, 23, 30, 0));
    }

    #[test]
    fn dates_and_times_fill_in() {
        let c = Ctx::new(now(), 0);
        let r = render("{date} at {time}. {date:short} / {date:iso} / {date:day} / {weekday} {day} {month} {year}", &c);
        assert_eq!(r.text, "Wednesday 14th October 2026 at 13:05. 14/10/2026 / 2026-10-14 / 14th October / Wednesday 14 October 2026");
        assert!(r.unknown.is_empty());
        assert_eq!(render("{time:12h}", &c).text, "1:05 pm");
        assert_eq!(render("{time:hms}", &c).text, "13:05:09");
        let midnight = Ctx::new(from_local(2026, 10, 14, 0, 7, 0), 0);
        assert_eq!(render("{time:12h}", &midnight).text, "12:07 am");
    }

    #[test]
    fn your_own_names_work_and_are_not_case_sensitive() {
        let mut c = Ctx::new(now(), 0);
        c.set("Speaker", "Pastor Dan").set("series", "Romans");
        assert_eq!(render("{speaker} - {SERIES}", &c).text, "Pastor Dan - Romans");
    }

    #[test]
    fn unknown_names_stay_visible_and_are_reported_once() {
        let c = Ctx::new(now(), 0);
        let r = render("Hi {nope} and {nope} and {date:weird}", &c);
        assert_eq!(r.text, "Hi {nope} and {nope} and {date:weird}");
        assert_eq!(r.unknown, vec!["nope", "date:weird"]);
    }

    #[test]
    fn braces_can_be_escaped_and_stray_ones_are_kept() {
        let c = Ctx::new(now(), 0);
        assert_eq!(render("{{date}} {date:iso}", &c).text, "{date} 2026-10-14");
        assert_eq!(render("a { b", &c).text, "a { b");
        assert_eq!(render("a } b", &c).text, "a } b");
        assert_eq!(render("{", &c).text, "{");
        assert_eq!(render("{unterminated\nnext}", &c).text, "{unterminated\nnext}");
        assert_eq!(render("", &c).text, "");
        assert_eq!(render("Señor {year} ✝", &c).text, "Señor 2026 ✝");
    }

    #[test]
    fn countdowns() {
        let mut c = Ctx::new(now(), 0);
        c.set_timer("Sermon", 299).set_timer("over", -5).set_timer("long", 3725);
        assert_eq!(render("{countdown:sermon}", &c).text, "04:59");
        assert_eq!(render("{countdown:over}", &c).text, "-00:05");
        assert_eq!(render("{countdown:long}", &c).text, "1:02:05");
        assert_eq!(render("{countdown:missing}", &c).unknown, vec!["countdown:missing"]);
        // 13:05:09 now, service at 13:30 -> 24:51 left
        assert_eq!(render("{countdown_to:13:30}", &c).text, "24:51");
        assert_eq!(render("{countdown_to:09:00}", &c).text, "00:00"); // already passed
        assert_eq!(render("{countdown_to:25:00}", &c).unknown.len(), 1);
    }
}
