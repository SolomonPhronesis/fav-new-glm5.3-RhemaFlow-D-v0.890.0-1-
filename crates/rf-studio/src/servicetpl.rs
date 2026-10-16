//! Feature 16: service templates and "copy last Sunday". Make a template from a service, start next week's from it with
//! the dates filled in, and list the coming Sundays.

use crate::vars::{self, Ctx};
use rf_service::Service;

/// The next time (strictly after `from`, or the same day if `include_today`) that falls on `weekday` (0 = Sunday)
/// at `hour:minute` local time. Returns seconds since 1970 (UTC).
pub fn next_weekday(from_unix: i64, utc_offset_min: i32, weekday: i64, hour: i64, minute: i64, include_today: bool) -> i64 {
    let l = vars::local(from_unix, utc_offset_min);
    let today = vars::days_from_civil(l.year, l.month, l.day);
    let mut add = (weekday - l.weekday).rem_euclid(7);
    let target_today = vars::from_local(l.year, l.month, l.day, hour, minute, utc_offset_min);
    if add == 0 && !(include_today && target_today > from_unix) {
        add = 7;
    }
    let (y, m, d) = vars::civil_from_days(today + add);
    vars::from_local(y, m, d, hour, minute, utc_offset_min)
}

/// The next `n` services on a weekday (for a "plan ahead" list).
pub fn upcoming(from_unix: i64, utc_offset_min: i32, weekday: i64, hour: i64, minute: i64, n: usize) -> Vec<i64> {
    let mut out = Vec::new();
    let mut t = from_unix;
    for i in 0..n {
        t = next_weekday(t, utc_offset_min, weekday, hour, minute, i == 0);
        out.push(t);
    }
    out
}

/// "Sunday 18th October 2026"
pub fn date_title(unix: i64, utc_offset_min: i32) -> String {
    vars::long_date(&vars::local(unix, utc_offset_min))
}

/// A template is a service with nothing "live" in it. Dates in titles written as `{date}` are filled in when it is used.
pub fn make_template(service: &Service, name: &str) -> Service {
    let mut t = service.clone();
    t.title = name.trim().to_string();
    t.last_live = None;
    t
}

/// A new service from a template for the given day. Item titles may contain `{date}`, `{weekday}`, ... Everything else is copied.
pub fn instantiate(template: &Service, service_unix: i64, utc_offset_min: i32, title: Option<&str>) -> Service {
    let ctx = Ctx::new(service_unix, utc_offset_min);
    let mut s = template.clone();
    s.title = match title {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => date_title(service_unix, utc_offset_min),
    };
    s.last_live = None;
    for item in &mut s.items {
        item.title = vars::render(&item.title, &ctx).text;
    }
    s
}

/// Next week's service from last week's: same items, a new date. Any date of the old service written in a title
/// ("Sunday 11th October 2026") is replaced by the new one.
pub fn copy_forward(old: &Service, old_unix: i64, new_unix: i64, utc_offset_min: i32) -> Service {
    let old_text = date_title(old_unix, utc_offset_min);
    let new_text = date_title(new_unix, utc_offset_min);
    let mut s = old.clone();
    s.last_live = None;
    s.title = if old.title.contains(&old_text) { old.title.replace(&old_text, &new_text) } else { new_text.clone() };
    for item in &mut s.items {
        item.title = item.title.replace(&old_text, &new_text);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_service::ItemKind;

    fn sunday_10_00(y: i64, m: i64, d: i64) -> i64 {
        vars::from_local(y, m, d, 10, 0, 0)
    }

    #[test]
    fn the_next_sunday_is_found_from_any_day() {
        // Wednesday 14 Oct 2026 -> Sunday 18 Oct 2026 10:00
        let wed = vars::from_local(2026, 10, 14, 15, 0, 0);
        assert_eq!(next_weekday(wed, 0, 0, 10, 0, false), sunday_10_00(2026, 10, 18));
        // on Sunday at 09:00, "today" still counts when asked, and does not when not
        let sun_early = vars::from_local(2026, 10, 18, 9, 0, 0);
        assert_eq!(next_weekday(sun_early, 0, 0, 10, 0, true), sunday_10_00(2026, 10, 18));
        assert_eq!(next_weekday(sun_early, 0, 0, 10, 0, false), sunday_10_00(2026, 10, 25));
        // on Sunday at 11:00 it is already over
        let sun_late = vars::from_local(2026, 10, 18, 11, 0, 0);
        assert_eq!(next_weekday(sun_late, 0, 0, 10, 0, true), sunday_10_00(2026, 10, 25));
    }

    #[test]
    fn month_and_year_ends_and_time_zones_work() {
        let thu = vars::from_local(2026, 12, 31, 12, 0, 0);
        assert_eq!(next_weekday(thu, 0, 0, 10, 0, false), sunday_10_00(2027, 1, 3));
        // UK summer time (+60): 10:00 local on 18 Oct is 09:00 UTC... clocks go back 25 Oct, but offsets are given by the caller
        let t = next_weekday(vars::from_local(2026, 10, 14, 12, 0, 60), 60, 0, 10, 0, false);
        assert_eq!(vars::local(t, 60).hour, 10);
        assert_eq!(vars::local(t, 60).weekday, 0);
    }

    #[test]
    fn a_list_of_coming_sundays_is_a_week_apart() {
        let wed = vars::from_local(2026, 10, 14, 15, 0, 0);
        let l = upcoming(wed, 0, 0, 10, 0, 4);
        assert_eq!(l.len(), 4);
        assert!(l.windows(2).all(|w| w[1] - w[0] == 7 * 86_400));
        assert_eq!(date_title(l[0], 0), "Sunday 18th October 2026");
        // starting on a Sunday morning, that very Sunday is first
        assert_eq!(upcoming(vars::from_local(2026, 10, 18, 8, 0, 0), 0, 0, 10, 0, 2)[0], sunday_10_00(2026, 10, 18));
        assert!(upcoming(wed, 0, 0, 10, 0, 0).is_empty());
    }

    fn sample() -> Service {
        let mut s = Service::new("Sunday 11th October 2026");
        s.add_item("Welcome - Sunday 11th October 2026", ItemKind::Timer { seconds: 300, label: "Start".into() });
        s.add_item("Notices for {date}", ItemKind::Scripture { reference: "John 3:16".into(), translation: "KJV".into() });
        s.last_live = Some(rf_service::Output::Black);
        s
    }

    #[test]
    fn a_template_has_nothing_live_and_a_new_service_gets_its_dates() {
        let t = make_template(&sample(), "Standard Sunday");
        assert_eq!(t.title, "Standard Sunday");
        assert!(t.last_live.is_none());
        let s = instantiate(&t, sunday_10_00(2026, 10, 18), 0, None);
        assert_eq!(s.title, "Sunday 18th October 2026");
        assert_eq!(s.items[1].title, "Notices for Sunday 18th October 2026");
        assert_eq!(s.items.len(), 2);
        let named = instantiate(&t, sunday_10_00(2026, 10, 18), 0, Some("Harvest"));
        assert_eq!(named.title, "Harvest");
        assert_eq!(instantiate(&t, 0, 0, Some("   ")).title, "Thursday 1st January 1970");
    }

    #[test]
    fn copying_forward_moves_dates_and_keeps_the_items() {
        let old = sample();
        let new = copy_forward(&old, sunday_10_00(2026, 10, 11), sunday_10_00(2026, 10, 18), 0);
        assert_eq!(new.title, "Sunday 18th October 2026");
        assert_eq!(new.items[0].title, "Welcome - Sunday 18th October 2026");
        assert!(new.last_live.is_none());
        assert_eq!(new.items.len(), old.items.len());
        assert_eq!(old.title, "Sunday 11th October 2026", "the old service is untouched");
        // a service titled something else gets the new date as its title
        let mut other = sample();
        other.title = "Harvest".into();
        assert_eq!(copy_forward(&other, sunday_10_00(2026, 10, 11), sunday_10_00(2026, 10, 18), 0).title, "Sunday 18th October 2026");
    }
}
