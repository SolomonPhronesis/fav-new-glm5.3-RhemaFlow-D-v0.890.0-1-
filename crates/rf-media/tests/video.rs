//! Spec S3.1, S3.7, S3.8, S3.10 for video, on a REAL clip made by a different encoder (x264):
//! 128x72, 60 frames at 30 fps, B-frames on, plus an AAC sound track.
//! Frame i is flat colour red = 4*i, green = 80, blue = 255 - 4*i, so the colour tells us which frame we got.
use rf_media::VideoReader;

fn clip() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_bframes_audio.mp4")).unwrap()
}

fn middle_pixel(f: &rf_media::Frame) -> [u8; 4] {
    let (w, h) = (f.image.width as usize, f.image.height as usize);
    let o = ((h / 2) * w + w / 2) * 4;
    [f.image.pixels[o], f.image.pixels[o + 1], f.image.pixels[o + 2], f.image.pixels[o + 3]]
}

#[test]
fn s3_8_the_picture_track_is_found_even_though_the_file_also_has_a_sound_track() {
    let r = VideoReader::from_bytes(clip()).expect("opens");
    let i = r.info();
    assert_eq!((i.width, i.height, i.frame_count), (128, 72, 60));
    assert!((1900..=2100).contains(&i.duration_ms), "duration {}", i.duration_ms);
    assert!((28.0..=32.0).contains(&i.fps), "fps {}", i.fps);
}

#[test]
fn s3_7_all_frames_come_out_in_display_order_with_the_right_picture() {
    let mut r = VideoReader::from_bytes(clip()).unwrap();
    let mut frames = Vec::new();
    while let Some(f) = r.next_frame(true).expect("no decode error") {
        frames.push(f);
    }
    assert_eq!(frames.len(), 60, "every frame must come out");
    assert_eq!(frames[0].pts_ms, 0);
    for w in frames.windows(2) {
        assert!(w[1].pts_ms > w[0].pts_ms, "display times must go up: {} then {}", w[0].pts_ms, w[1].pts_ms);
    }
    assert!((1900..=2000).contains(&frames[59].pts_ms), "last frame at {}", frames[59].pts_ms);
    for (i, f) in frames.iter().enumerate() {
        assert_eq!((f.image.width, f.image.height), (128, 72));
        assert_eq!(f.image.pixels.len(), 128 * 72 * 4);
        let [red, green, blue, alpha] = middle_pixel(f);
        let want = (i * 4) as i32;
        assert!((red as i32 - want).abs() <= 18, "frame {i}: red {red}, wanted about {want} (frames out of order?)");
        assert!((blue as i32 - (255 - want)).abs() <= 18, "frame {i}: blue {blue}");
        assert!((green as i32 - 80).abs() <= 18, "frame {i}: green {green}");
        assert_eq!(alpha, 255);
    }
}

#[test]
fn s3_4_skipping_the_picture_still_decodes_every_frame() {
    let mut r = VideoReader::from_bytes(clip()).unwrap();
    let mut n = 0;
    while let Some(f) = r.next_frame(false).unwrap() {
        assert!(f.image.pixels.is_empty(), "no pixels were asked for");
        n += 1;
    }
    assert_eq!(n, 60);
}

#[test]
fn s3_6_rewind_goes_back_to_the_first_frame() {
    let mut r = VideoReader::from_bytes(clip()).unwrap();
    let first = r.next_frame(true).unwrap().unwrap();
    for _ in 0..30 {
        r.next_frame(false).unwrap();
    }
    assert!(r.peek_pts().unwrap() > 900);
    r.rewind().unwrap();
    assert_eq!(r.peek_pts(), Some(0));
    let again = r.next_frame(true).unwrap().unwrap();
    assert_eq!(again.pts_ms, 0);
    assert_eq!(middle_pixel(&first), middle_pixel(&again));
}

#[test]
fn s3_10_not_an_mp4_is_a_plain_error_with_advice() {
    let e = VideoReader::from_bytes(b"RIFF....AVI LIST not an mp4 at all".to_vec()).err().expect("must fail");
    assert!(e.contains("MP4") && e.to_lowercase().contains("handbrake"), "{e}");
    assert!(VideoReader::from_bytes(Vec::new()).is_err());
}

#[test]
fn s3_10_a_cut_off_file_never_crashes() {
    let full = clip();
    for keep in [full.len() / 10, full.len() / 2, full.len() - 500] {
        let cut = full[..keep].to_vec();
        match VideoReader::from_bytes(cut) {
            Err(e) => assert!(!e.is_empty()),
            Ok(mut r) => {
                // opening can work if the index survived; reading must then end in Err or fewer frames, never a panic
                let mut n = 0;
                loop {
                    match r.next_frame(false) {
                        Ok(Some(_)) => n += 1,
                        Ok(None) | Err(_) => break,
                    }
                }
                assert!(n <= 60);
            }
        }
    }
}

#[test]
fn s3_10_a_missing_file_is_a_plain_error() {
    let e = VideoReader::open(std::path::Path::new("/no/such/video.mp4")).err().unwrap();
    assert!(e.contains("Cannot find"), "{e}");
}

#[test]
fn s3_1_a_real_h265_file_is_refused_with_advice_instead_of_playing_garbage() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_hevc.mp4")).unwrap();
    let e = VideoReader::from_bytes(bytes).err().expect("HEVC cannot be played");
    assert!(e.contains("H.265") && e.to_lowercase().contains("handbrake"), "{e}");
}

// ---- v0.822.0: a sound track the parser cannot read must not stop the picture from opening

/// Make the fixture's sound-track header unreadable the way real files do: a box inside the
/// `mp4a` entry that claims to be bigger than the entry itself.
fn with_broken_sound_header() -> Vec<u8> {
    let mut b = clip();
    let at = b.windows(4).position(|w| w == b"mp4a").expect("the clip has a sound track");
    // find the 'esds' box after mp4a and blow up its declared size
    let esds = at + b[at..].windows(4).position(|w| w == b"esds").unwrap() - 4;
    b[esds..esds + 4].copy_from_slice(&0x7fff_ffffu32.to_be_bytes());
    b
}

#[test]
fn a_broken_sound_header_stops_the_plain_parser() {
    let bytes = with_broken_sound_header();
    let size = bytes.len() as u64;
    let plain = mp4::Mp4Reader::read_header(std::io::Cursor::new(bytes), size);
    assert!(plain.is_err(), "the damage must be real, or this test proves nothing");
}

#[test]
fn the_picture_still_opens_and_plays_when_only_the_sound_header_is_broken() {
    let mut r = VideoReader::from_bytes(with_broken_sound_header()).expect("picture opens without the sound track");
    assert_eq!((r.info().width, r.info().height, r.info().frame_count), (128, 72, 60));
    let mut n = 0;
    while r.next_frame(true).unwrap().is_some() { n += 1; }
    assert_eq!(n, 60);
}

#[test]
fn a_file_on_disk_is_repaired_while_reading_and_left_untouched() {
    let dir = std::env::temp_dir().join(format!("rf-mp4fix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("broken-sound.mp4");
    let bytes = with_broken_sound_header();
    std::fs::write(&path, &bytes).unwrap();
    assert!(VideoReader::open(&path).is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "the file on disk is never changed");
}

#[test]
fn only_non_video_tracks_are_marked_and_a_good_file_needs_no_patches() {
    let good = clip();
    let p = rf_media::mp4fix::non_video_track_patches(&mut std::io::Cursor::new(&good)).unwrap();
    assert_eq!(p.len(), 1, "exactly the sound track");
    assert!((p[0].0 as usize) + 4 <= good.len(), "the patch lies inside the file");
    assert!(VideoReader::from_bytes(good).is_ok());
    assert!(rf_media::mp4fix::non_video_track_patches(&mut std::io::Cursor::new(b"not an mp4 at all".to_vec())).unwrap().is_empty());
}

// ---- v0.833.0: jumping in a long video must not decode everything from the start ----

fn picture_sum(f: &rf_media::Frame) -> u64 {
    f.image.pixels.iter().enumerate().fold(0u64, |a, (i, &b)| a.wrapping_add((b as u64).wrapping_mul(i as u64 % 251 + 1)))
}

#[test]
fn a_jump_lands_on_exactly_the_picture_normal_playing_shows() {
    let path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_seek_20s.mp4"));
    // every picture, played from the start
    let mut seq = VideoReader::open(&path).unwrap();
    let mut all = std::collections::HashMap::new();
    while let Some(f) = seq.next_frame(true).unwrap() {
        all.insert(f.pts_ms, picture_sum(&f));
    }
    assert!(all.len() > 400, "the 20 s clip has about 500 pictures");
    // jumps forward, backward, to a keyframe, between keyframes, to the very start and near the end
    let mut r = VideoReader::open(&path).unwrap();
    for target in [13_000u64, 1_000, 7_480, 19_000, 0, 10_020, 400, 19_900] {
        r.seek_to(target).unwrap();
        while let Some(p) = r.peek_pts() {
            if p >= target {
                break;
            }
            r.next_frame(false).unwrap();
        }
        // like the player: a picture decoded while skipping has no pixels, so take the first one that has
        let f = loop {
            let f = r.next_frame(true).unwrap().expect("a picture after the jump");
            if !f.image.pixels.is_empty() {
                break f;
            }
        };
        assert!(f.pts_ms >= target && f.pts_ms < target + 200, "target {target}: got {}", f.pts_ms);
        assert_eq!(all.get(&f.pts_ms), Some(&picture_sum(&f)), "the picture at {} after a jump to {target} is the same as when played from the start", f.pts_ms);
        // and playing on from there stays in step
        // (a neighbour that was decoded while skipping has no pixels: that is how the player skips cheaply)
        for _ in 0..30 {
            let Some(g) = r.next_frame(true).unwrap() else { break };
            assert!(g.pts_ms > f.pts_ms);
            if !g.image.pixels.is_empty() {
                assert_eq!(all.get(&g.pts_ms), Some(&picture_sum(&g)), "playing on after a jump to {target} stays in step");
            }
        }
    }
}

#[test]
fn a_jump_decodes_only_one_keyframe_stretch_not_the_whole_file() {
    let path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_seek_20s.mp4"));
    let mut r = VideoReader::open(&path).unwrap();
    let t0 = std::time::Instant::now();
    r.seek_to(19_000).unwrap();
    let mut skipped = 0;
    while let Some(p) = r.peek_pts() {
        if p >= 19_000 {
            break;
        }
        r.next_frame(false).unwrap();
        skipped += 1;
    }
    assert!(skipped <= 30, "decoded {skipped} pictures to reach minute 0:19 (it used to be about 475)");
    assert!(t0.elapsed().as_millis() < 2_000);
}
