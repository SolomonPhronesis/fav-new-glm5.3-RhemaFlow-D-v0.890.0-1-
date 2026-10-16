//! Built-in web output (v0.812.0): an OBS browser-source page and a phone remote.
//!
//!   http://<this Mac>:<port>/stage    verse as a web page (OBS "Browser" source, transparent background)
//!   http://<this Mac>:<port>/remote   phone remote: next/previous, approve/dismiss, blackout, clear, read
//!   http://<this Mac>:<port>/state    the same data as JSON
//!
//! Plain `std::net`, no extra dependency. OFF by default. Listens on this Mac only
//! unless "allow phones on the same Wi-Fi" is ticked. Remote commands need the PIN
//! shown in the app; /state (read-only, what the congregation sees anyway) does not.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Everything the pages need, updated by the app, read by the server threads.
#[derive(Default, Clone)]
pub struct Shared {
    pub seq: u64,
    pub kind: String,
    pub label: String,
    pub text: String,
    pub translation: String,
    pub style: String,
    pub bg: String,
    pub fg: String,
    pub blackout: bool,
    pub zoom: f32,
    pub reading: bool,
    /// pending review cards: (id, title, accuracy text)
    pub review: Vec<(u64, String, String)>,
    /// the media shelf for the phone: (id, name, missing)
    pub media: Vec<(u64, String, bool)>,
    pub pin: String,
    /// commands from phones, drained by the app
    pub commands: Vec<String>,
}

pub const COMMANDS: &[&str] = &[
    "next_verse", "prev_verse", "next_chapter", "prev_chapter", "clear", "blackout", "approve", "dismiss", "read",
    "stop_reading", "plan_next", "history_back", "history_forward", "zoom_in", "zoom_out", "pick_1", "pick_2", "pick_3", "pick_4", "pick_5",
];

pub struct WebServer {
    pub shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    pub addr: String,
}

impl WebServer {
    /// `port` 0 = any free port (tests). `lan` = listen on every network interface.
    pub fn start(port: u16, lan: bool, pin: &str) -> std::io::Result<WebServer> {
        let host = if lan { "0.0.0.0" } else { "127.0.0.1" };
        let listener = TcpListener::bind((host, port))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?.to_string();
        let shared = Arc::new(Mutex::new(Shared { pin: pin.to_string(), zoom: 1.0, style: "classic".into(), kind: "blank".into(), ..Default::default() }));
        let stop = Arc::new(AtomicBool::new(false));
        let (sh, st) = (shared.clone(), stop.clone());
        std::thread::spawn(move || {
            while !st.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((conn, _)) => {
                        let sh = sh.clone();
                        std::thread::spawn(move || {
                            let _ = serve(conn, &sh);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(25)),
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        });
        Ok(WebServer { shared, stop, addr })
    }

    pub fn port(&self) -> u16 {
        self.addr.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(0)
    }

    /// Commands phones have sent since the last call.
    pub fn take_commands(&self) -> Vec<String> {
        self.shared.lock().map(|mut s| std::mem::take(&mut s.commands)).unwrap_or_default()
    }

    pub fn update(&self, f: impl FnOnce(&mut Shared)) {
        if let Ok(mut s) = self.shared.lock() {
            f(&mut s);
        }
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(mut conn: TcpStream, shared: &Arc<Mutex<Shared>>) -> std::io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 8192 {
        let n = conn.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let first = head.lines().next().unwrap_or("");
    let (status, ctype, body) = route(first, shared);
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    conn.write_all(resp.as_bytes())?;
    conn.write_all(body.as_bytes())?;
    conn.flush()
}

/// "GET /state?x=1 HTTP/1.1" -> (status, content type, body). Pure, so it is testable.
pub fn route(request_line: &str, shared: &Arc<Mutex<Shared>>) -> (&'static str, &'static str, String) {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let q = |key: &str| -> String {
        query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_string())
            .unwrap_or_default()
    };
    let snapshot = || shared.lock().map(|s| s.clone()).unwrap_or_default();
    let pin_ok = |s: &Shared| !s.pin.is_empty() && q("pin") == s.pin;
    match (method, path) {
        ("GET", "/") => ("200 OK", "text/html; charset=utf-8", INDEX_HTML.to_string()),
        ("GET", "/stage") => ("200 OK", "text/html; charset=utf-8", STAGE_HTML.to_string()),
        ("GET", "/operator") => ("200 OK", "text/html; charset=utf-8", OPERATOR_HTML.to_string()),
        ("GET", "/actions") => {
            let list: Vec<_> = crate::controllers::ACTIONS.iter().map(|(n, l)| serde_json::json!({"name": n, "label": l})).collect();
            ("200 OK", "application/json", serde_json::json!({"actions": list}).to_string())
        }
        ("GET", "/remote") => ("200 OK", "text/html; charset=utf-8", REMOTE_HTML.to_string()),
        ("GET", "/state") => {
            let s = snapshot();
            let j = serde_json::json!({
                "seq": s.seq, "kind": s.kind, "label": s.label, "text": s.text, "translation": s.translation,
                "style": s.style, "bg": s.bg, "fg": s.fg, "blackout": s.blackout, "zoom": s.zoom, "reading": s.reading,
            });
            ("200 OK", "application/json", j.to_string())
        }
        ("GET", "/review") => {
            let s = snapshot();
            if !pin_ok(&s) {
                return ("403 Forbidden", "application/json", "{\"error\":\"pin\"}".into());
            }
            let cards: Vec<_> = s.review.iter().map(|(id, t, a)| serde_json::json!({"id": id, "title": t, "acc": a})).collect();
            ("200 OK", "application/json", serde_json::json!({"cards": cards, "label": s.label, "blackout": s.blackout, "reading": s.reading}).to_string())
        }
        ("GET", "/media") => {
            let s = snapshot();
            if !pin_ok(&s) {
                return ("403 Forbidden", "application/json", "{\"error\":\"pin\"}".into());
            }
            let items: Vec<_> = s.media.iter().take(200).map(|(id, n, miss)| serde_json::json!({"id": id, "name": n, "missing": miss})).collect();
            ("200 OK", "application/json", serde_json::json!({"items": items}).to_string())
        }
        ("POST", "/cmd") | ("GET", "/cmd") => {
            let s = snapshot();
            if !pin_ok(&s) {
                return ("403 Forbidden", "application/json", "{\"error\":\"pin\"}".into());
            }
            let c = q("c");
            let is_media = c.strip_prefix("media_").map_or(false, |n| !n.is_empty() && n.len() < 12 && n.chars().all(|ch| ch.is_ascii_digit()));
            if !COMMANDS.contains(&c.as_str()) && !is_media && !crate::controllers::is_known(&c) {
                return ("400 Bad Request", "application/json", "{\"error\":\"unknown command\"}".into());
            }
            if let Ok(mut g) = shared.lock() {
                if g.commands.len() < 50 {
                    g.commands.push(c);
                }
            }
            ("200 OK", "application/json", "{\"ok\":true}".into())
        }
        _ => ("404 Not Found", "text/plain", "not found".into()),
    }
}

const INDEX_HTML: &str = r#"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>RhemaFlow</title><style>body{font:16px system-ui;background:#10131a;color:#e6e8ee;max-width:560px;margin:40px auto;padding:0 16px}a{color:#e0b25a}</style>
<h1>RhemaFlow-D</h1><p><a href="/stage">/stage</a> &mdash; the verse as a web page (add as an OBS <b>Browser</b> source, width 1920 height 1080)</p>
<p><a href="/remote">/remote</a> &mdash; phone remote (needs the PIN shown in the app)</p><p><a href="/state">/state</a> &mdash; JSON</p>"#;

const STAGE_HTML: &str = r#"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>RhemaFlow stage</title>
<style>
html,body{margin:0;height:100%;overflow:hidden;background:transparent;font-family:Georgia,'Times New Roman',serif}
body.chroma{background:#00b140}
#box{position:absolute;left:8%;right:8%;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;top:0;bottom:0;color:#fff;opacity:0;transition:opacity .35s}
body.lower #box{top:auto;bottom:4%;height:30%;background:rgba(0,0,0,.78);border-radius:12px;left:5%;right:5%}
#txt{line-height:1.3;white-space:pre-line;text-shadow:0 2px 6px rgba(0,0,0,.6)}#ref{margin-top:.6em;color:#e0b25a;font-size:.5em}
body.chroma #ref{color:#fff}body.large #ref{display:none}
</style><body><div id=box><div id=txt></div><div id=ref></div></div>
<script>
let last=-1;
function fit(){const b=document.getElementById('box'),t=document.getElementById('txt');
 const q=new URLSearchParams(location.search);const z=parseFloat(q.get('zoom')||window.__zoom||1);
 const lower=document.body.classList.contains('lower'),large=document.body.classList.contains('large');
 let size=Math.min(innerWidth,innerHeight*1.4)/ (t.textContent.length>180?22:t.textContent.length>90?17:13)*z*(large?1.35:1)*(lower?.55:1);
 b.style.fontSize=size+'px';
 for(let i=0;i<14&&b.scrollHeight>b.clientHeight*1.0&&size>10;i++){size*=.92;b.style.fontSize=size+'px';}}
async function tick(){try{const r=await fetch('/state',{cache:'no-store'});const s=await r.json();
 const q=new URLSearchParams(location.search);const style=q.get('style')||s.style;
 document.body.className=style;window.__zoom=s.zoom;
 if(style!=='chroma'&&style!=='lower'){document.body.style.background=s.bg||'#000';}else if(style==='lower'){document.body.style.background='transparent';}else{document.body.style.background='';}
 document.getElementById('box').style.color=s.fg||'#fff';
 const box=document.getElementById('box');
 const show=!s.blackout&&(s.kind==='scripture'||s.kind==='slide'||s.kind==='countdown');
 if(!show){box.style.opacity=0;}
 else{document.getElementById('txt').textContent=s.text;document.getElementById('ref').textContent=s.kind==='scripture'?s.label+' \u2014 '+s.translation:(s.kind==='countdown'?s.label:'');fit();box.style.opacity=1;}
 last=s.seq;}catch(e){}}
setInterval(tick,400);tick();addEventListener('resize',fit);
</script>"#;

/// Second-campus / helper operator page: what is on the screen, plus a button for every action (needs the PIN).
const OPERATOR_HTML: &str = r#"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1,maximum-scale=1">
<title>RhemaFlow operator</title>
<style>body{font:16px system-ui;background:#10131a;color:#e6e8ee;margin:0;padding:12px}#now{background:#1b2030;border-radius:10px;padding:12px;margin-bottom:12px;min-height:72px}
#now b{color:#e0b25a}#g{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:8px}button{padding:14px 8px;border:0;border-radius:10px;background:#2a3350;color:#fff;font-size:15px}
button:active{background:#e0b25a;color:#000}input{padding:10px;border-radius:8px;border:0;width:120px}#m{opacity:.7;margin:8px 0}</style>
<h2>RhemaFlow operator</h2>
<div>PIN <input id=pin type=password inputmode=numeric> <span id=m></span></div>
<div id=now>...</div><div id=g></div>
<script>
const P=new URLSearchParams(location.search);const pin=document.getElementById('pin');pin.value=P.get('pin')||localStorage.getItem('rfpin')||'';
async function poll(){try{const s=await (await fetch('/state')).json();document.getElementById('now').innerHTML='<b>'+(s.label||'(blank)')+'</b>'+(s.blackout?' &middot; blackout':'')+'<br>'+(s.text||'').replace(/</g,'&lt;').slice(0,300)}catch(e){}}
async function send(c){localStorage.setItem('rfpin',pin.value);const r=await fetch('/cmd?c='+c+'&pin='+encodeURIComponent(pin.value));document.getElementById('m').textContent=r.ok?'sent':'wrong PIN?';setTimeout(poll,300)}
(async()=>{const a=(await (await fetch('/actions')).json()).actions;const g=document.getElementById('g');for(const x of a){const b=document.createElement('button');b.textContent=x.label;b.onclick=()=>send(x.name);g.appendChild(b)}})();
poll();setInterval(poll,1500);
</script>"#;

const REMOTE_HTML: &str = r#"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1,maximum-scale=1">
<title>RhemaFlow remote</title>
<style>
body{font:17px system-ui;background:#10131a;color:#e6e8ee;margin:0;padding:12px;max-width:640px;margin:auto}
h1{font-size:18px;color:#e0b25a;margin:4px 0 10px}#now{background:#000;border-radius:10px;padding:12px;min-height:3.2em;font-family:Georgia,serif}
#lab{color:#e0b25a;font-size:.8em;margin-top:6px}
.grid{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:10px 0}
button{font:inherit;padding:16px 8px;border:0;border-radius:10px;background:#252b38;color:#e6e8ee}
button:active{background:#3a4358}.go{background:#1f6f4a}.no{background:#7a2b2b}.wide{grid-column:1/3}
.card{background:#1a1f2b;border-radius:10px;padding:10px;margin:8px 0}.card b{font-size:1.1em}
input{font:inherit;padding:10px;border-radius:8px;border:1px solid #444;background:#0c0e13;color:#fff;width:6em}
</style>
<h1>RhemaFlow remote</h1>
<div id=pin><label>PIN <input id=p inputmode=numeric></label> <button onclick="setpin()">OK</button></div>
<div id=now>&nbsp;</div><div id=lab></div>
<div id=cards></div>
<h1>Media shelf</h1><div id=media class=grid></div>
<div class=grid>
<button onclick="c('prev_verse')">&#9664; verse</button><button onclick="c('next_verse')">verse &#9654;</button>
<button onclick="c('prev_chapter')">&#9664; chapter</button><button onclick="c('next_chapter')">chapter &#9654;</button>
<button class=wide onclick="c('plan_next')">Next in sermon plan</button>
<button onclick="c('blackout')">Blackout</button><button onclick="c('clear')">Clear</button>
<button onclick="c('read')">Read aloud</button><button onclick="c('stop_reading')">Stop reading</button>
<button onclick="c('zoom_out')">Text &minus;</button><button onclick="c('zoom_in')">Text +</button>
</div>
<script>
let pin=localStorage.getItem('rfpin')||'';document.getElementById('p').value=pin;
function setpin(){pin=document.getElementById('p').value.trim();localStorage.setItem('rfpin',pin);poll();}
async function c(n){await fetch('/cmd?c='+n+'&pin='+encodeURIComponent(pin),{method:'POST'});setTimeout(poll,150);}
async function poll(){try{
 const s=await (await fetch('/state',{cache:'no-store'})).json();
 document.getElementById('now').textContent=s.kind==='scripture'?s.text:'(nothing on screen)';
 document.getElementById('lab').textContent=s.kind==='scripture'?s.label+' \u2014 '+s.translation+(s.blackout?'  [BLACKOUT]':''):'';
 const r=await fetch('/review?pin='+encodeURIComponent(pin),{cache:'no-store'});
 if(r.status===403){document.getElementById('pin').style.display='block';document.getElementById('cards').innerHTML='<div class=card>Enter the PIN shown in RhemaFlow.</div>';return;}
 document.getElementById('pin').style.display='none';
 const j=await r.json();let h='';
 j.cards.forEach((k,i)=>{h+='<div class=card><b>'+k.title+'</b> <small>'+k.acc+'</small><div class=grid><button class=go onclick="c(\'approve\')">Approve</button><button class=no onclick="c(\'dismiss\')">Dismiss</button></div></div>';});
 document.getElementById('cards').innerHTML=h;
 const m=await (await fetch('/media?pin='+encodeURIComponent(pin),{cache:'no-store'})).json();
 const box=document.getElementById('media');box.textContent='';
 (m.items||[]).forEach(it=>{const b=document.createElement('button');b.textContent=it.missing?(it.name+' (missing)'):it.name;b.disabled=!!it.missing;b.onclick=()=>c('media_'+it.id);box.appendChild(b);});
}catch(e){}}
setInterval(poll,1200);poll();
</script>"#;

/// A 4-digit PIN that is not easy to guess from the clock (mixes time and pid).
pub fn new_pin() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64 ^ d.as_secs().rotate_left(13)).unwrap_or(1234);
    format!("{:04}", (t ^ (std::process::id() as u64).rotate_left(7)) % 10_000)
}

/// This Mac's address on the local network, for the phone URL (best effort).
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?; // no packet is sent; this only picks the outgoing interface
    Some(s.local_addr().ok()?.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(pin: &str) -> Arc<Mutex<Shared>> {
        Arc::new(Mutex::new(Shared { pin: pin.into(), kind: "scripture".into(), label: "Revelation 1:6".into(), text: "And hath made us kings".into(), translation: "KJV".into(), zoom: 1.0, style: "classic".into(), ..Default::default() }))
    }

    #[test]
    fn state_is_public_json() {
        let sh = shared("1234");
        let (st, ct, body) = route("GET /state HTTP/1.1", &sh);
        assert_eq!((st, ct), ("200 OK", "application/json"));
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["label"], "Revelation 1:6");
        assert_eq!(v["style"], "classic");
    }

    #[test]
    fn commands_need_the_pin_and_a_known_name() {
        let sh = shared("1234");
        assert_eq!(route("POST /cmd?c=clear HTTP/1.1", &sh).0, "403 Forbidden");
        assert_eq!(route("POST /cmd?c=clear&pin=9999 HTTP/1.1", &sh).0, "403 Forbidden");
        assert_eq!(route("POST /cmd?c=rm-rf&pin=1234 HTTP/1.1", &sh).0, "400 Bad Request");
        assert_eq!(route("POST /cmd?c=next_verse&pin=1234 HTTP/1.1", &sh).0, "200 OK");
        assert_eq!(sh.lock().unwrap().commands, vec!["next_verse"]);
        // an empty PIN never matches (a server started without one is not open)
        let open = shared("");
        assert_eq!(route("POST /cmd?c=clear&pin= HTTP/1.1", &open).0, "403 Forbidden");
        assert_eq!(route("GET /review HTTP/1.1", &sh).0, "403 Forbidden");
        assert_eq!(route("GET /review?pin=1234 HTTP/1.1", &sh).0, "200 OK");
    }

    #[test]
    fn pages_exist_and_unknown_paths_404() {
        let sh = shared("1");
        for p in ["/", "/stage", "/remote"] {
            let (st, ct, body) = route(&format!("GET {p} HTTP/1.1"), &sh);
            assert_eq!((st, ct), ("200 OK", "text/html; charset=utf-8"));
            assert!(body.contains("<"));
        }
        assert_eq!(route("GET /etc/passwd HTTP/1.1", &sh).0, "404 Not Found");
    }

    #[test]
    fn real_socket_round_trip_on_localhost() {
        let srv = WebServer::start(0, false, "4321").unwrap();
        srv.update(|s| {
            s.kind = "scripture".into();
            s.label = "John 3:16".into();
        });
        let mut c = TcpStream::connect(&srv.addr).unwrap();
        c.write_all(b"GET /state HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        c.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK"), "{out}");
        assert!(out.contains("John 3:16"));
        let mut c = TcpStream::connect(&srv.addr).unwrap();
        c.write_all(b"POST /cmd?c=blackout&pin=4321 HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        c.read_to_string(&mut out).unwrap();
        assert!(out.contains("\"ok\":true"), "{out}");
        assert_eq!(srv.take_commands(), vec!["blackout"]);
        assert!(srv.take_commands().is_empty());
        assert!(srv.addr.starts_with("127.0.0.1:"), "default is this Mac only");
    }

    #[test]
    fn stage_page_shows_slides_and_countdowns_but_not_pictures() {
        // (spec S7.1) the page script decides by kind: slide and countdown are drawn as text, media is not
        assert!(STAGE_HTML.contains("s.kind==='slide'") && STAGE_HTML.contains("s.kind==='countdown'"));
        assert!(!STAGE_HTML.contains("s.kind==='media'"));
        assert!(STAGE_HTML.contains("white-space:pre-line"), "song lines must stay on their own lines");
    }

    #[test]
    fn pins_are_four_digits() {
        let p = new_pin();
        assert_eq!(p.len(), 4);
        assert!(p.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn the_phone_sees_the_media_shelf_only_with_the_pin_and_may_only_ask_for_numbered_items() {
        let sh = shared("1234");
        sh.lock().unwrap().media = vec![(7, "Holy <Lamb>".into(), false), (9, "Gone".into(), true)];
        assert_eq!(route("GET /media?pin=0000 HTTP/1.1", &sh).0, "403 Forbidden");
        let (code, _, body) = route("GET /media?pin=1234 HTTP/1.1", &sh);
        assert_eq!(code, "200 OK");
        assert!(body.contains("\"id\":7") && body.contains("\"missing\":true"));
        assert_eq!(route("POST /cmd?c=media_7&pin=1234 HTTP/1.1", &sh).0, "200 OK");
        assert_eq!(route("POST /cmd?c=media_&pin=1234 HTTP/1.1", &sh).0, "400 Bad Request");
        assert_eq!(route("POST /cmd?c=media_7;rm&pin=1234 HTTP/1.1", &sh).0, "400 Bad Request");
        assert_eq!(sh.lock().unwrap().commands, vec!["media_7".to_string()]);
    }

    #[test]
    fn operator_page_and_action_list() {
        let sh = Arc::new(Mutex::new(Shared { pin: "1234".into(), ..Default::default() }));
        let (st, ct, body) = route("GET /operator HTTP/1.1", &sh);
        assert_eq!(st, "200 OK");
        assert!(ct.starts_with("text/html") && body.contains("/actions"));
        let (st, _, body) = route("GET /actions HTTP/1.1", &sh);
        assert_eq!(st, "200 OK");
        assert!(body.contains("runsheet_next") && body.contains("giving_toggle"));
    }

    #[test]
    fn new_actions_need_the_pin_and_are_queued() {
        let sh = Arc::new(Mutex::new(Shared { pin: "1234".into(), ..Default::default() }));
        let (st, _, _) = route("GET /cmd?c=runsheet_next&pin=bad HTTP/1.1", &sh);
        assert_eq!(st, "403 Forbidden");
        let (st, _, _) = route("GET /cmd?c=runsheet_next&pin=1234 HTTP/1.1", &sh);
        assert_eq!(st, "200 OK");
        assert_eq!(sh.lock().unwrap().commands, vec!["runsheet_next".to_string()]);
        let (st, _, _) = route("GET /cmd?c=format_disk&pin=1234 HTTP/1.1", &sh);
        assert_eq!(st, "400 Bad Request");
    }
}
