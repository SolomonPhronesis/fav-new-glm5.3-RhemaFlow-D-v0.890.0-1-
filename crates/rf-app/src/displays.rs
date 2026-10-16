//! Screens (v0.814.0): find the connected displays and put the congregation stage on one
//! of them: the operator window stays on one screen, the congregation stage goes to another.
//!
//! How: macOS is asked for its screens with a one-line JavaScript-for-Automation script
//! (`osascript -l JavaScript`, no extra dependency). The stage is then a BORDERLESS window
//! sized exactly to the chosen display. We do not use macOS native full screen: it makes a
//! separate desktop ("Space") for the window, which is what made "exit full screen" flaky.
//! A borderless window on screen 2 leaves screen 1 (your control window) alone, and leaving
//! "full screen" is just putting the window back.

use serde::Deserialize;

/// A connected display, in the coordinates the window system wants: points, origin at the
/// top-left of the MAIN display, y growing downwards.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub primary: bool,
}

impl Display {
    pub fn label(&self, index: usize) -> String {
        let role = if self.primary { "main" } else { "extended" };
        format!("Screen {} \u{2014} {} ({}\u{00d7}{}, {})", index + 1, self.name, self.w as i32, self.h as i32, role)
    }
}

#[derive(Deserialize)]
struct Raw {
    name: Option<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Cocoa gives each screen a frame with the origin at the BOTTOM-left of the main screen and y
/// growing upwards. Window systems want top-left and y downwards:
/// `y_top = main_height - (y + h)`. The first screen in the list is the main one.
pub fn from_cocoa_json(json: &str) -> Result<Vec<Display>, String> {
    let raws: Vec<Raw> = serde_json::from_str(json.trim()).map_err(|e| format!("cannot read the screen list: {e}"))?;
    if raws.is_empty() {
        return Err("no screens reported".into());
    }
    let main_h = raws[0].h;
    Ok(raws
        .iter()
        .enumerate()
        .map(|(i, r)| Display {
            name: r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Display {}", i + 1)),
            x: r.x as f32,
            y: (main_h - (r.y + r.h)) as f32,
            w: r.w as f32,
            h: r.h as f32,
            primary: i == 0,
        })
        .collect())
}

const JXA: &str = r#"ObjC.import("AppKit");var s=$.NSScreen.screens,o=[];for(var i=0;i<s.count;i++){var c=s.objectAtIndex(i),f=c.frame,n="";try{n=ObjC.unwrap(c.localizedName)}catch(e){}o.push({name:n,x:f.origin.x,y:f.origin.y,w:f.size.width,h:f.size.height})}JSON.stringify(o)"#;

/// Ask the system for its screens. Empty on other systems or if the call fails (the stage then
/// just opens as a normal window the user can drag).
pub fn detect() -> Vec<Display> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", JXA]).output() {
            if out.status.success() {
                if let Ok(d) = from_cocoa_json(&String::from_utf8_lossy(&out.stdout)) {
                    return d;
                }
            }
        }
    }
    let _ = JXA;
    Vec::new()
}

/// Which display the stage should use. `want` = the name the operator chose last time.
/// With nothing chosen: the first EXTENDED display if there is one (screen 2, which is where
/// the stage usually goes), otherwise the main one.
pub fn choose(displays: &[Display], want: Option<&str>) -> Option<usize> {
    if displays.is_empty() {
        return None;
    }
    if let Some(w) = want {
        if let Some(i) = displays.iter().position(|d| d.name == w) {
            return Some(i);
        }
    }
    Some(displays.iter().position(|d| !d.primary).unwrap_or(0))
}

/// Where the stage goes: (position, size). Full output covers the display exactly; the normal
/// window is 960x540 inset on it.
pub fn place(d: &Display, output: bool) -> ((f32, f32), (f32, f32)) {
    if output {
        ((d.x, d.y), (d.w, d.h))
    } else {
        let (w, h) = (960.0_f32.min(d.w - 80.0), 540.0_f32.min(d.h - 120.0));
        ((d.x + 60.0, d.y + 80.0), (w.max(320.0), h.max(180.0)))
    }
}


/// A window system does not always give the size it was asked for (a screen with a different
/// pixel density from the one the window opened on can shrink it to, say, 80%). So after asking,
/// we LOOK at what we got and ask again with the difference built in. `cmd` = what we last asked
/// for, `want` = what we need, `got` = what the window really is. None = close enough (2 points).
pub fn corrected_size(cmd: (f32, f32), want: (f32, f32), got: (f32, f32)) -> Option<(f32, f32)> {
    if got.0 < 1.0 || got.1 < 1.0 || ((want.0 - got.0).abs() <= 2.0 && (want.1 - got.1).abs() <= 2.0) {
        return None;
    }
    let fix = |c: f32, w: f32, g: f32| (c * w / g).clamp(w * 0.25, w * 4.0);
    Some((fix(cmd.0, want.0, got.0), fix(cmd.1, want.1, got.1)))
}

/// Same idea for the top-left corner: move by the missing distance.
pub fn corrected_pos(cmd: (f32, f32), want: (f32, f32), got: (f32, f32)) -> Option<(f32, f32)> {
    if (want.0 - got.0).abs() <= 2.0 && (want.1 - got.1).abs() <= 2.0 {
        return None;
    }
    Some((cmd.0 + (want.0 - got.0), cmd.1 + (want.1 - got.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // a laptop (1440x900) with a projector (1920x1080) to its right, bottoms aligned
    const RIGHT: &str = r#"[{"name":"Built-in Retina Display","x":0,"y":0,"w":1440,"h":900},{"name":"Epson Projector","x":1440,"y":0,"w":1920,"h":1080}]"#;

    #[test]
    fn cocoa_coordinates_become_top_left_coordinates() {
        let d = from_cocoa_json(RIGHT).unwrap();
        assert_eq!(d.len(), 2);
        assert!(d[0].primary && !d[1].primary);
        assert_eq!((d[0].x, d[0].y), (0.0, 0.0));
        // bottoms aligned: the taller projector reaches 180 points ABOVE the laptop's top edge
        assert_eq!((d[1].x, d[1].y, d[1].w, d[1].h), (1440.0, -180.0, 1920.0, 1080.0));
    }

    #[test]
    fn a_screen_above_and_to_the_left_work_too() {
        let above = r#"[{"name":"A","x":0,"y":0,"w":1440,"h":900},{"name":"B","x":0,"y":900,"w":1920,"h":1080}]"#;
        let d = from_cocoa_json(above).unwrap();
        assert_eq!(d[1].y, -1080.0);
        let left = r#"[{"name":"A","x":0,"y":0,"w":1440,"h":900},{"name":"B","x":-1920,"y":0,"w":1920,"h":1080}]"#;
        let d = from_cocoa_json(left).unwrap();
        assert_eq!((d[1].x, d[1].y), (-1920.0, -180.0));
    }

    #[test]
    fn screen_2_is_the_default_and_a_remembered_name_wins() {
        let d = from_cocoa_json(RIGHT).unwrap();
        assert_eq!(choose(&d, None), Some(1), "extended screen first");
        assert_eq!(choose(&d, Some("Built-in Retina Display")), Some(0));
        assert_eq!(choose(&d, Some("A projector that was unplugged")), Some(1), "unknown name falls back");
        let one = from_cocoa_json(r#"[{"name":"Only","x":0,"y":0,"w":1280,"h":800}]"#).unwrap();
        assert_eq!(choose(&one, None), Some(0), "a single screen is still usable");
        assert_eq!(choose(&[], None), None);
    }

    #[test]
    fn output_covers_the_display_exactly_and_the_window_is_inset() {
        let d = from_cocoa_json(RIGHT).unwrap();
        assert_eq!(place(&d[1], true), ((1440.0, -180.0), (1920.0, 1080.0)));
        let ((x, y), (w, h)) = place(&d[1], false);
        assert!(x > 1440.0 && y > -180.0 && w <= 960.0 && h <= 540.0);
        // a small screen never gets a window bigger than itself
        let tiny = Display { name: "t".into(), x: 0.0, y: 0.0, w: 800.0, h: 500.0, primary: true };
        let (_, (w, h)) = place(&tiny, false);
        assert!(w <= 800.0 && h <= 500.0);
    }

    #[test]
    fn bad_input_is_a_clear_error_not_a_panic() {
        assert!(from_cocoa_json("").is_err());
        assert!(from_cocoa_json("[]").is_err());
        assert!(from_cocoa_json("{\"x\":1}").is_err());
        // a screen with no name still gets one
        let d = from_cocoa_json(r#"[{"name":"","x":0,"y":0,"w":10,"h":10}]"#).unwrap();
        assert_eq!(d[0].name, "Display 1");
        assert!(d[0].label(0).starts_with("Screen 1"));
    }

    #[test]
    fn a_window_that_comes_out_at_80_percent_is_asked_again_until_it_fills_the_screen() {
        let want = (1920.0, 1080.0);
        // a fake window system that always delivers 80% of what is asked
        let (mut cmd, mut tries) = (want, 0);
        let mut got = (cmd.0 * 0.8, cmd.1 * 0.8);
        while let Some(next) = corrected_size(cmd, want, got) {
            cmd = next;
            got = (cmd.0 * 0.8, cmd.1 * 0.8);
            tries += 1;
            assert!(tries < 5, "must settle quickly");
        }
        assert!((got.0 - 1920.0).abs() <= 2.0 && (got.1 - 1080.0).abs() <= 2.0 && tries >= 1);
    }

    #[test]
    fn a_correct_window_is_left_alone_and_nonsense_is_ignored() {
        assert_eq!(corrected_size((1920.0, 1080.0), (1920.0, 1080.0), (1919.0, 1081.0)), None);
        assert_eq!(corrected_size((1920.0, 1080.0), (1920.0, 1080.0), (0.0, 0.0)), None);
        // a wild reading cannot send the window to 100x its size
        let c = corrected_size((1920.0, 1080.0), (1920.0, 1080.0), (10.0, 10.0)).unwrap();
        assert!(c.0 <= 1920.0 * 4.0 && c.1 <= 1080.0 * 4.0);
    }

    #[test]
    fn a_window_that_lands_in_the_wrong_place_is_nudged_by_the_missing_distance() {
        assert_eq!(corrected_pos((1440.0, 0.0), (1440.0, 0.0), (1440.5, 1.0)), None);
        assert_eq!(corrected_pos((1440.0, 0.0), (1440.0, 0.0), (1152.0, 30.0)), Some((1728.0, -30.0)));
    }
}
