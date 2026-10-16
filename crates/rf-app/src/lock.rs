//! Volunteer lock (v0.835.0): a PIN-protected mode in which a helper can run the show (next, back, blackout, clear,
//! media, run-sheet, lower thirds) but cannot change settings, end the service, delete things or quit by accident.
//! The lock survives a restart. A deterrent against accidents, not security: the PIN is kept lightly hashed.

use serde::{Deserialize, Serialize};

/// Things a locked app refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guarded {
    EndService,
    Quit,
    Settings,
    EditSongs,
    EditMedia,
    ChangeScreens,
    ChangeLayout,
    Bibles,
    Account,
    Web,
    Backup,
    Looks,
    Controllers,
}

pub const MAX_TRIES: u32 = 5;
pub const LOCKOUT_MS: u64 = 30_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Lock {
    pub locked: bool,
    pin_hash: u64,
    #[serde(skip)]
    failed: u32,
    #[serde(skip)]
    blocked_until: u64,
}

fn hash(pin: &str) -> u64 {
    // FNV-1a with a fixed salt: stable across versions and platforms (DefaultHasher is not)
    let mut h: u64 = 0xcbf29ce484222325;
    for b in format!("rhemaflow-lock:{pin}").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl Lock {
    pub fn has_pin(&self) -> bool {
        self.pin_hash != 0
    }

    /// 4 to 8 digits.
    pub fn valid_pin(pin: &str) -> bool {
        (4..=8).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
    }

    pub fn set_pin(&mut self, pin: &str) -> Result<(), String> {
        if !Self::valid_pin(pin) {
            return Err("The PIN must be 4 to 8 digits.".into());
        }
        self.pin_hash = hash(pin);
        Ok(())
    }

    /// Lock now. Needs a PIN to have been set (otherwise nobody could unlock it).
    pub fn lock(&mut self) -> Result<(), String> {
        if !self.has_pin() {
            return Err("Choose a PIN first, so you can unlock again.".into());
        }
        self.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self, pin: &str, now: u64) -> Result<(), String> {
        if now < self.blocked_until {
            return Err(format!("Too many wrong tries. Wait {} seconds.", (self.blocked_until - now).div_ceil(1000)));
        }
        if self.pin_hash != 0 && hash(pin.trim()) == self.pin_hash {
            self.locked = false;
            self.failed = 0;
            return Ok(());
        }
        self.failed += 1;
        if self.failed >= MAX_TRIES {
            self.failed = 0;
            self.blocked_until = now + LOCKOUT_MS;
            return Err(format!("Wrong PIN. Locked out for {} seconds.", LOCKOUT_MS / 1000));
        }
        Err(format!("Wrong PIN ({} tries left).", MAX_TRIES - self.failed))
    }

    pub fn allows(&self, _what: Guarded) -> bool {
        !self.locked
    }
}

/// Words for a refused action.
pub fn refusal(what: Guarded) -> &'static str {
    match what {
        Guarded::EndService => "Locked: a helper cannot end the service. Unlock with the PIN first.",
        Guarded::Quit => "Locked: RhemaFlow will not close while it is locked. Unlock with the PIN first.",
        _ => "Locked: this setting is for the person in charge. Unlock with the PIN first.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_cannot_lock_without_a_pin_and_pins_must_be_digits() {
        let mut l = Lock::default();
        assert!(l.lock().is_err());
        assert!(l.set_pin("12").is_err() && l.set_pin("12ab").is_err() && l.set_pin("123456789").is_err());
        l.set_pin("2468").unwrap();
        l.lock().unwrap();
        assert!(l.locked && !l.allows(Guarded::EndService) && !l.allows(Guarded::Settings));
    }

    #[test]
    fn the_right_pin_unlocks_and_a_wrong_one_does_not() {
        let mut l = Lock::default();
        l.set_pin("2468").unwrap();
        l.lock().unwrap();
        assert!(l.unlock("1111", 0).unwrap_err().contains("4 tries left"));
        assert!(l.locked);
        l.unlock(" 2468 ", 0).unwrap();
        assert!(!l.locked && l.allows(Guarded::Quit));
    }

    #[test]
    fn five_wrong_tries_lock_out_for_thirty_seconds_even_for_the_right_pin() {
        let mut l = Lock::default();
        l.set_pin("2468").unwrap();
        l.lock().unwrap();
        for i in 0..5 {
            assert!(l.unlock("0000", 1_000 + i).is_err());
        }
        assert!(l.unlock("2468", 2_000).unwrap_err().contains("Wait"));
        assert!(l.locked);
        l.unlock("2468", 1_004 + LOCKOUT_MS).unwrap();
        assert!(!l.locked);
    }

    #[test]
    fn the_lock_survives_a_restart_and_the_pin_is_not_stored_as_typed() {
        let mut l = Lock::default();
        l.set_pin("2468").unwrap();
        l.lock().unwrap();
        let j = serde_json::to_string(&l).unwrap();
        assert!(!j.contains("2468"));
        let mut back: Lock = serde_json::from_str(&j).unwrap();
        assert!(back.locked);
        back.unlock("2468", 0).unwrap();
        assert_eq!(hash("2468"), hash("2468"));
        assert_ne!(hash("2468"), hash("2469"));
    }
}
