//! Raw FFI to the two C++ shims in `native/`. Opaque handles + primitives only.
//! (Compiled only with the `native` feature.)

use std::os::raw::{c_char, c_float, c_int, c_longlong};

#[repr(C)]
pub struct RfWhisper {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RfParakeet {
    _private: [u8; 0],
}

extern "C" {
    pub fn rf_whisper_load(model_path: *const c_char, use_gpu: c_int, err: *mut c_char, err_len: usize) -> *mut RfWhisper;
    pub fn rf_whisper_free(w: *mut RfWhisper);
    pub fn rf_whisper_transcribe(
        w: *mut RfWhisper,
        samples: *const c_float,
        n_samples: c_int,
        initial_prompt: *const c_char,
        beam_size: c_int,
        n_threads: c_int,
    ) -> c_int;
    pub fn rf_whisper_segment_text(w: *const RfWhisper, i: c_int) -> *const c_char;
    pub fn rf_whisper_segment_t0_ms(w: *const RfWhisper, i: c_int) -> c_longlong;
    pub fn rf_whisper_segment_t1_ms(w: *const RfWhisper, i: c_int) -> c_longlong;

    pub fn rf_parakeet_load(model_path: *const c_char, use_gpu: c_int, err: *mut c_char, err_len: usize) -> *mut RfParakeet;
    pub fn rf_parakeet_free(p: *mut RfParakeet);
    pub fn rf_parakeet_transcribe(p: *mut RfParakeet, samples: *const c_float, n_samples: c_int, n_threads: c_int) -> c_int;
    pub fn rf_parakeet_segment_text(p: *const RfParakeet, i: c_int) -> *const c_char;
    pub fn rf_parakeet_segment_t0_ms(p: *const RfParakeet, i: c_int) -> c_longlong;
    pub fn rf_parakeet_segment_t1_ms(p: *const RfParakeet, i: c_int) -> c_longlong;
}
