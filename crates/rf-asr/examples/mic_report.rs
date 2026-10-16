//! Tell me what is wrong with the SOUND of a recording.
//!   cargo run --release -p rf-asr --example mic_report -- /path/to/sunday.wav
//! Any WAV file works (any sample rate, mono or stereo). Record 30 seconds of the preacher speaking
//! from the real microphone position, at the normal volume, and run this on it.
fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: mic_report <recording.wav>");
        std::process::exit(2);
    };
    match rf_asr::wav::load_16k_mono(&path) {
        Ok(samples) => {
            let r = rf_asr::micreport::analyze(&samples, 16_000);
            print!("{}", r.to_text());
            std::process::exit(match r.verdict {
                rf_asr::micreport::Level::Good => 0,
                rf_asr::micreport::Level::Warn => 1,
                rf_asr::micreport::Level::Bad => 3,
            });
        }
        Err(e) => {
            eprintln!("Could not read {path}: {e}");
            std::process::exit(2);
        }
    }
}
