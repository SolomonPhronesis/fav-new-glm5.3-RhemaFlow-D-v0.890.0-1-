//! rf-media — opens pictures and H.264 MP4 video for the stage (Stage 2, spec section 3).
//!
//! * `decode_image` / `load_image`: PNG, JPEG, GIF (first frame), BMP -> RGBA pixels.
//! * `VideoReader`: an MP4 with H.264 video -> RGBA frames in display order.
//! * `Player`: a background thread that plays a `VideoReader` on its own clock (play, pause,
//!   restart, loop) and always holds the newest frame for the screen to pick up.
//!
//! No sound (the H.264 decoder only makes pictures; sound is the app's job). No UI.

pub mod audiocut;
pub mod convert;
pub mod docsearch;
pub mod document;
pub mod thumb;
mod image_load;
mod player;
pub mod mp4fix;
mod video;

pub use image_load::{decode_image, load_image, MAX_IMAGE_SIDE};
pub use player::{Player, PlayerState};
pub use video::{Frame, VideoInfo, VideoReader};

/// Plain RGBA8 pixels, row by row, 4 bytes per pixel.
#[derive(Clone, Debug)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
