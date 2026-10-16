# sandbox-check

Lets a chat (or CI) compile and test the desktop app without a Mac and without a display.
`setup.sh` copies the project, replaces `eframe` (the window shell) with a 20-line stub, and adds
`layout_probe.rs`, a test that runs real frames of the real UI at 1190x770 / 1000x640 / 1440x900
and prints where every text was painted (books must be a vertical list, nothing outside the window).

What it does NOT prove: how it looks on a real screen, real second monitors, macOS audio, whisper.
Always say so, and ask the user to try those on their Mac.

Other checks: `node --test web/test/core.test.js`; `NODE_PATH=<jsdom> node --test web/test/dom.test.js`;
`python3 web/test/browser_test.py` (real Chromium via Playwright); `python3 web/build.py` rebuilds the web page.

v0.815.0: the crate list is now `-p rf-app -p rf-core -p rf-bible -p rf-service -p rf-media`. `rf-media` compiles Cisco's
OpenH264 C++ source, so it needs a C++ compiler (g++/clang) and, on Linux, `libasound2-dev` for the audio crate. On a chat
sandbox with Rust 1.75 the newest `openh264` (0.6) does not compile; the project pins `0.5` (fine on current Rust too).
`cargo run --release -p rf-media --example decode_speed -- some.mp4` measures decode speed on the machine you run it on.

v0.816.0: add `-p rf-strongs` to the crate list. `command_probe.rs` (copied in by setup.sh like `layout_probe.rs`) builds the REAL app and speaks
to it with `handle_parse`: it replays the operator's nine garbled pieces at their real timestamps and exercises every new voice command.
It needs the stub eframe, so it only runs in the sandbox. Run: `cargo test -p rf-app --ignore-rust-version command_probe -- --nocapture`.


v0.830.0: `shot_probe.rs` renders the real UI to /tmp/rf-shots/*.ppm (software rasterizer over egui's own triangles and font atlas). Convert with PIL (`Image.open(p).save("x.png")`). Rough: nearest-neighbour text, missing emoji glyphs show as boxes. Good for SEEING layout problems; NOT a Mac screenshot.
