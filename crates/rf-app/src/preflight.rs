//! v0.833.0: the pre-flight check, "is everything ready before I press Start service?".
//!
//! Pure rules (tested) turn what the app knows into green / amber / red lines. Red = the service will not work
//! as expected (no microphone, no speech model, disk nearly full). Amber = works, but you should know
//! (one screen only, no internet, subscription needs a check-in). A subscription problem is never red:
//! the Bible display always keeps working.

use crate::license::Access;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Ok,
    Warn,
    Bad,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub detail: String,
}

pub struct Inputs {
    pub mic_count: usize,
    pub mic_selected: Option<String>,
    pub has_model: bool,
    pub free_bytes: Option<u64>,
    pub ffmpeg: bool,
    pub afplay: bool,
    pub screens: usize,
    /// None = still checking
    pub internet: Option<bool>,
    pub access: Access,
}

fn c(name: &'static str, level: Level, detail: impl Into<String>) -> Check {
    Check { name, level, detail: detail.into() }
}

pub fn evaluate(i: &Inputs) -> Vec<Check> {
    let mut v = Vec::new();
    v.push(match (i.mic_count, &i.mic_selected) {
        (0, _) => c("Microphone", Level::Bad, "No microphone found. Plug one in, then press Check again."),
        (_, Some(m)) => c("Microphone", Level::Ok, format!("Using {m}")),
        (_, None) => c("Microphone", Level::Ok, "Using the Mac's default microphone"),
    });
    v.push(if i.has_model {
        c("Speech model", Level::Ok, "Offline speech recognition is ready")
    } else {
        c("Speech model", Level::Bad, "No speech model installed: voice detection will not work (typing and clicking still do)")
    });
    v.push(match i.free_bytes {
        None => c("Disk space", Level::Warn, "Could not read the free space on the save drive"),
        Some(b) => {
            let hours = b as f64 / crate::export::WAV_BYTES_PER_HOUR as f64;
            let gb = b as f64 / 1e9;
            if hours < 1.0 {
                c("Disk space", Level::Bad, format!("Only {gb:.1} GB free: less than 1 hour of recording"))
            } else if hours < 3.0 {
                c("Disk space", Level::Warn, format!("{gb:.1} GB free: about {hours:.1} hours of recording"))
            } else {
                c("Disk space", Level::Ok, format!("{gb:.0} GB free"))
            }
        }
    });
    v.push(match (i.afplay, i.ffmpeg) {
        (false, _) => c("Sound", Level::Warn, "The Mac's sound player was not found: video and music will be silent"),
        (true, false) => c("Sound", Level::Warn, "Sound plays, but jumping forward/back inside a sound needs ffmpeg (brew install ffmpeg)"),
        (true, true) => c("Sound", Level::Ok, "Sound player and ffmpeg found"),
    });
    v.push(match i.screens {
        0 | 1 => c("Screens", Level::Warn, "Only one screen: the congregation display would share it with this window"),
        n => c("Screens", Level::Ok, format!("{n} screens connected")),
    });
    v.push(match i.internet {
        None => c("Internet", Level::Warn, "Checking…"),
        Some(true) => c("Internet", Level::Ok, "Online (needed only for sign-in checks and updates)"),
        Some(false) => c("Internet", Level::Warn, "Offline: everything still works; your subscription is checked next time you are online"),
    });
    v.push(match i.access {
        Access::Active => c("Subscription", Level::Ok, "Active"),
        Access::CheckInSoon { days_left } => c("Subscription", Level::Warn, format!("Go online within {days_left} day(s) to keep it verified")),
        Access::PaymentLate { days_left } => c("Subscription", Level::Warn, format!("Payment is late: {days_left} day(s) left before free mode")),
        Access::SignedOut => c("Subscription", Level::Warn, "Not signed in: free Bible display only (press Sign in)"),
        Access::FreeMode(_) => c("Subscription", Level::Warn, "No active subscription: free Bible display only"),
    });
    v
}

/// The worst line decides the headline.
pub fn worst(checks: &[Check]) -> Level {
    checks.iter().map(|c| c.level).max().unwrap_or(Level::Ok)
}

/// One quick request to see whether the internet is reachable (run off the UI thread).
pub fn internet_reachable() -> bool {
    std::process::Command::new("curl")
        .args(["-sS", "-I", "--max-time", "4", "-o", "/dev/null", "https://www.apple.com/library/test/success.html"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Inputs {
        Inputs { mic_count: 1, mic_selected: None, has_model: true, free_bytes: Some(200_000_000_000), ffmpeg: true, afplay: true, screens: 2, internet: Some(true), access: Access::Active }
    }

    #[test]
    fn everything_ready_is_all_green() {
        let r = evaluate(&good());
        assert_eq!(r.len(), 7);
        assert_eq!(worst(&r), Level::Ok);
    }

    #[test]
    fn no_mic_or_model_or_disk_is_red_and_the_rest_amber() {
        let mut i = good();
        i.mic_count = 0;
        assert_eq!(worst(&evaluate(&i)), Level::Bad);
        let mut i = good();
        i.has_model = false;
        assert_eq!(worst(&evaluate(&i)), Level::Bad);
        let mut i = good();
        i.free_bytes = Some(50_000_000); // under an hour
        assert_eq!(evaluate(&i)[2].level, Level::Bad);
        i.free_bytes = Some(200_000_000); // about 1.7 hours
        assert_eq!(evaluate(&i)[2].level, Level::Warn);
        let mut i = good();
        i.screens = 1;
        i.internet = Some(false);
        assert_eq!(worst(&evaluate(&i)), Level::Warn);
    }

    #[test]
    fn a_subscription_problem_never_blocks_the_service() {
        for a in [Access::SignedOut, Access::CheckInSoon { days_left: 2 }, Access::PaymentLate { days_left: 1 }, Access::FreeMode(crate::license::Why::SubscriptionEnded)] {
            let mut i = good();
            i.access = a;
            let r = evaluate(&i);
            assert_eq!(r[6].level, Level::Warn, "{a:?}");
            assert_ne!(worst(&r), Level::Bad);
        }
    }
}
