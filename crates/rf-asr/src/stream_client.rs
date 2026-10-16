//! Client for a local streaming-ASR service that speaks the Confucius4-R2T2
//! WebSocket protocol (`ws_server.py`, endpoint `/asr_stream_api_v1`):
//!
//!   client → server : binary frames of raw 16 kHz mono **int16 LE** PCM
//!                     (reference client: 160 ms = 2560 samples per frame)
//!                     text frame `YOUDAO_ONETIME_ASR_STREAM_EOS` = end of audio
//!   server → client : JSON `{"status":"success","msg":{"text":"…","reset":bool,…}}`
//!                     `text` is the NEW (incremental, append-only) words;
//!                     `reset:true` = utterance finished, decoder state restarted.
//!
//! Extension used by the Mac sidecar (ignored by the stock server): a text
//! frame `RF_FLUSH` asks the server to finish the current utterance now
//! (sent when our own VAD hears the speaker stop).
//!
//! Extension (v0.854): a text frame `RF_CONTEXT <words>` updates the engine's
//! hotwords — the R2T2 `context` slot (book names + words learned from the
//! operator's corrections). Sent once when a session opens and again whenever
//! the app has new learned words; the sidecar applies it from the next
//! utterance on. Ignored by the stock R2T2 server.
//!
//! Everything runs on ONE background thread so there is no shared socket
//! state to get wrong: drain audio → write frames → read with a short timeout.

use crossbeam_channel::{unbounded, Receiver, Sender};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tungstenite::{client::IntoClientRequest, Message, WebSocket};

pub const EOS: &str = "YOUDAO_ONETIME_ASR_STREAM_EOS";
pub const FLUSH: &str = "RF_FLUSH";
pub const CONTEXT: &str = "RF_CONTEXT";
/// 160 ms @ 16 kHz — the reference client's frame size
pub const FRAME_SAMPLES: usize = 2560;

#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    Connected,
    /// new words (append-only)
    Text(String),
    /// the server finished an utterance (VAD end, EOS, hard reset)
    Reset,
    /// round-trip cost the server reported for the latest chunk, ms
    Cost(f32),
    Disconnected(String),
}

pub enum StreamCmd {
    Audio(Vec<f32>),
    Flush,
    Eos,
    /// push new hotwords to the engine (R2T2 `context` slot)
    Context(String),
}

pub struct StreamClient {
    pub cmd_tx: Sender<StreamCmd>,
    pub events: Receiver<StreamEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

pub fn f32_to_i16_le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Parse one server JSON message into events. Unknown shapes are ignored.
pub fn parse_server_message(raw: &str) -> Vec<StreamEvent> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else { return vec![] };
    if v.get("status").and_then(|s| s.as_str()).map_or(false, |s| s != "success") {
        return vec![StreamEvent::Disconnected(format!("server error: {raw}"))];
    }
    let Some(msg) = v.get("msg") else { return vec![] };
    let mut out = Vec::new();
    if let Some(t) = msg.get("text").and_then(|t| t.as_str()) {
        if !t.is_empty() {
            out.push(StreamEvent::Text(t.to_string()));
        }
    }
    if let Some(c) = msg.get("asr_cost_ms").and_then(|c| c.as_f64()) {
        out.push(StreamEvent::Cost(c as f32));
    }
    if msg.get("reset").and_then(|r| r.as_bool()) == Some(true) {
        out.push(StreamEvent::Reset);
    }
    out
}

impl StreamClient {
    /// `url` like `ws://127.0.0.1:8272/asr_stream_api_v1`. Reconnects forever
    /// (1 s → 5 s backoff) until dropped.
    pub fn start(url: &str) -> Self {
        let (cmd_tx, cmd_rx) = unbounded::<StreamCmd>();
        let (ev_tx, events) = unbounded::<StreamEvent>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_w = stop.clone();
        let url = url.to_string();
        let worker = std::thread::Builder::new()
            .name("rf-stream-client".into())
            .spawn(move || run(url, cmd_rx, ev_tx, stop_w))
            .ok();
        StreamClient { cmd_tx, events, stop, worker }
    }

    pub fn send_audio(&self, samples: Vec<f32>) {
        let _ = self.cmd_tx.send(StreamCmd::Audio(samples));
    }
    pub fn flush(&self) {
        let _ = self.cmd_tx.send(StreamCmd::Flush);
    }
    /// Update the streaming engine's hotwords (book names / learned words).
    pub fn set_context(&self, words: &str) {
        let _ = self.cmd_tx.send(StreamCmd::Context(words.to_string()));
    }
}

impl Drop for StreamClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

fn connect(url: &str) -> Result<WebSocket<TcpStream>, String> {
    let req = url.into_client_request().map_err(|e| e.to_string())?;
    let host = req.uri().host().ok_or("url has no host")?.to_string();
    let port = req.uri().port_u16().unwrap_or(80);
    let tcp = TcpStream::connect_timeout(
        &std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port)).map_err(|e| e.to_string())?.next().ok_or("cannot resolve host")?,
        Duration::from_secs(3),
    )
    .map_err(|e| e.to_string())?;
    tcp.set_nodelay(true).ok();
    let (ws, _resp) = tungstenite::client(req, tcp).map_err(|e| e.to_string())?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(15))).ok();
    Ok(ws)
}

fn run(url: String, cmd_rx: Receiver<StreamCmd>, ev_tx: Sender<StreamEvent>, stop: Arc<AtomicBool>) {
    let mut backoff = Duration::from_secs(1);
    while !stop.load(Ordering::Relaxed) {
        match connect(&url) {
            Ok(mut ws) => {
                backoff = Duration::from_secs(1);
                let _ = ev_tx.send(StreamEvent::Connected);
                let why = session(&mut ws, &cmd_rx, &ev_tx, &stop);
                let _ = ws.close(None);
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let _ = ev_tx.send(StreamEvent::Disconnected(why));
            }
            Err(e) => {
                let _ = ev_tx.send(StreamEvent::Disconnected(format!("cannot connect to {url}: {e}")));
            }
        }
        // discard audio captured while down: stale speech must not arrive late
        while cmd_rx.try_recv().is_ok() {}
        let until = Instant::now() + backoff;
        while Instant::now() < until && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
        }
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

/// Returns the reason the session ended.
fn session(ws: &mut WebSocket<TcpStream>, cmd_rx: &Receiver<StreamCmd>, ev_tx: &Sender<StreamEvent>, stop: &AtomicBool) -> String {
    // v0.854: open every session with the current hotwords (66 book names +
    // learned words) so the engine's decoder is biased from the first word.
    let ctx = crate::engines::stream_context();
    if !ctx.is_empty() {
        if let Err(e) = ws.send(Message::text(format!("{CONTEXT} {ctx}"))).map_err(|e| e.to_string()) {
            drain_incoming(ws, ev_tx);
            return e;
        }
    }
    let mut pcm: Vec<u8> = Vec::new();
    loop {
        if stop.load(Ordering::Relaxed) {
            return "stopped".into();
        }
        // 1. outgoing: audio → whole frames; control messages
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                StreamCmd::Audio(a) => pcm.extend_from_slice(&f32_to_i16_le(&a)),
                StreamCmd::Flush => {
                    if !pcm.is_empty() {
                        // pad the partial frame with silence so no audio is held back
                        pcm.resize(FRAME_SAMPLES * 2 * ((pcm.len() + FRAME_SAMPLES * 2 - 1) / (FRAME_SAMPLES * 2)), 0);
                    }
                    if let Err(e) = send_frames(ws, &mut pcm) {
                        drain_incoming(ws, ev_tx);
                        return e;
                    }
                    if let Err(e) = ws.send(Message::text(FLUSH)) {
                        drain_incoming(ws, ev_tx);
                        return e.to_string();
                    }
                }
                StreamCmd::Eos => {
                    if let Err(e) = send_frames(ws, &mut pcm).and_then(|_| ws.send(Message::text(EOS)).map_err(|e| e.to_string())) {
                        drain_incoming(ws, ev_tx);
                        return e;
                    }
                }
                StreamCmd::Context(s) => {
                    if let Err(e) = ws.send(Message::text(format!("{CONTEXT} {s}"))).map_err(|e| e.to_string()) {
                        drain_incoming(ws, ev_tx);
                        return e;
                    }
                }
            }
        }
        if let Err(e) = send_frames(ws, &mut pcm) {
            drain_incoming(ws, ev_tx);
            return e;
        }
        // 2. incoming
        match ws.read() {
            Ok(Message::Text(t)) => {
                for ev in parse_server_message(t.as_str()) {
                    let _ = ev_tx.send(ev);
                }
            }
            Ok(Message::Close(_)) => {
                drain_incoming(ws, ev_tx);
                return "server closed the connection".into();
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return e.to_string(),
        }
    }
}

/// After a write failure the server may already have sent its last words
/// (EOS reply, final text before closing). Read whatever is still buffered
/// so nothing said is lost, then give up.
fn drain_incoming(ws: &mut WebSocket<TcpStream>, ev_tx: &Sender<StreamEvent>) {
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(200))).ok();
    for _ in 0..64 {
        match ws.read() {
            Ok(Message::Text(t)) => {
                for ev in parse_server_message(t.as_str()) {
                    let _ = ev_tx.send(ev);
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

fn send_frames(ws: &mut WebSocket<TcpStream>, pcm: &mut Vec<u8>) -> Result<(), String> {
    let frame_bytes = FRAME_SAMPLES * 2;
    while pcm.len() >= frame_bytes {
        let frame: Vec<u8> = pcm.drain(..frame_bytes).collect();
        match ws.send(Message::binary(frame)) {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn pcm_conversion_is_little_endian_i16_and_clamps() {
        let b = f32_to_i16_le(&[0.0, 1.0, -1.0, 2.0, -2.0, 0.5]);
        let v: Vec<i16> = b.chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(v, vec![0, 32767, -32767, 32767, -32767, 16384]);
    }

    #[test]
    fn server_json_becomes_events_and_bad_json_is_ignored() {
        let ev = parse_server_message(r#"{"status":"success","requestId":"x","msg":{"text":"hello","reset":false,"asr_cost_ms":35.4,"total_cost_ms":42.0}}"#);
        assert_eq!(ev, vec![StreamEvent::Text("hello".into()), StreamEvent::Cost(35.4)]);
        let ev = parse_server_message(r#"{"status":"success","msg":{"text":"","reset":true}}"#);
        assert_eq!(ev, vec![StreamEvent::Reset]);
        assert!(parse_server_message("not json").is_empty());
        assert!(matches!(parse_server_message(r#"{"status":"error"}"#)[..], [StreamEvent::Disconnected(_)]));
    }

    /// A tiny in-process server: counts audio bytes, answers every 2 frames
    /// with a word, honours FLUSH and EOS.
    fn spawn_server() -> (String, std::thread::JoinHandle<(usize, Vec<String>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/asr_stream_api_v1", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(tcp).unwrap();
            let (mut bytes, mut frames, mut controls) = (0usize, 0usize, Vec::new());
            let words = ["turn ", "to ", "john "];
            let mut w = 0;
            loop {
                match ws.read() {
                    Ok(Message::Binary(b)) => {
                        bytes += b.len();
                        frames += 1;
                        if frames % 2 == 0 {
                            let t = words[w % words.len()];
                            w += 1;
                            ws.send(Message::text(format!(r#"{{"status":"success","msg":{{"text":"{t}","reset":false,"asr_cost_ms":12.0}}}}"#))).unwrap();
                        }
                    }
                    Ok(Message::Text(t)) => {
                        controls.push(t.to_string());
                        if t.as_str() == FLUSH {
                            ws.send(Message::text(r#"{"status":"success","msg":{"text":"","reset":true}}"#)).unwrap();
                        }
                        if t.as_str() == EOS {
                            ws.send(Message::text(r#"{"status":"success","msg":{"text":"","reset":true}}"#)).unwrap();
                            break;
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
            (bytes, controls)
        });
        (url, h)
    }

    fn collect(c: &StreamClient, want_reset: usize, secs: u64) -> Vec<StreamEvent> {
        let mut got = Vec::new();
        let mut resets = 0;
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end && resets < want_reset {
            if let Ok(e) = c.events.recv_timeout(Duration::from_millis(50)) {
                if e == StreamEvent::Reset {
                    resets += 1;
                }
                got.push(e);
            }
        }
        got
    }

    #[test]
    fn audio_goes_out_in_160ms_frames_text_comes_back_flush_and_eos_work() {
        let (url, server) = spawn_server();
        let c = StreamClient::start(&url);
        // 1 s of audio in uneven pieces → 6 whole frames (+ remainder held back)
        for n in [1000usize, 3300, 5000, 600, 6100] {
            c.send_audio(vec![0.1; n]);
        }
        c.flush(); // pads the remainder to a whole frame, then RF_FLUSH
        let got = collect(&c, 1, 5);
        assert!(got.contains(&StreamEvent::Connected));
        let text: String = got.iter().filter_map(|e| if let StreamEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
        assert_eq!(text, "turn to john ", "{got:?}");
        assert!(got.iter().any(|e| matches!(e, StreamEvent::Cost(c) if *c == 12.0)));
        assert!(got.contains(&StreamEvent::Reset), "FLUSH → reset");
        let _ = c.cmd_tx.send(StreamCmd::Eos);
        let got2 = collect(&c, 1, 5);
        assert!(got2.contains(&StreamEvent::Reset), "EOS → final reset");
        drop(c);
        let (bytes, controls) = server.join().unwrap();
        // 16000 samples sent, remainder padded up to whole frames: 7 frames
        assert_eq!(bytes, 7 * FRAME_SAMPLES * 2);
        // v0.854: the session now opens with an RF_CONTEXT hotword frame
        assert_eq!(controls.len(), 3, "{controls:?}");
        assert!(controls[0].starts_with(CONTEXT), "session opens with hotwords, got {:?}", controls[0].get(0..24));
        assert!(controls[0].contains("Habakkuk"), "book names are in the context");
        assert_eq!(controls[1], FLUSH.to_string());
        assert_eq!(controls[2], EOS.to_string());
    }

    #[test]
    fn context_updates_reach_the_server_as_rf_context_frames() {
        let (url, server) = spawn_server();
        let c = StreamClient::start(&url);
        c.set_context("Habakkuk Malachi Second Chronicles Grace Chapel");
        let _ = c.cmd_tx.send(StreamCmd::Eos);
        collect(&c, 1, 5);
        drop(c);
        let (_, controls) = server.join().unwrap();
        assert!(controls.iter().any(|t| t == "RF_CONTEXT Habakkuk Malachi Second Chronicles Grace Chapel"), "{controls:?}");
    }

    #[test]
    fn an_unreachable_server_reports_and_keeps_retrying() {
        let c = StreamClient::start("ws://127.0.0.1:9/asr_stream_api_v1");
        let first = c.events.recv_timeout(Duration::from_secs(4)).expect("event");
        assert!(matches!(first, StreamEvent::Disconnected(_)), "{first:?}");
        let second = c.events.recv_timeout(Duration::from_secs(4)).expect("retry event");
        assert!(matches!(second, StreamEvent::Disconnected(_)));
    }

    #[test]
    fn a_server_that_goes_away_mid_stream_is_noticed_and_reconnected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/asr_stream_api_v1", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            // first connection: accept then drop immediately
            let (tcp, _) = listener.accept().unwrap();
            let ws = tungstenite::accept(tcp).unwrap();
            drop(ws);
            // second connection: stay up and answer a frame
            let (tcp, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(tcp).unwrap();
            while let Ok(m) = ws.read() {
                if let Message::Binary(_) = m {
                    ws.send(Message::text(r#"{"status":"success","msg":{"text":"back ","reset":false}}"#)).unwrap();
                    break;
                }
            }
        });
        let c = StreamClient::start(&url);
        let mut connects = 0;
        let mut text = String::new();
        let end = Instant::now() + Duration::from_secs(8);
        while Instant::now() < end && text.is_empty() {
            c.send_audio(vec![0.1; FRAME_SAMPLES]);
            if let Ok(e) = c.events.recv_timeout(Duration::from_millis(100)) {
                match e {
                    StreamEvent::Connected => connects += 1,
                    StreamEvent::Text(t) => text = t,
                    _ => {}
                }
            }
        }
        assert_eq!(text, "back ");
        assert!(connects >= 2, "reconnected (connects={connects})");
        drop(c);
        let _ = h.join();
    }

    /// Cross-language check against the real Python sidecar (mock backend):
    ///   python3 sidecar/r2t2_server.py --backend mock --port 8272 &
    ///   RF_TEST_WS_URL=ws://127.0.0.1:8272/asr_stream_api_v1 cargo test -p rf-asr -- --ignored sidecar
    #[test]
    #[ignore]
    fn talks_to_the_python_sidecar() {
        let url = std::env::var("RF_TEST_WS_URL").expect("set RF_TEST_WS_URL");
        let c = StreamClient::start(&url);
        for _ in 0..13 {
            c.send_audio(vec![0.0; FRAME_SAMPLES]); // ~2 s of silence; the mock ignores content
        }
        c.flush();
        let got = collect(&c, 1, 6);
        let text: String = got.iter().filter_map(|e| if let StreamEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
        assert_eq!(text.split_whitespace().collect::<Vec<_>>().join(" "), "turn with me to john chapter three verse sixteen", "{got:?}");
        assert!(got.contains(&StreamEvent::Reset));
    }
}
