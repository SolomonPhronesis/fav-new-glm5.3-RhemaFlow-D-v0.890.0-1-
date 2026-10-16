//! Builds the native speech engines: whisper.cpp (Whisper) and the Parakeet
//! engine that ships in the same repository, sharing ONE copy of ggml.
//!
//! Why one shared build: linking two separately-built ggml copies into a single
//! binary risks duplicate symbols or silently mismatched ABIs. Here CMake builds
//! `whisper`, `parakeet` and `ggml` together from one pinned source tree.
//!
//! Source tree resolution, in order:
//!   1. $RF_WHISPER_CPP_DIR (a checkout you supply)
//!   2. <workspace>/third_party/whisper.cpp-<sha12>  (vendored in the release zip)
//!   3. downloaded from GitHub at the pinned commit into (2)
//!
//! The Rust code never declares whisper's large parameter structs. Two tiny C++
//! shims (native/*.cpp) are compiled against the REAL headers and expose only
//! pointers, ints and C strings across the FFI boundary.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WHISPER_CPP_SHA: &str = "307869af285d7f6f689ba100b3515e2d1b3feb05";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-env-changed=RF_WHISPER_CPP_DIR");
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return; // mock-only build: nothing native to compile
    }
    let metal = env::var_os("CARGO_FEATURE_METAL").is_some();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let vendor = vendor_dir(&manifest);
    println!("cargo:rerun-if-changed={}", vendor.join("CMakeLists.txt").display());

    // --- 1. whisper + parakeet + ggml via CMake (static) --------------------
    let mut cfg = cmake::Config::new(&vendor);
    cfg.profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("WHISPER_BUILD_TESTS", "OFF")
        .define("WHISPER_BUILD_EXAMPLES", "OFF")
        .define("WHISPER_BUILD_SERVER", "OFF")
        .define("GGML_BUILD_TESTS", "OFF")
        .define("GGML_BUILD_EXAMPLES", "OFF")
        .define("GGML_OPENMP", "OFF")
        .define("GGML_CCACHE", "OFF")
        // ggml turns Metal ON by default on Apple targets — be explicit so the
        // default build stays on the CPU path.
        .define("GGML_METAL", if metal { "ON" } else { "OFF" });
    if metal {
        cfg.define("GGML_METAL_EMBED_LIBRARY", "ON");
    }
    let dst = cfg.build();

    // --- 2. the C++ shims, compiled against the real vendored headers -------
    let mut shim = cc::Build::new();
    shim.cpp(true)
        .flag_if_supported("-std=c++17")
        .warnings(false)
        .include(vendor.join("include"))
        .include(vendor.join("ggml").join("include"))
        .file(manifest.join("native/rf_whisper_shim.cpp"))
        .file(manifest.join("native/rf_parakeet_shim.cpp"));
    // compile() emits the link directive for the shim; it must come BEFORE the
    // libraries it depends on so single-pass linkers resolve them.
    shim.compile("rf_native");

    // --- 3. link the static libs CMake produced, dependents first -----------
    let mut libs: Vec<(u8, String, PathBuf)> = Vec::new();
    collect_libs(&dst, &mut libs);
    libs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    libs.dedup_by(|a, b| a.1 == b.1);
    let mut dirs: Vec<PathBuf> = libs.iter().map(|l| l.2.clone()).collect();
    dirs.sort();
    dirs.dedup();
    for d in &dirs {
        println!("cargo:rustc-link-search=native={}", d.display());
    }
    for (_, name, _) in &libs {
        println!("cargo:rustc-link-lib=static={name}");
    }
    if libs.iter().all(|l| l.1 != "parakeet") || libs.iter().all(|l| l.1 != "whisper") {
        panic!("rf-asr build: CMake did not produce libwhisper.a and libparakeet.a under {}", dst.display());
    }

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=Accelerate");
        if metal {
            for f in ["Foundation", "Metal", "MetalKit"] {
                println!("cargo:rustc-link-lib=framework={f}");
            }
        }
    }
}

/// Link order rank: dependents before dependencies.
fn rank(name: &str) -> Option<u8> {
    match name {
        "whisper" => Some(0),
        "parakeet" => Some(1),
        "ggml" => Some(2),
        "ggml-metal" | "ggml-blas" => Some(3),
        "ggml-cpu" => Some(4),
        "ggml-base" => Some(5),
        n if n.starts_with("ggml") => Some(3),
        _ => None,
    }
}

fn collect_libs(dir: &Path, out: &mut Vec<(u8, String, PathBuf)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect_libs(&p, out);
        } else if let (Some(stem), Some(ext)) = (p.file_stem().and_then(|s| s.to_str()), p.extension()) {
            if ext == "a" {
                if let Some(name) = stem.strip_prefix("lib") {
                    if let Some(r) = rank(name) {
                        out.push((r, name.to_string(), p.parent().unwrap().to_path_buf()));
                    }
                }
            }
        }
    }
}

fn check_tree(p: &Path) -> bool {
    p.join("include/whisper.h").exists() && p.join("include/parakeet.h").exists()
}

fn vendor_dir(manifest: &Path) -> PathBuf {
    if let Some(p) = env::var_os("RF_WHISPER_CPP_DIR") {
        let p = PathBuf::from(p);
        if !check_tree(&p) {
            panic!("RF_WHISPER_CPP_DIR={} lacks include/whisper.h + include/parakeet.h", p.display());
        }
        return p;
    }
    let root = manifest.join("..").join("..");
    let dir = root.join("third_party").join(format!("whisper.cpp-{}", &WHISPER_CPP_SHA[..12]));
    if !check_tree(&dir) {
        download(&dir);
    }
    dir
}

fn run(cmd: &mut Command, what: &str) {
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("{what} failed ({s})"),
        Err(e) => panic!("{what} could not start: {e}"),
    }
}

fn download(dir: &Path) {
    let url = format!("https://codeload.github.com/ggerganov/whisper.cpp/tar.gz/{WHISPER_CPP_SHA}");
    let parent = dir.parent().unwrap();
    fs::create_dir_all(parent).expect("create third_party/");
    let tmp = parent.join(".whisper.cpp-download.tar.gz");
    let staging = parent.join(".whisper.cpp-staging");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).expect("create staging dir");
    eprintln!("rf-asr: downloading whisper.cpp @ {} …", &WHISPER_CPP_SHA[..12]);
    run(
        Command::new("curl").args(["-fsSL", "--retry", "3", "-o"]).arg(&tmp).arg(&url),
        &format!("downloading {url} (no network? set RF_WHISPER_CPP_DIR to a checkout instead)"),
    );
    run(
        Command::new("tar").arg("-xzf").arg(&tmp).arg("-C").arg(&staging).args(["--strip-components", "1"]),
        "unpacking whisper.cpp",
    );
    if !check_tree(&staging) {
        panic!("downloaded whisper.cpp tree is missing include/parakeet.h — wrong commit?");
    }
    let _ = fs::remove_dir_all(dir);
    fs::rename(&staging, dir).expect("move whisper.cpp into third_party/");
    let _ = fs::remove_file(&tmp);
}
