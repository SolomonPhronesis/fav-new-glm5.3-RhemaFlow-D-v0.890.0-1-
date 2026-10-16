//! Account + subscription (v0.831.0): Supabase sign-in and the paid-until check.
//!
//! Talks to Supabase through the system `curl` (always present on macOS), so the app gains no TLS
//! dependency. The parsing is pure and tested; only `Client::call` touches the network.
//! The RULES (grace periods, never cutting a live service) stay in `license.rs`; this file only
//! turns the server's answer into a `license::Ticket`.
//!
//! Server side: see `supabase/schema.sql` and `supabase/functions/create-checkout/index.ts`.
//! Configure with env `RF_SUPABASE_URL` and `RF_SUPABASE_ANON_KEY` (the anon key is public by design).

use crate::license::{Plan, Ts};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub user_id: String,
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subscription {
    pub plan: Plan,
    pub paid_until: Ts,
    pub status: String,
}

/// What the screen shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    SignedOut,
    Working(&'static str),
    SignedIn { email: String, sub: Subscription },
    /// signed in, but the account is already on 2 computers (v0.833.0)
    DeviceLimit,
    Notice(String),
    Failed(String),
}

enum Msg {
    In(Session, Subscription),
    Limit(Session),
    CodeSent,
    Err(String),
    Url(String),
}

pub struct Account {
    pub status: Status,
    pub email: String,
    pub password: String,
    pub open: bool,
    pub create_mode: bool,
    /// "email me a code" sign-in (no password to remember)
    pub code_mode: bool,
    pub code_sent: bool,
    pub code: String,
    session: Option<Session>,
    rx: Option<Receiver<Msg>>,
    dir: PathBuf,
}

// ---------------------------------------------------------------- pure parsing (tested)

/// "2026-11-13T10:00:00.123+00:00" -> unix seconds. None if it is not a date.
pub fn parse_iso(s: &str) -> Option<Ts> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    // days from civil (Howard Hinnant)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut t = days * 86_400 + hh * 3600 + mm * 60 + ss;
    // optional fraction, then Z or +hh:mm / -hh:mm
    let rest = &s[19..];
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit() && rest.starts_with('.'));
    if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
        let off = rest.get(1..3).and_then(|h| h.parse::<i64>().ok()).unwrap_or(0) * 3600
            + rest.get(4..6).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0) * 60;
        t -= if sign == '+' { off } else { -off };
    }
    Some(t)
}

pub fn plan_from(name: &str) -> Plan {
    match name {
        "church" | "pro" => Plan::Pro,
        _ => Plan::Standard, // monthly, annual
    }
}

/// The `subscriptions` rows -> what the app may use. No row, or an unpaid status = paid_until 0.
pub fn parse_subscription(rows: &Value) -> Subscription {
    let row = rows.as_array().and_then(|a| a.first());
    let Some(r) = row else { return Subscription { plan: Plan::Standard, paid_until: 0, status: "none".into() } };
    let status = r["status"].as_str().unwrap_or("inactive").to_string();
    let paying = matches!(status.as_str(), "active" | "trialing" | "past_due");
    let until = r["current_period_end"].as_str().and_then(parse_iso).unwrap_or(0);
    Subscription { plan: plan_from(r["plan"].as_str().unwrap_or("monthly")), paid_until: if paying { until } else { 0 }, status }
}

pub fn parse_session(v: &Value) -> Result<Session, String> {
    if let Some(m) = v["msg"].as_str().or(v["error_description"].as_str()).or(v["message"].as_str()) {
        if v["access_token"].is_null() {
            return Err(friendly(m));
        }
    }
    let get = |k: &str| v[k].as_str().map(str::to_string).ok_or_else(|| "The server answer was not understood.".to_string());
    Ok(Session {
        user_id: v["user"]["id"].as_str().map(str::to_string).ok_or("The server answer had no user.")?,
        email: v["user"]["email"].as_str().unwrap_or("").to_string(),
        access_token: get("access_token")?,
        refresh_token: get("refresh_token")?,
    })
}

fn friendly(m: &str) -> String {
    let l = m.to_lowercase();
    if l.contains("invalid login") {
        "That email and password do not match.".into()
    } else if l.contains("already registered") {
        "That email already has an account. Sign in instead.".into()
    } else if l.contains("email not confirmed") {
        "Please open the confirmation email we sent, then sign in.".into()
    } else {
        m.to_string()
    }
}

// ---------------------------------------------------------------- network

fn config() -> Option<(String, String)> {
    let url = std::env::var("RF_SUPABASE_URL").ok().or(option_env!("RF_SUPABASE_URL").map(str::to_string))?;
    let key = std::env::var("RF_SUPABASE_ANON_KEY").ok().or(option_env!("RF_SUPABASE_ANON_KEY").map(str::to_string))?;
    Some((url.trim_end_matches('/').to_string(), key))
}

fn call(method: &str, path: &str, bearer: Option<&str>, body: Option<Value>) -> Result<Value, String> {
    let (base, key) = config().ok_or("Accounts are not set up in this build (RF_SUPABASE_URL / RF_SUPABASE_ANON_KEY).")?;
    let mut c = Command::new("curl");
    c.args(["-sS", "--max-time", "15", "-X", method, &format!("{base}{path}"), "-H", &format!("apikey: {key}"), "-H", "Content-Type: application/json"]);
    c.args(["-H", &format!("Authorization: Bearer {}", bearer.unwrap_or(&key))]);
    if let Some(b) = body {
        c.args(["-d", &b.to_string()]);
    }
    let out = c.output().map_err(|_| "Could not start curl to reach the internet.".to_string())?;
    if !out.status.success() {
        return Err("No internet connection to the account server.".into());
    }
    serde_json::from_slice(&out.stdout).map_err(|_| "The account server sent something unreadable.".to_string())
}

fn sign_in_net(email: &str, pw: &str, create: bool) -> Result<Session, String> {
    let v = if create {
        call("POST", "/auth/v1/signup", None, Some(json!({"email": email, "password": pw})))?
    } else {
        call("POST", "/auth/v1/token?grant_type=password", None, Some(json!({"email": email, "password": pw})))?
    };
    if create && v["access_token"].is_null() && v["msg"].is_null() && v["error_description"].is_null() {
        return Err("Account created. Open the confirmation email, then sign in.".into());
    }
    parse_session(&v)
}

fn refresh_net(refresh: &str) -> Result<Session, String> {
    parse_session(&call("POST", "/auth/v1/token?grant_type=refresh_token", None, Some(json!({"refresh_token": refresh})))?)
}

/// Emails a 6-digit code (creates the account the first time: that is the sign-up).
fn send_code_net(email: &str) -> Result<(), String> {
    let v = call("POST", "/auth/v1/otp", None, Some(json!({"email": email, "create_user": true})))?;
    match v["msg"].as_str().or(v["error_description"].as_str()).or(v["message"].as_str()) {
        Some(m) => Err(friendly(m)),
        None => Ok(()),
    }
}

fn verify_code_net(email: &str, code: &str) -> Result<Session, String> {
    parse_session(&call("POST", "/auth/v1/verify", None, Some(json!({"type": "email", "email": email, "token": code.trim()})))?)
}

fn device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| Command::new("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()))
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "this computer".into())
}

/// Ok(true) = this computer is allowed, Ok(false) = the account is already on 2 other computers.
fn register_device(s: &Session) -> Result<bool, String> {
    let v = call("POST", "/rest/v1/rpc/register_device", Some(&s.access_token), Some(json!({"device_name": device_name()})))?;
    Ok(!v["message"].as_str().map_or(false, |m| m.contains("device_limit")))
}

/// After any successful sign-in: check the computer limit, then read the subscription.
fn finish(s: Session) -> Msg {
    match register_device(&s) {
        Ok(true) => match fetch_sub(&s) {
            Ok(sub) => Msg::In(s, sub),
            Err(e) => Msg::Err(e),
        },
        Ok(false) => Msg::Limit(s),
        Err(e) => Msg::Err(e),
    }
}

fn fetch_sub(s: &Session) -> Result<Subscription, String> {
    let rows = call("GET", &format!("/rest/v1/subscriptions?user_id=eq.{}&select=status,plan,current_period_end", s.user_id), Some(&s.access_token), None)?;
    Ok(parse_subscription(&rows))
}

/// The last good ticket is kept on disk so the offline grace period survives a restart.
pub fn save_ticket(dir: &std::path::Path, t: &crate::license::Ticket) {
    let plan = if t.plan == Plan::Pro { "pro" } else { "standard" };
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("ticket.json"), json!({"plan": plan, "paid_until": t.paid_until, "last_check_in": t.last_check_in, "latest_seen": t.latest_seen}).to_string());
}

pub fn load_ticket(dir: &std::path::Path) -> Option<crate::license::Ticket> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("ticket.json")).ok()?).ok()?;
    Some(crate::license::Ticket {
        plan: plan_from(v["plan"].as_str()?),
        paid_until: v["paid_until"].as_i64()?,
        last_check_in: v["last_check_in"].as_i64()?,
        latest_seen: v["latest_seen"].as_i64()?,
    })
}

pub fn dir_path() -> PathBuf {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".rhemaflow")).unwrap_or_else(std::env::temp_dir)
}

// ---------------------------------------------------------------- the object the app holds

impl Account {
    pub fn new(dir: PathBuf) -> Account {
        let mut a = Account { status: Status::SignedOut, email: String::new(), password: String::new(), open: false, create_mode: false, code_mode: false, code_sent: false, code: String::new(), session: None, rx: None, dir };
        // quietly sign in again with the saved refresh token (works offline: the last ticket is kept by the caller)
        if let Some(t) = std::fs::read_to_string(a.file()).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v["refresh_token"].as_str().map(str::to_string)) {
            a.status = Status::Working("Checking your subscription…");
            a.spawn(move || refresh_net(&t).map(finish).unwrap_or_else(Msg::Err));
        }
        a
    }

    fn file(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    fn spawn(&mut self, f: impl FnOnce() -> Msg + Send + 'static) {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        self.rx = Some(rx);
    }

    pub fn sign_in(&mut self) {
        let (e, p, create) = (self.email.trim().to_string(), self.password.clone(), self.create_mode);
        if e.is_empty() || p.len() < 6 {
            self.status = Status::Failed("Enter your email and a password of at least 6 characters.".into());
            return;
        }
        self.status = Status::Working(if create { "Creating your account…" } else { "Signing in…" });
        self.spawn(move || sign_in_net(&e, &p, create).map(finish).unwrap_or_else(Msg::Err));
    }

    pub fn send_code(&mut self) {
        let e = self.email.trim().to_string();
        if !e.contains('@') {
            self.status = Status::Failed("Enter your email address first.".into());
            return;
        }
        self.status = Status::Working("Sending your code…");
        self.spawn(move || send_code_net(&e).map(|_| Msg::CodeSent).unwrap_or_else(Msg::Err));
    }

    pub fn verify_code(&mut self) {
        let (e, c) = (self.email.trim().to_string(), self.code.clone());
        if c.trim().len() < 6 {
            self.status = Status::Failed("Type the 6-digit code from the email.".into());
            return;
        }
        self.status = Status::Working("Checking the code…");
        self.spawn(move || verify_code_net(&e, &c).map(finish).unwrap_or_else(Msg::Err));
    }

    /// "Sign out my other computers": forgets them on the server, then tries this one again.
    pub fn free_devices(&mut self) {
        let Some(s) = self.session.clone() else { return };
        self.status = Status::Working("Signing out your other computers…");
        self.spawn(move || match call("POST", "/rest/v1/rpc/forget_other_devices", Some(&s.access_token), Some(json!({"device_name": device_name()}))) {
            Ok(_) => finish(s),
            Err(e) => Msg::Err(e),
        });
    }

    /// Opens Stripe's own page: update the card, see invoices, cancel.
    pub fn billing(&mut self) {
        let Some(s) = self.session.clone() else { return };
        self.spawn(move || match call("POST", "/functions/v1/billing-portal", Some(&s.access_token), Some(json!({}))) {
            Ok(v) => v["url"].as_str().map(|u| Msg::Url(u.to_string())).unwrap_or(Msg::Err("Could not open billing (subscribe first).".into())),
            Err(e) => Msg::Err(e),
        });
    }

    pub fn sign_out(&mut self) {
        self.code_sent = false;
        self.code.clear();
        self.session = None;
        self.status = Status::SignedOut;
        self.password.clear();
        let _ = std::fs::remove_file(self.file());
    }

    /// Opens Stripe Checkout in the browser (the server creates the session: see the edge function).
    pub fn subscribe(&mut self, plan: &'static str) {
        let Some(s) = self.session.clone() else { return };
        self.spawn(move || match call("POST", "/functions/v1/create-checkout", Some(&s.access_token), Some(json!({"plan": plan}))) {
            Ok(v) => v["url"].as_str().map(|u| Msg::Url(u.to_string())).unwrap_or(Msg::Err("Could not start checkout.".into())),
            Err(e) => Msg::Err(e),
        });
    }

    /// Call every frame. Returns a fresh `Ticket` when the server has just answered.
    pub fn poll(&mut self, now: Ts, old: Option<&crate::license::Ticket>) -> Option<crate::license::Ticket> {
        let m = self.rx.as_ref()?.try_recv().ok()?;
        self.rx = None;
        match m {
            Msg::In(s, sub) => {
                // the refresh token stays on this computer only (file mode 600 on macOS/Linux)
                let _ = std::fs::create_dir_all(&self.dir);
                let _ = std::fs::write(self.file(), json!({"refresh_token": s.refresh_token}).to_string());
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(self.file(), std::fs::Permissions::from_mode(0o600));
                }
                let t = crate::license::after_check_in(now, sub.plan, sub.paid_until, old);
                self.status = Status::SignedIn { email: s.email.clone(), sub };
                self.session = Some(s);
                self.password.clear();
                Some(t)
            }
            Msg::Limit(s) => {
                self.session = Some(s);
                self.status = Status::DeviceLimit;
                None
            }
            Msg::CodeSent => {
                self.code_sent = true;
                self.status = Status::Notice("We emailed you a 6-digit code. Type it below.".into());
                None
            }
            Msg::Err(e) => {
                self.status = if self.session.is_some() && matches!(self.status, Status::SignedIn { .. }) { self.status.clone() } else { Status::Failed(e) };
                None
            }
            Msg::Url(u) => {
                let _ = Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(u).spawn();
                None
            }
        }
    }

    pub fn is_working(&self) -> bool {
        self.rx.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_become_unix_seconds() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2026-10-13T12:00:00+00:00"), Some(1_791_892_800));
        assert_eq!(parse_iso("2026-10-13T13:00:00.250+01:00"), Some(1_791_892_800));
        assert_eq!(parse_iso("not a date"), None);
    }

    #[test]
    fn active_subscription_is_paid_until_period_end() {
        let rows = json!([{"status": "active", "plan": "annual", "current_period_end": "2026-11-13T10:00:00+00:00"}]);
        let s = parse_subscription(&rows);
        assert_eq!(s.plan, Plan::Standard);
        assert_eq!(s.paid_until, parse_iso("2026-11-13T10:00:00+00:00").unwrap());
    }

    #[test]
    fn canceled_or_missing_means_not_paid() {
        assert_eq!(parse_subscription(&json!([])).paid_until, 0);
        assert_eq!(parse_subscription(&json!([{"status": "canceled", "plan": "monthly", "current_period_end": "2030-01-01T00:00:00Z"}])).paid_until, 0);
        assert_eq!(parse_subscription(&json!([{"status": "active", "plan": "church", "current_period_end": "2030-01-01T00:00:00Z"}])).plan, Plan::Pro);
    }

    #[test]
    fn the_ticket_survives_a_restart() {
        let d = std::env::temp_dir().join(format!("rf-acct-{}", std::process::id()));
        let t = crate::license::Ticket { plan: Plan::Pro, paid_until: 5, last_check_in: 3, latest_seen: 4 };
        save_ticket(&d, &t);
        let r = load_ticket(&d).unwrap();
        assert_eq!((r.plan, r.paid_until, r.last_check_in, r.latest_seen), (Plan::Pro, 5, 3, 4));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sessions_and_errors_are_read() {
        let ok = json!({"access_token": "a", "refresh_token": "r", "user": {"id": "u1", "email": "x@y.z"}});
        assert_eq!(parse_session(&ok).unwrap().user_id, "u1");
        let bad = json!({"error_description": "Invalid login credentials"});
        assert_eq!(parse_session(&bad).unwrap_err(), "That email and password do not match.");
    }

    #[test]
    fn a_paid_ticket_runs_through_the_license_rules() {
        let now = 1_791_892_800;
        let sub = parse_subscription(&json!([{"status": "active", "plan": "monthly", "current_period_end": "2026-11-13T10:00:00Z"}]));
        let t = crate::license::after_check_in(now, sub.plan, sub.paid_until, None);
        assert_eq!(crate::license::evaluate(now, Some(&t), &Default::default(), false, None), crate::license::Access::Active);
        let none = parse_subscription(&json!([]));
        let t2 = crate::license::after_check_in(now, none.plan, none.paid_until, None);
        assert!(matches!(crate::license::evaluate(now, Some(&t2), &Default::default(), false, None), crate::license::Access::FreeMode(_)));
    }
}
