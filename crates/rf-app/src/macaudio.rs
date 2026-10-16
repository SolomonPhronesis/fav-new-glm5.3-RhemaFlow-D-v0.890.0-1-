//! The Mac's own audio player, used in-process (v0.839.0).
//!
//! Why: the old way ran the `afplay` program. It needs half a second to start, cannot start part-way in
//! (so every jump had to cut a new copy of the sound), cannot say where it is (we kept our own clock),
//! and so the sound and picture drifted. `AVAudioPlayer` (part of macOS, no extra library) starts at once,
//! jumps instantly (`currentTime`), and tells the truth about where it is. The picture follows that
//! position, so the two stay matched with nothing for the operator to adjust.
//!
//! It is called through the Objective-C runtime directly (no crate): a handful of messages, each typed
//! exactly. If anything fails (not a Mac, a format AVAudioPlayer cannot read) `open` returns `None` and
//! `sound.rs` falls back to `afplay`, so nothing gets worse. Set RHEMAFLOW_AFPLAY=1 to force the old way.
//!
//! Also here: `disable_app_nap()`. macOS slows a window that is behind others or not clicked ("App Nap"),
//! which froze the picture while the sound (a separate program) played on, and made it catch up slowly
//! when you clicked back. Declaring the app "user initiated, latency critical" stops that.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_char, c_void, CString};

    pub type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_fpret();
    }
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    fn sel(name: &str) -> Sel {
        let c = CString::new(name).unwrap_or_default();
        unsafe { sel_registerName(c.as_ptr()) }
    }
    fn class(name: &str) -> Id {
        let c = CString::new(name).unwrap_or_default();
        unsafe { objc_getClass(c.as_ptr()) }
    }
    fn send_ptr() -> *const c_void {
        objc_msgSend as *const c_void
    }
    #[cfg(target_arch = "x86_64")]
    fn send_fp_ptr() -> *const c_void {
        objc_msgSend_fpret as *const c_void
    }
    #[cfg(not(target_arch = "x86_64"))]
    fn send_fp_ptr() -> *const c_void {
        objc_msgSend as *const c_void
    }

    unsafe fn msg_id(o: Id, s: &str) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(send_ptr());
        f(o, sel(s))
    }
    unsafe fn msg_id_id(o: Id, s: &str, a: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(send_ptr());
        f(o, sel(s), a)
    }
    unsafe fn msg_bool(o: Id, s: &str) -> bool {
        let f: unsafe extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(send_ptr());
        f(o, sel(s)) != 0
    }
    unsafe fn msg_void(o: Id, s: &str) {
        let f: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(send_ptr());
        f(o, sel(s))
    }
    unsafe fn msg_void_f64(o: Id, s: &str, v: f64) {
        let f: unsafe extern "C" fn(Id, Sel, f64) = std::mem::transmute(send_ptr());
        f(o, sel(s), v)
    }
    unsafe fn msg_void_f32(o: Id, s: &str, v: f32) {
        let f: unsafe extern "C" fn(Id, Sel, f32) = std::mem::transmute(send_ptr());
        f(o, sel(s), v)
    }
    unsafe fn msg_f64(o: Id, s: &str) -> f64 {
        let f: unsafe extern "C" fn(Id, Sel) -> f64 = std::mem::transmute(send_fp_ptr());
        f(o, sel(s))
    }

    /// An autoreleased NSString (only valid until the pool is drained).
    unsafe fn nsstring(s: &str) -> Id {
        let c = CString::new(s).unwrap_or_default();
        let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id = std::mem::transmute(send_ptr());
        f(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr())
    }

    pub struct NativePlayer {
        obj: Id,
    }
    // the player is only ever used from the UI thread; Send lets the app struct move at start-up
    unsafe impl Send for NativePlayer {}

    impl NativePlayer {
        pub fn open(path: &str) -> Option<NativePlayer> {
            if std::env::var_os("RHEMAFLOW_AFPLAY").is_some() {
                return None;
            }
            unsafe {
                let pool = msg_id(msg_id(class("NSAutoreleasePool"), "alloc"), "init");
                let url = msg_id_id(class("NSURL"), "fileURLWithPath:", nsstring(path));
                let alloc = msg_id(class("AVAudioPlayer"), "alloc");
                let mut err: Id = std::ptr::null_mut();
                let f: unsafe extern "C" fn(Id, Sel, Id, *mut Id) -> Id = std::mem::transmute(send_ptr());
                let obj = f(alloc, sel("initWithContentsOfURL:error:"), url, &mut err);
                msg_void(pool, "drain");
                if obj.is_null() {
                    return None;
                }
                msg_bool(obj, "prepareToPlay");
                Some(NativePlayer { obj })
            }
        }
        pub fn play(&self) -> bool {
            unsafe { msg_bool(self.obj, "play") }
        }
        pub fn pause(&self) {
            unsafe { msg_void(self.obj, "pause") }
        }
        pub fn is_playing(&self) -> bool {
            unsafe { msg_bool(self.obj, "isPlaying") }
        }
        pub fn set_time_secs(&self, t: f64) {
            unsafe { msg_void_f64(self.obj, "setCurrentTime:", t.max(0.0)) }
        }
        pub fn time_secs(&self) -> f64 {
            unsafe { msg_f64(self.obj, "currentTime") }
        }
        pub fn duration_secs(&self) -> f64 {
            unsafe { msg_f64(self.obj, "duration") }
        }
        pub fn set_volume(&self, v: f32) {
            unsafe { msg_void_f32(self.obj, "setVolume:", v.clamp(0.0, 1.0)) }
        }
    }
    impl Drop for NativePlayer {
        fn drop(&mut self) {
            unsafe {
                msg_void(self.obj, "stop");
                msg_void(self.obj, "release");
            }
        }
    }

    /// Tell macOS this app is doing live, time-critical work so it never slows or "naps" it.
    pub fn disable_app_nap() {
        unsafe {
            let pool = msg_id(msg_id(class("NSAutoreleasePool"), "alloc"), "init");
            let info = msg_id(class("NSProcessInfo"), "processInfo");
            let reason = nsstring("RhemaFlow-D is presenting live video and sound");
            // NSActivityUserInitiated (0x00FFFFFF) | NSActivityLatencyCritical (0xFF00000000)
            let opts: u64 = 0x00FF_FFFF | 0xFF_0000_0000;
            let f: unsafe extern "C" fn(Id, Sel, u64, Id) -> Id = std::mem::transmute(send_ptr());
            let token = f(info, sel("beginActivityWithOptions:reason:"), opts, reason);
            if !token.is_null() {
                msg_id(token, "retain"); // keep it for the life of the program
            }
            msg_void(pool, "drain");
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct NativePlayer;
    impl NativePlayer {
        pub fn open(_path: &str) -> Option<NativePlayer> {
            None
        }
        pub fn play(&self) -> bool {
            false
        }
        pub fn pause(&self) {}
        pub fn is_playing(&self) -> bool {
            false
        }
        pub fn set_time_secs(&self, _t: f64) {}
        pub fn time_secs(&self) -> f64 {
            0.0
        }
        pub fn duration_secs(&self) -> f64 {
            0.0
        }
        pub fn set_volume(&self, _v: f32) {}
    }
    pub fn disable_app_nap() {}
}

pub use imp::{disable_app_nap, NativePlayer};
