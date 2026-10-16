//! v0.833.0: close down cleanly when the operating system asks the app to quit, even if the screen is frozen.
//!
//! Activity Monitor "Quit", the Dock's "Quit", `killall` and Terminal's Ctrl-C all send a polite signal
//! (SIGTERM / SIGINT / SIGHUP) first. A frozen window cannot answer it, but this small background thread can:
//! it stops the sound players the app started (afplay/ffmpeg), then exits. Only a hard "Force Quit" (SIGKILL)
//! cannot be caught; for that case the next start stops any leftover player (`sound::kill_strays`), and
//! `Force-Quit-RhemaFlow.command` does both in one double-click.

use std::sync::atomic::{AtomicBool, Ordering};

static ASKED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: libc::c_int) {
    // only sets a flag: nothing else is safe to do inside a signal handler
    ASKED.store(true, Ordering::SeqCst);
}

pub fn install() {
    #[cfg(unix)]
    unsafe {
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::signal(sig, on_signal as usize);
        }
    }
    let _ = std::thread::Builder::new().name("quit-guard".into()).spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_millis(150));
        if ASKED.load(Ordering::SeqCst) {
            crate::sound::kill_strays();
            std::process::exit(0);
        }
    });
}
