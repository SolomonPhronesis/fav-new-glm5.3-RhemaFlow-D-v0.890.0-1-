# Third-party software in RhemaFlow-D

*(`docs/LEGAL-AND-CLEAN-ROOM.md` asks for this list. v0.815.0.)*

## Added in Stage 2 (audited: this is the full dependency tree of `rf-media` and `rf-service`)

Every one of these is MIT, Apache-2.0, BSD-2-Clause, Zlib, 0BSD or Unlicense. **None is GPL, AGPL, LGPL or MPL.** Checked with `cargo tree --format "{p}|{l}"` on 7 Oct 2026.

| Crate and version | Licence |
|---|---|
| adler2 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| bitflags 1.3.2 | MIT/Apache-2.0 |
| bytemuck 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| byteorder 1.5.0 | Unlicense OR MIT |
| bytes 1.12.1 | MIT |
| cfg-if 1.0.5 | MIT OR Apache-2.0 |
| color_quant 1.1.0 | MIT |
| crc32fast 1.5.2 | MIT OR Apache-2.0 |
| fdeflate 0.3.7 | MIT OR Apache-2.0 |
| flate2 1.1.10 | MIT OR Apache-2.0 |
| gif 0.13.3 | MIT OR Apache-2.0 |
| image 0.24.9 | MIT OR Apache-2.0 |
| itoa 1.0.18 | MIT OR Apache-2.0 |
| jpeg-decoder 0.3.2 | MIT OR Apache-2.0 |
| memchr 2.8.3 | Unlicense OR MIT |
| miniz_oxide 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| mp4 0.14.0 | MIT |
| num-bigint 0.4.8 | MIT OR Apache-2.0 |
| num-integer 0.1.47 | MIT OR Apache-2.0 |
| num-rational 0.4.2 | MIT OR Apache-2.0 |
| num-traits 0.2.19 | MIT OR Apache-2.0 |
| openh264 0.5.0 | BSD-2-Clause |
| openh264-sys2 0.5.0 | BSD-2-Clause |
| png 0.17.16 | MIT OR Apache-2.0 |
| proc-macro2 1.0.107 | MIT OR Apache-2.0 |
| quote 1.0.47 | MIT OR Apache-2.0 |
| serde 1.0.229 | MIT OR Apache-2.0 |
| serde_core 1.0.229 | MIT OR Apache-2.0 |
| serde_derive 1.0.229 | MIT OR Apache-2.0 |
| serde_json 1.0.151 | MIT OR Apache-2.0 |
| simd-adler32 0.3.10 | MIT |
| syn 2.0.119 | MIT OR Apache-2.0 |
| syn 3.0.6 | MIT OR Apache-2.0 |
| thiserror 1.0.69 | MIT OR Apache-2.0 |
| thiserror-impl 1.0.69 | MIT OR Apache-2.0 |
| unicode-ident 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| weezl 0.1.12 | MIT OR Apache-2.0 |
| zip 0.6.6 | MIT |
| zmij 1.0.23 | MIT |

What the main ones are for:

* **openh264 / openh264-sys2** (BSD-2-Clause): turns H.264 video into pictures. This is Cisco's OpenH264 *source code*, compiled by us.
* **mp4** (MIT): reads the MP4 file (finds the picture track and the frame times).
* **image** with png, jpeg-decoder, gif, bmp (MIT or Apache-2.0): opens pictures.
* **zip** (MIT): opens `.pptx` files (a PowerPoint file is a zip file of XML). Deflate only.

### The H.264 patent note (read before selling)

The BSD licence covers the *code*. H.264 is also covered by *patents* held by many companies and licensed through a pool run by Via LA. What I found on 7 Oct 2026 (web search; **please confirm with Via LA and a lawyer**):

* Via LA's published AVC/H.264 terms: first 100,000 units a year US$0, then US$0.20 each up to 5 million.
* Cisco pays the licence for the **ready-made OpenH264 file Cisco itself distributes**, if you use that file and show the words: **"OpenH264 Video Codec provided by Cisco Systems, Inc."**.
* People who **compile OpenH264 from source** (which this build does, so that it works anywhere offline) are told to arrange their own licence.

Two ways to be safe: (a) sign up with Via LA for the free first 100,000 units; or (b) change the one line `OpenH264API::from_source()` in `crates/rf-media/src/video.rs` to load Cisco's downloadable file, and keep the credit line. The credit line is already shown at the bottom of the Pictures & video tab.

## Added in v0.816.0: Strong's dictionary data

* **Greek dictionary (built in, `crates/rf-strongs/data/greek.json`, 5,624 entries): CC0 1.0 (public-domain dedication).** Strong's Greek Dictionary in XML, Ulrik Sandborg-Petersen, github.com/morphgnt/strongs-dictionary-xml, from James Strong's 1890 concordance. Converted by `tools/make_strongs_greek.py`. (README of that repository read 8 Oct 2026.)
* The OpenScriptures copy of the Greek dictionary is marked **CC-BY-SA** (share-alike) in its own file header, so it was **not** used.
* **Hebrew dictionary and a Strong's-tagged KJV are NOT in the zip.** I could not confirm a clear licence for the Hebrew file, and could not download a tagged KJV from my build machine. `tools/fetch_strongs.py` installs them on the church's own computer; see `docs/STRONGS.md`. Not legal advice.
* New Rust crate `rf-strongs` uses only `serde` and `serde_json` (MIT / Apache-2.0).

## Sound: macOS `afplay` (not a library)

Video sound is played by `afplay`, a program that is part of macOS. We call it; we do not ship it. We did **not** use `symphonia` or `rodio` (MPL-2.0, and rodio uses symphonia) because MPL is not on the allowed list.

## Dependencies from before Stage 2: NOT yet audited

`egui`/`eframe`, `cpal`, `whisper-rs`/whisper.cpp, `hound`, `crossbeam-channel`, `serde`, `anyhow` and their own dependencies were chosen in earlier versions and are listed in `Cargo.lock`. I did not audit their licences in this stage. Before you sell the app, run `cargo install cargo-deny` and then `cargo deny check licenses` (or make it part of Stage 6).

## Other content

* Bibles in `bibles/` are public-domain texts (see `docs/BIBLE-VERSIONS.md`). **No licensed Bible (NIV, NKJV, AMP, ESV, NLT) is included.**
* Font: Liberation Serif 2.1.5 (the file itself says SIL Open Font License 1.1), in `crates/rf-app/assets/`.
* No song words, pictures or videos are included. The test clips in `crates/rf-media/tests/fixtures/` are tiny plain-colour frames made for testing (with PyAV: x264 and x265); they contain nothing copyrighted.

## Added in v0.835.0
- **OpenDyslexic** font (SIL Open Font License 1.1) - `crates/rf-app/assets/`, licence text beside it.
- **Atkinson Hyperlegible** font (SIL Open Font License 1.1) - same place.
- `qrcode` (MIT/Apache-2.0) - QR codes for the giving card.
- `midir` (MIT) - MIDI input.
- `zip` 0.6 (MIT) - backup files.
Not yet audited transitive dependencies of these three; run the same audit as for Stage 2 before selling.

## Added in 0.839.0

| Crate | Licence | Why |
|---|---|---|
| muda 0.15 (macOS only) and its own dependencies | MIT OR Apache-2.0 | the real Mac menu bar at the top of the screen |

The in-process sound player uses macOS's own AVFoundation through the Objective-C runtime (no crate, no extra licence).
