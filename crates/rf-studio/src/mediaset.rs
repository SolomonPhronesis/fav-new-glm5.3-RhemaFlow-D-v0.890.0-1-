//! Feature 11: per-file media settings. Trim in/out, loop, speed, volume with fades, how it fits the screen,
//! and colour filters. Pure maths: where in the file are we after this much wall-clock time, and what colour is this pixel now.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fit {
    /// Cover the whole screen, cropping the edges.
    Fill,
    /// Show everything, bars if the shapes differ.
    Contain,
    /// Stretch to the screen shape (distorts).
    Stretch,
    /// Actual size in the middle.
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Curve {
    Linear,
    /// Sounds even to the ear when two songs cross.
    EqualPower,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    /// -1 (black) .. 1 (white)
    pub brightness: f32,
    /// 0 (grey) .. 2
    pub contrast: f32,
    /// 0 (black and white) .. 2
    pub saturation: f32,
    /// degrees
    pub hue: f32,
    pub invert: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Filters { brightness: 0.0, contrast: 1.0, saturation: 1.0, hue: 0.0, invert: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaSettings {
    pub trim_in_ms: u64,
    /// `None` = to the end of the file.
    pub trim_out_ms: Option<u64>,
    pub looping: bool,
    /// 0.25 .. 4
    pub speed: f32,
    /// 0 .. 1
    pub volume: f32,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub curve: Curve,
    pub fit: Fit,
    pub filters: Filters,
}

impl Default for MediaSettings {
    fn default() -> Self {
        MediaSettings { trim_in_ms: 0, trim_out_ms: None, looping: false, speed: 1.0, volume: 1.0, fade_in_ms: 0, fade_out_ms: 0, curve: Curve::EqualPower, fit: Fit::Contain, filters: Filters::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    /// Position inside the file.
    pub media_ms: u64,
    pub finished: bool,
    /// How many times it has wrapped round.
    pub loops: u64,
}

impl MediaSettings {
    /// Make every value safe: speed, volume and filters in range, trim points inside the file and in order.
    pub fn sanitized(mut self, file_ms: u64) -> MediaSettings {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        self.speed = fin(self.speed, 1.0).clamp(0.25, 4.0);
        self.volume = fin(self.volume, 1.0).clamp(0.0, 1.0);
        self.filters.brightness = fin(self.filters.brightness, 0.0).clamp(-1.0, 1.0);
        self.filters.contrast = fin(self.filters.contrast, 1.0).clamp(0.0, 2.0);
        self.filters.saturation = fin(self.filters.saturation, 1.0).clamp(0.0, 2.0);
        self.filters.hue = fin(self.filters.hue, 0.0).rem_euclid(360.0);
        self.trim_in_ms = self.trim_in_ms.min(file_ms.saturating_sub(1));
        if let Some(o) = self.trim_out_ms {
            let o = o.min(file_ms);
            self.trim_out_ms = if o <= self.trim_in_ms { None } else { Some(o) };
        }
        self
    }

    /// The part of the file that plays.
    pub fn window(&self, file_ms: u64) -> (u64, u64) {
        let s = self.sanitized(file_ms);
        (s.trim_in_ms, s.trim_out_ms.unwrap_or(file_ms))
    }

    /// How long it takes to play once on the wall clock (faster speed = shorter).
    pub fn play_length_ms(&self, file_ms: u64) -> u64 {
        let s = self.sanitized(file_ms);
        let (a, b) = s.window(file_ms);
        ((b - a) as f64 / s.speed as f64).round() as u64
    }

    /// After `wall_ms` of playing, where are we in the file?
    pub fn position(&self, wall_ms: u64, file_ms: u64) -> Pos {
        let s = self.sanitized(file_ms);
        let (a, b) = s.window(file_ms);
        let len = (b - a).max(1);
        let moved = (wall_ms as f64 * s.speed as f64) as u64;
        if s.looping {
            Pos { media_ms: a + moved % len, finished: false, loops: moved / len }
        } else if moved >= len {
            Pos { media_ms: b, finished: true, loops: 0 }
        } else {
            Pos { media_ms: a + moved, finished: false, loops: 0 }
        }
    }

    /// Volume at a moment of the play (wall clock, counted from the start of this play), with the fade in and out.
    pub fn gain_at(&self, wall_ms: u64, file_ms: u64) -> f32 {
        let s = self.sanitized(file_ms);
        let total = s.play_length_ms(file_ms);
        let mut g = 1.0f32;
        if s.fade_in_ms > 0 && wall_ms < s.fade_in_ms as u64 {
            g = g.min(fade_gain(s.curve, wall_ms as f32 / s.fade_in_ms as f32));
        }
        if !s.looping && s.fade_out_ms > 0 {
            let left = total.saturating_sub(wall_ms);
            if left < s.fade_out_ms as u64 {
                g = g.min(fade_gain(s.curve, left as f32 / s.fade_out_ms as f32));
            }
        }
        g * s.volume
    }
}

/// 0 (silent) to 1 (full) as `p` goes 0 to 1.
pub fn fade_gain(c: Curve, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    match c {
        Curve::Linear => p,
        Curve::EqualPower => (p * std::f32::consts::FRAC_PI_2).sin(),
        Curve::Smooth => p * p * (3.0 - 2.0 * p),
    }
}

/// Where a picture of `src` size goes on a screen of `dst` size: (x, y, width, height) in screen pixels.
pub fn fit_rect(fit: Fit, src: (f32, f32), dst: (f32, f32)) -> (f32, f32, f32, f32) {
    let (sw, sh) = (src.0.max(1.0), src.1.max(1.0));
    let (dw, dh) = dst;
    match fit {
        Fit::Stretch => (0.0, 0.0, dw, dh),
        Fit::Center => ((dw - sw) / 2.0, (dh - sh) / 2.0, sw, sh),
        Fit::Contain | Fit::Fill => {
            let k = if fit == Fit::Contain { (dw / sw).min(dh / sh) } else { (dw / sw).max(dh / sh) };
            let (w, h) = (sw * k, sh * k);
            ((dw - w) / 2.0, (dh - h) / 2.0, w, h)
        }
    }
}

impl Filters {
    pub fn is_neutral(&self) -> bool {
        *self == Filters::default()
    }

    /// Apply to one pixel (red, green, blue, alpha). Alpha is left alone.
    pub fn apply(&self, px: [u8; 4]) -> [u8; 4] {
        if self.is_neutral() {
            return px;
        }
        let mut c = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0];
        // hue: rotate around the grey axis
        if self.hue != 0.0 {
            let a = self.hue.to_radians();
            let (cs, sn) = (a.cos(), a.sin());
            let m = [
                [0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928],
                [0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283],
                [0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072],
            ];
            c = [m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2], m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2], m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2]];
        }
        let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for v in c.iter_mut() {
            *v = lum + (*v - lum) * self.saturation; // saturation
            *v = (*v - 0.5) * self.contrast + 0.5; // contrast
            *v += self.brightness; // brightness
            if self.invert {
                *v = 1.0 - *v;
            }
        }
        [(c[0].clamp(0.0, 1.0) * 255.0).round() as u8, (c[1].clamp(0.0, 1.0) * 255.0).round() as u8, (c[2].clamp(0.0, 1.0) * 255.0).round() as u8, px[3]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: u64 = 60_000;

    #[test]
    fn plain_playback_follows_the_clock_and_finishes() {
        let m = MediaSettings::default();
        assert_eq!(m.position(10_000, FILE), Pos { media_ms: 10_000, finished: false, loops: 0 });
        let end = m.position(60_000, FILE);
        assert!(end.finished && end.media_ms == FILE);
        assert!(m.position(999_999, FILE).finished);
    }

    #[test]
    fn trimming_plays_only_the_chosen_part() {
        let m = MediaSettings { trim_in_ms: 10_000, trim_out_ms: Some(20_000), ..Default::default() };
        assert_eq!(m.position(0, FILE).media_ms, 10_000);
        assert_eq!(m.position(5_000, FILE).media_ms, 15_000);
        assert!(m.position(10_000, FILE).finished);
        assert_eq!(m.play_length_ms(FILE), 10_000);
    }

    #[test]
    fn looping_wraps_inside_the_trim_window() {
        let m = MediaSettings { trim_in_ms: 10_000, trim_out_ms: Some(20_000), looping: true, ..Default::default() };
        let p = m.position(25_000, FILE);
        assert_eq!((p.media_ms, p.loops, p.finished), (15_000, 2, false));
    }

    #[test]
    fn speed_changes_how_long_it_takes() {
        let fast = MediaSettings { speed: 2.0, ..Default::default() };
        assert_eq!(fast.play_length_ms(FILE), 30_000);
        assert_eq!(fast.position(10_000, FILE).media_ms, 20_000);
        let slow = MediaSettings { speed: 0.5, ..Default::default() };
        assert_eq!(slow.play_length_ms(FILE), 120_000);
    }

    #[test]
    fn nonsense_values_are_made_safe() {
        let m = MediaSettings { speed: f32::NAN, volume: 9.0, trim_in_ms: 999_999, trim_out_ms: Some(5), filters: Filters { hue: -90.0, contrast: 50.0, ..Default::default() }, ..Default::default() }.sanitized(FILE);
        assert_eq!(m.speed, 1.0);
        assert_eq!(m.volume, 1.0);
        assert!(m.trim_in_ms < FILE);
        assert_eq!(m.trim_out_ms, None); // out before in: ignored
        assert_eq!(m.filters.hue, 270.0);
        assert_eq!(m.filters.contrast, 2.0);
        assert_eq!(MediaSettings::default().position(5, 0).media_ms, 0); // an empty file does not panic
    }

    #[test]
    fn fades_ramp_the_volume_at_both_ends() {
        let m = MediaSettings { fade_in_ms: 2000, fade_out_ms: 2000, curve: Curve::Linear, volume: 0.8, ..Default::default() };
        assert_eq!(m.gain_at(0, FILE), 0.0);
        assert!((m.gain_at(1000, FILE) - 0.4).abs() < 1e-5);
        assert!((m.gain_at(30_000, FILE) - 0.8).abs() < 1e-5);
        assert!((m.gain_at(59_000, FILE) - 0.4).abs() < 1e-5);
        assert_eq!(m.gain_at(60_000, FILE), 0.0);
        let looped = MediaSettings { looping: true, fade_out_ms: 2000, ..Default::default() };
        assert_eq!(looped.gain_at(59_999, FILE), 1.0); // no fade-out on a loop
    }

    #[test]
    fn equal_power_keeps_the_sum_of_two_crossing_songs_steady() {
        for i in 0..=10 {
            let p = i as f32 / 10.0;
            let (out, inn) = (fade_gain(Curve::EqualPower, 1.0 - p), fade_gain(Curve::EqualPower, p));
            assert!((out * out + inn * inn - 1.0).abs() < 1e-5);
        }
        for c in [Curve::Linear, Curve::EqualPower, Curve::Smooth] {
            assert_eq!(fade_gain(c, 0.0), 0.0);
            assert!((fade_gain(c, 1.0) - 1.0).abs() < 1e-6);
            assert_eq!(fade_gain(c, -4.0), 0.0);
        }
    }

    #[test]
    fn fit_modes() {
        // a 4:3 picture on a 16:9 screen
        let (x, y, w, h) = fit_rect(Fit::Contain, (400.0, 300.0), (1600.0, 900.0));
        assert_eq!((w, h, y), (1200.0, 900.0, 0.0));
        assert_eq!(x, 200.0);
        let (x, y, w, h) = fit_rect(Fit::Fill, (400.0, 300.0), (1600.0, 900.0));
        assert_eq!((w, h, x), (1600.0, 1200.0, 0.0));
        assert_eq!(y, -150.0);
        assert_eq!(fit_rect(Fit::Stretch, (1.0, 1.0), (1600.0, 900.0)), (0.0, 0.0, 1600.0, 900.0));
        assert_eq!(fit_rect(Fit::Center, (400.0, 300.0), (1600.0, 900.0)), (600.0, 300.0, 400.0, 300.0));
        let z = fit_rect(Fit::Contain, (0.0, 0.0), (100.0, 100.0)); // zero-size source: no division by zero
        assert!(z.2.is_finite() && z.3.is_finite());
    }

    #[test]
    fn filters() {
        let px = [200, 100, 50, 255];
        assert_eq!(Filters::default().apply(px), px);
        assert_eq!(Filters { invert: true, ..Default::default() }.apply([0, 255, 100, 7]), [255, 0, 155, 7]);
        let grey = Filters { saturation: 0.0, ..Default::default() }.apply(px);
        assert!(grey[0] == grey[1] && grey[1] == grey[2]);
        let bright = Filters { brightness: 1.0, ..Default::default() }.apply(px);
        assert_eq!(&bright[..3], &[255, 255, 255]);
        let dark = Filters { brightness: -1.0, ..Default::default() }.apply(px);
        assert_eq!(&dark[..3], &[0, 0, 0]);
        let hue = Filters { hue: 360.0, ..Default::default() }.apply(px); // a full turn changes nothing
        assert!((hue[0] as i32 - 200).abs() <= 1 && (hue[1] as i32 - 100).abs() <= 1);
        let red_to_other = Filters { hue: 120.0, ..Default::default() }.apply([255, 0, 0, 255]);
        assert!(red_to_other[1] > red_to_other[0]);
    }
}
