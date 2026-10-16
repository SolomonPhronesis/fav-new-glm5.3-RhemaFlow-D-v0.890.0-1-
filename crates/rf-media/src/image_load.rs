//! Pictures (spec S3.1, S3.2, S3.10).

use crate::Rgba;
use image::imageops::FilterType;
use image::io::Limits;
use std::io::Cursor;
use std::path::Path;

/// S3.2: nothing bigger than this is kept in memory.
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// A picture file bigger than this is refused before we even read it.
const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;

pub fn decode_image(bytes: &[u8], max_side: u32) -> Result<Rgba, String> {
    let bad = |why: String| format!("This picture could not be opened ({why}). It may be damaged, or not really a PNG, JPEG, GIF or BMP.");
    let mut reader = image::io::Reader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| bad(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(768 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| bad(e.to_string()))?;
    let max_side = max_side.max(1);
    let img = if img.width().max(img.height()) > max_side { img.resize(max_side, max_side, FilterType::Triangle) } else { img };
    let rgba = img.to_rgba8();
    Ok(Rgba { width: rgba.width(), height: rgba.height(), pixels: rgba.into_raw() })
}

pub fn load_image(path: &Path, max_side: u32) -> Result<Rgba, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot find or open this file ({e})."))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("This picture file is over 200 MB. Please make it smaller first.".to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read this file ({e})."))?;
    decode_image(&bytes, max_side)
}
