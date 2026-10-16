//! Subscription / sign-in policy (v0.814.0): the RULES only, tested. Not yet connected to a server.
//!
//! The idea in one breath: signing in gives the app a signed "ticket" that says who paid and until
//! when. The app must phone home every so often to renew the ticket. If the Wi-Fi in the church
//! basement is bad, there is a grace period. Nothing is ever switched off in the middle of a
//! service, and when the ticket finally runs out the app falls back to a FREE mode (the
//! core Bible display keeps working) instead of a locked screen.
//!
//! Real tickets must be SIGNED by your server (e.g. Ed25519) and checked here with the matching
//! public key, so a user cannot edit the dates. That check plugs in where `Ticket` is made.

/// Unix seconds.
pub type Ts = i64;

const DAY: Ts = 86_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Free,
    Standard,
    Pro,
}

/// What the server last told us, plus when we last managed to talk to it.
#[derive(Clone, Copy, Debug)]
pub struct Ticket {
    pub plan: Plan,
    /// the subscription is paid until this moment
    pub paid_until: Ts,
    /// the last time the app reached the server successfully
    pub last_check_in: Ts,
    /// the latest clock reading we have ever seen (to notice the clock being wound back)
    pub latest_seen: Ts,
}

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// ask the server again after this many days
    pub check_in_every_days: i64,
    /// keep working offline this many days AFTER a check-in is due
    pub offline_grace_days: i64,
    /// after the subscription ends, keep working this many days (card failed, renewal late)
    pub payment_grace_days: i64,
}

impl Default for Policy {
    fn default() -> Self {
        // weekly check-in: a church that meets on Sundays gets a fresh ticket whenever the
        // Wi-Fi works once a week; 14 more days of grace cover a missed fortnight
        Policy { check_in_every_days: 7, offline_grace_days: 14, payment_grace_days: 5 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// everything on, nothing to say
    Active,
    /// everything on, but ask the person to connect to the internet soon
    CheckInSoon { days_left: i64 },
    /// subscription ended recently: everything on, but tell them to renew
    PaymentLate { days_left: i64 },
    /// paid features off; the free Bible display keeps working
    FreeMode(Why),
    /// not signed in at all
    SignedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    SubscriptionEnded,
    OfflineTooLong,
    ClockWoundBack,
}

/// What may the app do right now? `service_live` = a service is running: a change for the worse
/// is held until the service ends (never cut a live service).
pub fn evaluate(now: Ts, t: Option<&Ticket>, p: &Policy, service_live: bool, previous: Option<Access>) -> Access {
    let Some(t) = t else { return Access::SignedOut };
    if t.plan == Plan::Free {
        return Access::Active; // the free plan never expires
    }
    let decided = decide(now, t, p);
    // hold a downgrade until the service is over
    if service_live {
        if let Some(prev) = previous {
            if is_on(prev) && !is_on(decided) {
                return prev;
            }
        }
    }
    decided
}

fn is_on(a: Access) -> bool {
    matches!(a, Access::Active | Access::CheckInSoon { .. } | Access::PaymentLate { .. })
}

fn decide(now: Ts, t: &Ticket, p: &Policy) -> Access {
    // the clock went backwards more than an hour: someone is stretching the grace period
    if now + 3600 < t.latest_seen {
        return Access::FreeMode(Why::ClockWoundBack);
    }
    // paid?
    if now > t.paid_until {
        let late_by = (now - t.paid_until) / DAY;
        return if late_by < p.payment_grace_days {
            Access::PaymentLate { days_left: p.payment_grace_days - late_by }
        } else {
            Access::FreeMode(Why::SubscriptionEnded)
        };
    }
    // phoned home recently enough?
    let since = (now - t.last_check_in) / DAY;
    if since < p.check_in_every_days {
        Access::Active
    } else if since < p.check_in_every_days + p.offline_grace_days {
        Access::CheckInSoon { days_left: p.check_in_every_days + p.offline_grace_days - since }
    } else {
        Access::FreeMode(Why::OfflineTooLong)
    }
}

/// A successful check-in: the server's answer replaces the ticket.
pub fn after_check_in(now: Ts, plan: Plan, paid_until: Ts, old: Option<&Ticket>) -> Ticket {
    Ticket { plan, paid_until, last_check_in: now, latest_seen: old.map_or(now, |o| o.latest_seen.max(now)) }
}

/// Call on every launch / every few minutes: remember the latest clock reading.
pub fn note_clock(now: Ts, t: &mut Ticket) {
    if now > t.latest_seen {
        t.latest_seen = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Ts = 1_790_000_000;
    fn paid_ticket(days_ago_check_in: i64, paid_days_left: i64) -> (Ts, Ticket) {
        let now = T0 + 100 * DAY;
        (now, Ticket { plan: Plan::Standard, paid_until: now + paid_days_left * DAY, last_check_in: now - days_ago_check_in * DAY, latest_seen: now - 60 })
    }
    fn go(now: Ts, t: &Ticket) -> Access {
        evaluate(now, Some(t), &Policy::default(), false, None)
    }

    #[test]
    fn a_fresh_check_in_is_fully_active() {
        let (now, t) = paid_ticket(0, 300);
        assert_eq!(go(now, &t), Access::Active);
        let (now, t) = paid_ticket(6, 300);
        assert_eq!(go(now, &t), Access::Active, "6 days since check-in is within the 7");
    }

    #[test]
    fn a_missed_check_in_gives_a_countdown_then_free_mode() {
        let (now, t) = paid_ticket(7, 300);
        assert_eq!(go(now, &t), Access::CheckInSoon { days_left: 14 });
        let (now, t) = paid_ticket(20, 300);
        assert_eq!(go(now, &t), Access::CheckInSoon { days_left: 1 });
        let (now, t) = paid_ticket(21, 300);
        assert_eq!(go(now, &t), Access::FreeMode(Why::OfflineTooLong));
    }

    #[test]
    fn a_failed_card_gets_a_few_days_not_a_cliff() {
        let (now, t) = paid_ticket(1, -2); // ended two days ago
        assert_eq!(go(now, &t), Access::PaymentLate { days_left: 3 });
        let (now, t) = paid_ticket(1, -6);
        assert_eq!(go(now, &t), Access::FreeMode(Why::SubscriptionEnded));
    }

    #[test]
    fn winding_the_clock_back_does_not_buy_more_time() {
        let (now, mut t) = paid_ticket(30, 300); // overdue: free mode
        assert_eq!(go(now, &t), Access::FreeMode(Why::OfflineTooLong));
        // the person sets the clock back 29 days to look like a recent check-in
        let fake = now - 29 * DAY;
        t.latest_seen = now; // we saw "now" on a previous launch
        assert_eq!(go(fake, &t), Access::FreeMode(Why::ClockWoundBack));
        // a small correction (clock sync, daylight saving) is fine
        let (now2, mut t2) = paid_ticket(1, 300);
        t2.latest_seen = now2 + 1800;
        assert_eq!(go(now2, &t2), Access::Active);
    }

    #[test]
    fn nothing_is_switched_off_in_the_middle_of_a_service() {
        let (now, t) = paid_ticket(21, 300); // would be free mode
        let prev = Access::CheckInSoon { days_left: 1 };
        assert_eq!(evaluate(now, Some(&t), &Policy::default(), true, Some(prev)), prev, "held while live");
        assert_eq!(evaluate(now, Some(&t), &Policy::default(), false, Some(prev)), Access::FreeMode(Why::OfflineTooLong), "applied after");
        // an upgrade is never held back
        let (now, t) = paid_ticket(0, 300);
        assert_eq!(evaluate(now, Some(&t), &Policy::default(), true, Some(Access::FreeMode(Why::OfflineTooLong))), Access::Active);
    }

    #[test]
    fn signed_out_free_plan_and_check_in_renewal() {
        assert_eq!(evaluate(T0, None, &Policy::default(), false, None), Access::SignedOut);
        let free = Ticket { plan: Plan::Free, paid_until: 0, last_check_in: 0, latest_seen: 0 };
        assert_eq!(go(T0, &free), Access::Active, "the free plan never expires");
        let (now, old) = paid_ticket(30, 300);
        let renewed = after_check_in(now, Plan::Pro, now + 365 * DAY, Some(&old));
        assert_eq!(go(now, &renewed), Access::Active);
        assert_eq!(renewed.latest_seen, now, "never goes backwards");
        let mut t = renewed;
        note_clock(now + 5, &mut t);
        assert_eq!(t.latest_seen, now + 5);
        note_clock(now - 500, &mut t);
        assert_eq!(t.latest_seen, now + 5, "an earlier reading is ignored");
    }

    #[test]
    fn the_default_policy_is_weekly_check_in_with_two_weeks_of_grace() {
        let p = Policy::default();
        assert_eq!((p.check_in_every_days, p.offline_grace_days, p.payment_grace_days), (7, 14, 5));
    }
}
