//! RhemaFlow-D v0.831.0 — entry point.
//!
//! The preacher speaks. RhemaFlow understands.

mod account;
mod actions;
mod app;
mod benchmark;
mod evalset;
mod browser;
mod clip;
mod ccli;
mod calibrate;
mod displays;
mod docview;
mod export;
mod naming;
mod order;
mod preflight;
mod quit_guard;
mod failover;
mod history;
#[allow(dead_code)] // rules only for now; connected to the server in a later stage
mod access;
mod announce;
mod controllers;
mod screens;
mod backup;
mod lock;
mod handout;
mod recovery;
mod clips;
mod readalong;
mod translate;
mod autoplan;
mod runsheet;
mod fsutil;
mod license;
mod themes;
mod lyrics;
mod macaudio;
mod macmenu;
mod overlay;
mod extras;
mod extras_ui;
mod studio;
mod studio_ui;
#[cfg(test)]
mod latency_sim;
mod partial;
mod present;
mod recorder;
mod scrub;
mod selection;
mod sound;
mod strongs_ui;
mod stage;
mod stage_extras;
mod state;
mod stream_bridge;
mod theme;
mod tts;
mod versions;
mod web;
mod ui;

use app::RhemaFlowApp;
use state::AsrMode;

const ABOUT: &str = "\
RhemaFlow-D v0.831.0 — intelligent church presentation (offline, local-first)

USAGE:
  rhemaflow-d                launch (auto-detects the speech model)
  rhemaflow-d --engine E     speech engine: whisper (default) | parakeet
  rhemaflow-d --model PATH   use a specific ggml model (engine guessed from the name)
  rhemaflow-d --stream-url auto   start the R2T2 sidecar for you (if installed), Whisper as backup
  rhemaflow-d --stream-url U use a local streaming service (Confucius4-R2T2 protocol), e.g.
                             ws://127.0.0.1:8272/asr_stream_api_v1  (see docs/R2T2-ON-M1.md)
  rhemaflow-d benchmark WAV  compare speech engines on one audio file (see --help there)
  rhemaflow-d evalset FOLDER score the engines on a folder of real clips + what was said (see docs/EVALSET.md)
  rhemaflow-d calibrate      read 30 sentences: measures YOUR word accuracy and learns your voice
  rhemaflow-d --demo         built-in demo ASR (no model needed)
  rhemaflow-d --no-asr       no speech recognition (simulator + meter only)
  rhemaflow-d --church NAME  church name shown on the stage window
  rhemaflow-d --voices       list installed TTS voices (for read-aloud) and exit
";

fn main() -> Result<(), eframe::Error> {
    quit_guard::install(); // a frozen window can still be closed politely: players are stopped first
    let args: Vec<String> = std::env::args().collect();

    // --voices exits before arg parsing (pure CLI utility, no GUI needed)
    if args.iter().any(|a| a == "--voices") {
        println!("Installed TTS voices (natural = Premium/Enhanced):\n");
        for v in tts::list_voices() {
            let nat = if v.is_natural() { "  * natural" } else { "" };
            println!("  {:<24} {:<8} {}{}", v.id, v.locale, v.label(), nat);
        }
        println!("\nMore natural voices (free, offline after download):");
        println!("  System Settings → Accessibility → Spoken Content → System Voice → Manage Voices…");
        return Ok(());
    }

    // `evalset` scores a folder of real clips (v0.851.0)
    if args.get(1).map(String::as_str) == Some("evalset") {
        if let Err(e) = evalset::run(&args[2..]) {
            eprintln!("evalset: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // `benchmark` is a pure CLI tool: no GUI, no microphone
    if args.get(1).map(String::as_str) == Some("benchmark") {
        if let Err(e) = benchmark::run(&args[2..]) {
            eprintln!("benchmark: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    keep_mac_awake();

    // `calibrate` learns this speaker's mishearings and reports word accuracy
    if args.get(1).map(String::as_str) == Some("calibrate") {
        if let Err(e) = calibrate::run(&args[2..]) {
            eprintln!("calibrate: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let mut model: Option<String> = None;
    let mut engine: Option<rf_asr::EngineKind> = None;
    let mut stream_url: Option<String> = None;
    let mut demo = false;
    let mut no_asr = false;
    let mut church = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" => {
                i += 1;
                model = args.get(i).cloned();
            }
            "--engine" => {
                i += 1;
                let v = args.get(i).cloned().unwrap_or_default();
                engine = Some(rf_asr::EngineKind::parse(&v).unwrap_or_else(|| {
                    eprintln!("unknown engine '{v}' (use: whisper | parakeet)");
                    std::process::exit(2);
                }));
            }
            "--stream-url" => {
                i += 1;
                stream_url = args.get(i).cloned();
            }
            "--demo" => demo = true,
            "--no-asr" => no_asr = true,
            "--church" => {
                i += 1;
                church = args.get(i).cloned().unwrap_or_default();
            }
            "--help" | "-h" => {
                println!("{ABOUT}");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}\n{ABOUT}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // model auto-detection (run.sh downloads to ./models/)
    let kind = engine
        .or_else(|| model.as_deref().map(rf_asr::EngineKind::guess_from_model_path))
        .unwrap_or(rf_asr::EngineKind::Whisper);
    if stream_url.as_deref() == Some("auto") {
        stream_url = Some(DEFAULT_STREAM_URL.to_string());
        start_sidecar_if_installed();
    }
    let asr_mode = if let Some(url) = stream_url.clone() {
        AsrMode::Stream(url)
    } else if demo {
        AsrMode::Mock
    } else if no_asr {
        AsrMode::Offline
    } else if !rf_asr::native_compiled() {
        AsrMode::Offline
    } else {
        match find_model(kind, model.as_deref()) {
            Some(p) if kind == rf_asr::EngineKind::Parakeet => AsrMode::Parakeet(p),
            Some(p) => AsrMode::Whisper(p),
            None => AsrMode::Offline,
        }
    };
    match &asr_mode {
        AsrMode::Whisper(p) => println!("[rf-app] whisper model: {p}"),
        AsrMode::Parakeet(p) => println!("[rf-app] parakeet model: {p}"),
        AsrMode::Stream(u) => println!("[rf-app] streaming ASR service: {u}"),
        AsrMode::Mock => println!("[rf-app] demo mode: built-in mock ASR"),
        AsrMode::Offline => {
            println!("[rf-app] no {} model found — running with simulator only.", kind.label());
            match kind {
                rf_asr::EngineKind::Whisper => println!("         run ./run.sh to download the model (or use --demo)."),
                rf_asr::EngineKind::Parakeet => println!("         run ./run.sh model-parakeet to download it (about 1.2 GB)."),
            }
        }
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title(theme::TITLE)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1100.0, 700.0]);

    eframe::run_native(
        theme::TITLE,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            // the real Mac menu bar, and no "App Nap" slowing the picture when this window is behind others
            macmenu::install(cc.egui_ctx.clone());
            macaudio::disable_app_nap();
            // while a video plays, ask for a redraw every frame from a helper thread (the stage is drawn from
            // this window's redraw, so a sleepy window would freeze the picture while the sound played on)
            let wake_ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));
                if present::KEEP_AWAKE.load(std::sync::atomic::Ordering::Relaxed) {
                    wake_ctx.request_repaint();
                }
            });
            Ok(Box::new(RhemaFlowApp::new(cc, asr_mode, church)))
        }),
    )
}

/// Locate a model file for `kind`: an explicit path first, then ./models/ and
/// the repo-relative models/ next to the executable.
/// ClearSpeech final pass + failover standby (v0.843.0): the highest-accuracy
/// LOCAL model available, preferred in this order:
///   1. NVIDIA Parakeet TDT 0.6B  — top of the HF Open ASR Leaderboard (~6%
///      average WER across noisy/reverberant/music test sets), 25–30x real
///      time on M-series, punctuated output. Download: ./run.sh model-parakeet
///   2. Whisper large-v3-turbo    — OpenAI's 680k-hour model, famously robust
///      to noise/music/overlapping voices. Download: ./run.sh model-turbo
///   3. Whisper base.en / tiny.en — always present after a normal ./run.sh
/// `RF_FINAL_MODEL=/path/to/model.bin` overrides (a filename containing
/// "parakeet" selects the Parakeet engine, anything else Whisper).
pub(crate) fn find_final_model() -> Option<(rf_asr::EngineKind, String)> {
    use std::path::Path;
    if let Ok(m) = std::env::var("RF_FINAL_MODEL") {
        if !m.is_empty() && Path::new(&m).exists() {
            let kind = if m.to_lowercase().contains("parakeet") {
                rf_asr::EngineKind::Parakeet
            } else {
                rf_asr::EngineKind::Whisper
            };
            return Some((kind, m));
        }
    }
    if let Some(p) = find_model(rf_asr::EngineKind::Parakeet, None) {
        return Some((rf_asr::EngineKind::Parakeet, p));
    }
    for name in ["ggml-large-v3-turbo-q5_0.bin", "ggml-large-v3-turbo.bin", "ggml-large-v3.bin"] {
        if let Some(p) = find_model_named(name) {
            return Some((rf_asr::EngineKind::Whisper, p));
        }
    }
    find_model(rf_asr::EngineKind::Whisper, None).map(|p| (rf_asr::EngineKind::Whisper, p))
}

/// Find one exact model filename in the same places `find_model` looks.
fn find_model_named(name: &str) -> Option<String> {
    let mut dirs: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from("./models")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("../../models"));
        }
    }
    for dir in dirs {
        let p = dir.join(name);
        if p.exists() {
            return Some(p.display().to_string());
        }
    }
    None
}

pub(crate) fn find_model(kind: rf_asr::EngineKind, explicit: Option<&str>) -> Option<String> {
    use rf_asr::EngineKind;
    if let Some(m) = explicit {
        return std::path::Path::new(m).exists().then(|| m.to_string());
    }
    let mut dirs: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from("./models")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("../../models"));
        }
    }
    for dir in dirs {
        match kind {
            EngineKind::Whisper => {
                for name in ["ggml-tiny.en.bin", "ggml-base.en.bin"] {
                    let p = dir.join(name);
                    if p.exists() {
                        return Some(p.display().to_string());
                    }
                }
            }
            EngineKind::Parakeet => {
                let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("ggml-parakeet") && n.ends_with(".bin"))
                            .unwrap_or(false)
                    })
                    .collect();
                found.sort();
                if let Some(p) = found.into_iter().next() {
                    return Some(p.display().to_string());
                }
            }
        }
    }
    None
}

/// macOS: stop idle sleep, display sleep and disk sleep while RhemaFlow runs, so a quiet
/// moment in the service can never put the Mac (or the projector output) to sleep.
/// `caffeinate -w PID` ends by itself when we exit. Best effort: failure is harmless.
fn keep_mac_awake() {
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id().to_string();
        let _ = std::process::Command::new("caffeinate")
            .args(["-d", "-i", "-m", "-w", &pid])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

const DEFAULT_STREAM_URL: &str = "ws://127.0.0.1:8272/asr_stream_api_v1";

/// `--stream-url auto`: start the R2T2 sidecar for the user if it is installed
/// (`sidecar/setup_mac.sh` creates ~/RhemaFlow-R2T2/run_sidecar.sh) and not already
/// running. The sidecar exits by itself when this app ends (--parent-pid).
fn start_sidecar_if_installed() {
    use std::net::{SocketAddr, TcpStream};
    let already = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], 8272)), std::time::Duration::from_millis(250)).is_ok();
    if already {
        println!("[rf-app] R2T2 sidecar already running on port 8272");
        return;
    }
    let script = std::env::var("R2T2_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("RhemaFlow-R2T2"))
        .join("run_sidecar.sh");
    if !script.exists() {
        println!("[rf-app] R2T2 sidecar not installed ({} missing) - run sidecar/setup_mac.sh; using Whisper meanwhile", script.display());
        return;
    }
    match std::process::Command::new(&script).args(["--parent-pid", &std::process::id().to_string()]).stdin(std::process::Stdio::null()).spawn() {
        Ok(_) => println!("[rf-app] started R2T2 sidecar: {}", script.display()),
        Err(e) => println!("[rf-app] could not start the R2T2 sidecar: {e}"),
    }
}
