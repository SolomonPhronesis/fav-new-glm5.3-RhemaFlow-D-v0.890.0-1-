//! Opening MP4 files whose SOUND track the parser cannot read (v0.822.0).
//!
//! Some files (QuickTime/iTunes-style audio headers, odd encoders) make the MP4 parser stop with
//! "mp4a box contains a box with a larger size than it", even though the PICTURE is perfectly good.
//! RhemaFlow only decodes the picture (video sound plays through the Mac's own player), so when the
//! normal open fails we label every track that is not video as "free" (an ignored box of the same
//! size) and try again. Nothing on disk is changed: the label change is applied while reading.

use std::io::{Read, Seek, SeekFrom};

/// (absolute file offset, 4 new bytes) pairs.
pub type Patches = Vec<(u64, [u8; 4])>;

const MAX_MOOV: u64 = 256 * 1024 * 1024;

fn be32(b: &[u8]) -> u32 { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) }

/// Child boxes of `buf[start..end]` as (type, box_start, content_start, box_end).
fn children(buf: &[u8], start: usize, end: usize) -> Vec<([u8; 4], usize, usize, usize)> {
    let mut out = Vec::new();
    let mut p = start;
    while p + 8 <= end {
        let size = be32(&buf[p..]) as usize;
        let ty = [buf[p + 4], buf[p + 5], buf[p + 6], buf[p + 7]];
        let (head, total) = match size {
            0 => (8, end - p),
            1 if p + 16 <= end => {
                let big = u64::from_be_bytes(buf[p + 8..p + 16].try_into().unwrap());
                (16, big.min(usize::MAX as u64 / 2) as usize)
            }
            1 => break,
            s => (8, s),
        };
        if total < head || p + total > end { break; }
        out.push((ty, p, p + head, p + total));
        p += total;
    }
    out
}

/// Find the tracks that are not video. Returns the patches that turn each `trak` into `free`.
pub fn non_video_track_patches<R: Read + Seek>(r: &mut R) -> std::io::Result<Patches> {
    let len = r.seek(SeekFrom::End(0))?;
    let mut pos = 0u64;
    while pos + 8 <= len {
        r.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 16];
        let n = r.read(&mut h)?;
        if n < 8 { break; }
        let size = be32(&h) as u64;
        let ty = [h[4], h[5], h[6], h[7]];
        let (head, total) = match size {
            0 => (8u64, len - pos),
            1 if n >= 16 => (16, u64::from_be_bytes(h[8..16].try_into().unwrap())),
            1 => break,
            s => (8, s),
        };
        if total < head { break; }
        if &ty == b"moov" && total <= MAX_MOOV {
            let content = pos + head;
            let mut buf = vec![0u8; (total - head).min(len - content) as usize];
            r.seek(SeekFrom::Start(content))?;
            r.read_exact(&mut buf)?;
            let mut patches = Vec::new();
            for (t, bstart, cstart, bend) in children(&buf, 0, buf.len()) {
                if &t != b"trak" { continue; }
                let handler = children(&buf, cstart, bend).into_iter().find(|c| &c.0 == b"mdia").and_then(|mdia| {
                    children(&buf, mdia.2, mdia.3).into_iter().find(|c| &c.0 == b"hdlr").and_then(|hdlr| {
                        // version+flags (4), pre_defined (4), handler_type (4)
                        let o = hdlr.2 + 8;
                        (o + 4 <= hdlr.3).then(|| [buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
                    })
                });
                // no readable handler: judge by what the track holds (sound sample types)
                let holds_sound = || buf[bstart..bend].windows(4).any(|w| w == b"mp4a" || w == b"soun" || w == b"alac" || w == b"sowt" || w == b"twos" || w == b"ac-3");
                let not_video = match handler { Some(h) => &h != b"vide", None => holds_sound() };
                if not_video {
                    patches.push((content + bstart as u64 + 4, *b"free"));
                }
            }
            return Ok(patches);
        }
        pos += total.max(8);
    }
    Ok(Vec::new())
}

/// Apply patches to bytes held in memory.
pub fn apply_to_bytes(bytes: &mut [u8], patches: &Patches) {
    for (off, v) in patches {
        let o = *off as usize;
        if o + 4 <= bytes.len() { bytes[o..o + 4].copy_from_slice(v); }
    }
}

/// A reader over a real file that shows the patched labels instead of the ones on disk.
pub struct PatchedReader<R: Read + Seek> { inner: R, pos: u64, patches: Patches }

impl<R: Read + Seek> PatchedReader<R> {
    pub fn new(inner: R, patches: Patches) -> Self { Self { inner, pos: 0, patches } }
}
impl<R: Read + Seek> Read for PatchedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.seek(SeekFrom::Start(self.pos))?;
        let n = self.inner.read(buf)?;
        for (off, v) in &self.patches {
            for (i, b) in v.iter().enumerate() {
                let at = off + i as u64;
                if at >= self.pos && at < self.pos + n as u64 { buf[(at - self.pos) as usize] = *b; }
            }
        }
        self.pos += n as u64;
        Ok(n)
    }
}
impl<R: Read + Seek> Seek for PatchedReader<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let np = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(d) => self.inner.seek(SeekFrom::End(0))? as i128 + d as i128,
        };
        if np < 0 { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start")); }
        self.pos = np as u64;
        Ok(self.pos)
    }
}


/// A readable outline of the file's boxes (top level, tracks, their handler and sample types), for a
/// bug report when a file will not open. Never fails; says what it could not read.
pub fn describe<R: Read + Seek>(r: &mut R) -> String {
    let mut out = String::new();
    let len = match r.seek(SeekFrom::End(0)) { Ok(l) => l, Err(e) => return format!("cannot measure file: {e}") };
    out.push_str(&format!("file size {len}\n"));
    let mut pos = 0u64;
    let mut guard = 0;
    while pos + 8 <= len && guard < 200 {
        guard += 1;
        if r.seek(SeekFrom::Start(pos)).is_err() { break; }
        let mut h = [0u8; 16];
        let n = r.read(&mut h).unwrap_or(0);
        if n < 8 { break; }
        let size = be32(&h) as u64;
        let ty = String::from_utf8_lossy(&h[4..8]).to_string();
        let (head, total) = match size { 0 => (8u64, len - pos), 1 if n >= 16 => (16, u64::from_be_bytes(h[8..16].try_into().unwrap())), 1 => break, s => (8, s) };
        out.push_str(&format!("top-level '{ty}' size {total}\n"));
        if total < head { break; }
        if ty == "moov" && total <= MAX_MOOV {
            let mut buf = vec![0u8; (total - head).min(len - pos - head) as usize];
            if r.seek(SeekFrom::Start(pos + head)).is_ok() && r.read_exact(&mut buf).is_ok() {
                for (t, _, cs, be) in children(&buf, 0, buf.len()) {
                    if &t != b"trak" { continue; }
                    let mut line = String::from("  trak:");
                    if let Some(mdia) = children(&buf, cs, be).into_iter().find(|c| &c.0 == b"mdia") {
                        for c in children(&buf, mdia.2, mdia.3) {
                            if &c.0 == b"hdlr" && c.2 + 12 <= c.3 { line.push_str(&format!(" handler={}", String::from_utf8_lossy(&buf[c.2 + 8..c.2 + 12]))); }
                            if &c.0 == b"minf" {
                                if let Some(stbl) = children(&buf, c.2, c.3).into_iter().find(|x| &x.0 == b"stbl") {
                                    if let Some(stsd) = children(&buf, stbl.2, stbl.3).into_iter().find(|x| &x.0 == b"stsd") {
                                        // version/flags (4) + entry count (4), then sample entries
                                        for e in children(&buf, stsd.2 + 8, stsd.3) { line.push_str(&format!(" entry={} size={}", String::from_utf8_lossy(&e.0), e.3 - e.1)); }
                                    }
                                }
                            }
                        }
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
            }
        }
        pos += total.max(8);
    }
    out
}
