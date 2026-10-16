#!/bin/bash
# Build a COMPILE-AND-TEST workspace for the desktop app on a machine without a screen or the macOS
# toolchain (e.g. a chat sandbox with Rust 1.75). It swaps the real `eframe` window shell for a tiny
# stub (the app logic and the real `egui` are still compiled and tested), drops the native whisper
# build, and adds a headless layout probe that renders the real UI and reports where text is painted.
#
#   bash tools/sandbox-check/setup.sh /path/to/project /tmp/rf-check
#   cd /tmp/rf-check && cargo test -p rf-app -p rf-core -p rf-bible -p rf-service -p rf-media -p rf-strongs --ignore-rust-version
#   cargo test -p rf-app --ignore-rust-version probe -- --nocapture      # layout probe
#
# On your own Mac with a recent Rust you do NOT need this: just `cargo test` in the project.
set -e
SRC="${1:?project folder}"; OUT="${2:-/tmp/rf-check}"
HERE="$(cd "$(dirname "$0")" && pwd)"
rm -rf "$OUT" && mkdir -p "$OUT"
cp -r "$SRC/crates" "$OUT/crates"
cp "$SRC/Cargo.toml" "$OUT/Cargo.toml"
cp -r "$HERE/eframe-stub" "$OUT/eframe"
[ -f "$SRC/Cargo.lock" ] && cp "$SRC/Cargo.lock" "$OUT/Cargo.lock" && sed -i '0,/^version = 4/s//version = 3/' "$OUT/Cargo.lock"
sed -i 's|members = \[|members = [\n    "eframe",|' "$OUT/Cargo.toml"
sed -i 's|^eframe = .*|eframe = { path = "../../eframe" }|' "$OUT/crates/rf-app/Cargo.toml"
sed -i 's|rf-asr = { path = "../rf-asr" }|rf-asr = { path = "../rf-asr", default-features = false }|' "$OUT/crates/rf-app/Cargo.toml"
cp "$HERE/layout_probe.rs" "$OUT/crates/rf-app/src/layout_probe.rs"
cp "$HERE/command_probe.rs" "$OUT/crates/rf-app/src/command_probe.rs"
cp "$HERE/order_probe.rs" "$OUT/crates/rf-app/src/order_probe.rs"
cp "$HERE/ux_probe.rs" "$OUT/crates/rf-app/src/ux_probe.rs"
cp "$HERE/shot_probe.rs" "$OUT/crates/rf-app/src/shot_probe.rs"
cp "$HERE/v0831_probe.rs" "$OUT/crates/rf-app/src/v0831_probe.rs"
cp "$HERE/v0841_probe.rs" "$OUT/crates/rf-app/src/v0841_probe.rs"
sed -i 's/^mod versions;/mod versions;\n#[cfg(test)]\nmod layout_probe;\n#[cfg(test)]\nmod command_probe;\n#[cfg(test)]\nmod order_probe;\n#[cfg(test)]\nmod ux_probe;\n#[cfg(test)]\nmod shot_probe;\n#[cfg(test)]\nmod v0831_probe;\n#[cfg(test)]\nmod v0841_probe;/' "$OUT/crates/rf-app/src/main.rs"
echo "ready: $OUT   (run cargo with --ignore-rust-version if your Rust is older than 1.76)"
