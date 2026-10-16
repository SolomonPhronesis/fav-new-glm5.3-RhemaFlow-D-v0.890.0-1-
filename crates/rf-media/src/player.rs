//! A video that plays itself on a background thread (spec S3.4 to S3.6).
//!
//! The thread owns the decoder. It decides which frame is due from a `PlaybackClock` (wall time,
//! not "one frame per loop"), so a slow computer drops frames and stays in time. The newest due
//! frame is parked in shared memory; the screen just asks `latest()` each repaint.

use crate::video::{Frame, VideoInfo, VideoReader};
use rf_service::media::PlaybackClock;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct Shared {
    clock: PlaybackClock,
    latest: Option<Arc<Frame>>,
    serial: u64,
    restart_serial: u64,
    quit: bool,
    error: Option<String>,
}

type Link = Arc<(Mutex<Shared>, Condvar)>;

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub playing: bool,
    pub ended: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub looping: bool,
    /// Set if the file turned out to be damaged part-way through.
    pub error: Option<String>,
}

pub struct Player {
    link: Link,
    start: Instant,
    info: VideoInfo,
    path: PathBuf,
    handle: Option<JoinHandle<()>>,
}

impl Player {
    /// Opens the file on its own thread. Errors in opening come back here, in plain words.
    /// The first picture is ready almost at once; the video starts **paused** at 0:00.
    pub fn open(path: &Path, looping: bool) -> Result<Player, String> {
        let start = Instant::now();
        let (tx, rx) = mpsc::channel::<Result<VideoInfo, String>>();
        let link: Link = Arc::new((
            Mutex::new(Shared { clock: PlaybackClock::new(0, looping), latest: None, serial: 0, restart_serial: 0, quit: false, error: None }),
            Condvar::new(),
        ));
        let link2 = link.clone();
        let p = path.to_path_buf();
        let handle = std::thread::Builder::new()
            .name("rf-video".into())
            .spawn(move || match VideoReader::open(&p) {
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
                Ok(reader) => {
                    let info = reader.info().clone();
                    {
                        let mut s = link2.0.lock().unwrap();
                        s.clock = PlaybackClock::new(info.duration_ms, looping);
                    }
                    let _ = tx.send(Ok(info));
                    run(reader, link2, start);
                }
            })
            .map_err(|e| format!("Could not start the video thread ({e})."))?;
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(info)) => Ok(Player { link, start, info, path: path.to_path_buf(), handle: Some(handle) }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => Err("The video took too long to open.".to_string()),
        }
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn play(&self) {
        let now = self.now();
        let (m, cv) = &*self.link;
        let mut s = m.lock().unwrap();
        if s.clock.ended(now) {
            s.restart_serial += 1; // pressing play after the end starts from the top
            s.clock.restart(now);
        } else {
            s.clock.play(now);
        }
        cv.notify_all();
    }
    pub fn pause(&self) {
        let now = self.now();
        let (m, cv) = &*self.link;
        m.lock().unwrap().clock.pause(now);
        cv.notify_all();
    }
    pub fn restart(&self) {
        let now = self.now();
        let (m, cv) = &*self.link;
        let mut s = m.lock().unwrap();
        s.restart_serial += 1;
        s.clock.restart(now);
        cv.notify_all();
    }
    /// Jump to `ms`. Playing stays playing, paused stays paused (and shows the picture at that time).
    pub fn seek(&self, ms: u64) {
        let now = self.now();
        let (m, cv) = &*self.link;
        let mut s = m.lock().unwrap();
        s.restart_serial += 1; // the thread rewinds, then decodes quickly forward to the new time
        s.clock.seek(ms, now);
        cv.notify_all();
    }
    /// Move the clock to `ms` without rewinding the decoder (for small corrections: the picture follows the
    /// sound's true position). The decoder thread skips ahead cheaply or waits, whichever is needed.
    pub fn sync_to(&self, ms: u64) {
        let now = self.now();
        let (m, cv) = &*self.link;
        let mut s = m.lock().unwrap();
        s.clock.seek(ms, now);
        cv.notify_all();
    }
    /// Jump forward (positive) or back (negative) by `delta_ms` from where the video is now.
    pub fn skip(&self, delta_ms: i64) {
        let pos = self.state().position_ms as i64;
        self.seek((pos + delta_ms).max(0) as u64);
    }
    pub fn set_looping(&self, on: bool) {
        let now = self.now();
        let (m, cv) = &*self.link;
        m.lock().unwrap().clock.set_looping(on, now);
        cv.notify_all();
    }

    pub fn state(&self) -> PlayerState {
        let now = self.now();
        let s = self.link.0.lock().unwrap();
        PlayerState {
            playing: s.clock.is_playing() && !s.clock.ended(now),
            ended: s.clock.ended(now),
            position_ms: s.clock.position_ms(now),
            duration_ms: s.clock.duration_ms(),
            looping: s.clock.is_looping(),
            error: s.error.clone(),
        }
    }

    /// The newest frame and a counter that goes up whenever it changes (so the screen only
    /// re-uploads when needed).
    pub fn latest(&self) -> Option<(u64, Arc<Frame>)> {
        let s = self.link.0.lock().unwrap();
        s.latest.clone().map(|f| (s.serial, f))
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        {
            let (m, cv) = &*self.link;
            m.lock().unwrap().quit = true;
            cv.notify_all();
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn publish(link: &Link, f: Frame) {
    let mut s = link.0.lock().unwrap();
    s.latest = Some(Arc::new(f));
    s.serial += 1;
}

fn fail(link: &Link, msg: String) {
    link.0.lock().unwrap().error = Some(msg);
}

/// Sleep, but wake at once if a button was pressed or the player is being dropped.
fn nap(link: &Link, ms: u64) {
    let (m, cv) = &**link;
    let g = m.lock().unwrap();
    if g.quit {
        return;
    }
    let _ = cv.wait_timeout(g, Duration::from_millis(ms.max(1))).unwrap();
}

fn run(mut reader: VideoReader, link: Link, start: Instant) {
    let now = || start.elapsed().as_millis() as u64;
    let frame_ms = (1000.0 / reader.info().fps.max(1.0)) as u64;
    let last_pts = reader.last_pts();
    // the first picture, so a paused video already shows something
    match reader.next_frame(true) {
        Ok(Some(f)) => publish(&link, f),
        Ok(None) => {}
        Err(e) => {
            fail(&link, e);
            return;
        }
    }
    let mut seen_restart = 0u64;
    let mut waiting_for_wrap = false;
    loop {
        let (quit, restart_serial, playing, ended, pos, looping) = {
            let s = link.0.lock().unwrap();
            let n = now();
            (s.quit, s.restart_serial, s.clock.is_playing(), s.clock.ended(n), s.clock.position_ms(n), s.clock.is_looping())
        };
        if quit {
            return;
        }
        if restart_serial != seen_restart {
            seen_restart = restart_serial;
            waiting_for_wrap = false;
            // a seek: jump to the keyframe before the wanted time (not back to the start of the file), then
            // decode (without making pictures) up to the wanted time, then show that picture
            let target = link.0.lock().unwrap().clock.position_ms(now());
            if let Err(e) = reader.seek_to(target) {
                fail(&link, e);
                return;
            }
            while let Some(p) = reader.peek_pts() {
                if p >= target {
                    break;
                }
                if let Err(e) = reader.next_frame(false) {
                    fail(&link, e);
                    return;
                }
            }
            loop {
                match reader.next_frame(true) {
                    Ok(Some(f)) => {
                        if !f.image.pixels.is_empty() {
                            publish(&link, f);
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        fail(&link, e);
                        return;
                    }
                }
            }
            continue;
        }
        if !playing || ended {
            nap(&link, 40);
            continue;
        }
        if waiting_for_wrap {
            // we are at the end of the file and rewound; hold until the clock has wrapped round
            if pos < last_pts || !looping {
                waiting_for_wrap = false;
            } else {
                nap(&link, 4);
                continue;
            }
        }
        let Some(next_pts) = reader.peek_pts() else {
            // end of the file
            if looping {
                if let Err(e) = reader.rewind() {
                    fail(&link, e);
                    return;
                }
                waiting_for_wrap = pos >= last_pts && last_pts > 0;
                if last_pts == 0 {
                    nap(&link, 40);
                }
            } else {
                nap(&link, 40);
            }
            continue;
        };
        if next_pts > pos {
            nap(&link, (next_pts - pos).min(15));
            continue;
        }
        // due now. If we are far behind, decode without making pictures (cheap frame skipping).
        let convert = pos - next_pts <= frame_ms * 2;
        match reader.next_frame(convert) {
            Ok(Some(f)) => {
                // frames decoded while we were catching up carry no pixels: they are skipped
                if convert && !f.image.pixels.is_empty() {
                    publish(&link, f);
                }
            }
            Ok(None) => {}
            Err(e) => {
                fail(&link, e);
                // keep the last good picture on screen; stop trying
                let (m, _) = &*link;
                m.lock().unwrap().clock.pause(now());
                return;
            }
        }
    }
}
