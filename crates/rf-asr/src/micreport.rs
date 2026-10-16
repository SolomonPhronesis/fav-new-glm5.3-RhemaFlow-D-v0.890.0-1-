//! Microphone check (v0.818.0): listen to a recording and say, in plain words, what is wrong with the SOUND.
//!
//! Why: speech recognition can only be as good as the audio it is given, and in a church the usual
//! culprits are not the software: the mic is too far away, the gain is wrong, a fan or a crowd is
//! louder than it seems, a cable hums. This turns a recording (a minute of the preacher, or a
//! whole service) into a short list of findings with a fix for each. Pure maths, no microphone
//! access, so it is tested with real speech plus deliberately broken copies of it.
//!
//! Levels are in dBFS: 0 dB is the loudest a recording can be, -20 dB is a comfortable voice,
//! -60 dB is nearly silence.

use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub name: &'static str,
    pub level: Level,
    /// what was measured, in words and numbers
    pub value: String,
    /// what to do about it (empty when all is well)
    pub advice: String,
}

#[derive(Clone, Debug)]
pub struct MicReport {
    pub seconds: f32,
    pub speech_seconds: f32,
    pub findings: Vec<Finding>,
    /// the worst of the findings
    pub verdict: Level,
}

const MIN_SECONDS: f32 = 5.0;
const FRAME_MS: usize = 20;

fn finding(name: &'static str, level: Level, value: impl Into<String>, advice: impl Into<String>) -> Finding {
    Finding { name, level, value: value.into(), advice: advice.into() }
}

fn db(rms: f32) -> f32 {
    20.0 * rms.max(1e-5).log10()
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return -100.0;
    }
    sorted[((sorted.len() - 1) as f32 * p).round() as usize]
}

/// Power at one frequency (Goertzel), after a Hann window so a strong tone does not leak into its neighbours.
fn tone_power(x: &[f32], hz: f32, rate: f32) -> f32 {
    let n = x.len();
    let w = 2.0 * PI * hz / rate;
    let (c, s) = (w.cos(), w.sin());
    let (mut re, mut im) = (0.0f32, 0.0f32);
    // direct correlation (n is a few seconds at most): simple and exact
    let (mut cr, mut ci) = (1.0f32, 0.0f32);
    for (i, v) in x.iter().enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * PI * i as f32 / (n.max(2) - 1) as f32).cos();
        re += v * hann * cr;
        im -= v * hann * ci;
        let (nr, ni) = (cr * c - ci * s, cr * s + ci * c);
        cr = nr;
        ci = ni;
    }
    (re * re + im * im) / (n.max(1) as f32 * n.max(1) as f32)
}

/// Which mains frequency hums in this (voice-free) audio and how far above its surroundings the
/// strongest of its first three harmonics stands. The surroundings are the TYPICAL (median) power
/// 12 and 24 Hz either side, so a little rumble or a stray word cannot hide a hum.
fn hum(quiet: &[f32], rate: f32) -> Option<(f32, f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    for mains in [50.0f32, 60.0] {
        let mut top: Option<(f32, f32)> = None; // (how far above its surroundings, its own level in dB)
        for h in 1..=3 {
            let f = mains * h as f32;
            if f + 30.0 >= rate / 2.0 {
                continue;
            }
            let mut around: Vec<f32> = [-24.0f32, -12.0, 12.0, 24.0].iter().filter(|o| f + **o > 5.0).map(|o| tone_power(quiet, f + o, rate)).collect();
            if around.len() < 3 {
                continue;
            }
            around.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let typical = (around[around.len() / 2 - 1] + around[around.len() / 2]) / 2.0;
            let p = tone_power(quiet, f, rate);
            let ratio_db = 10.0 * (p / typical.max(1e-14)).max(1e-9).log10();
            // a steady tone of amplitude A through a Hann window gives power A^2/16, and its rms is A/sqrt(2)
            let rms_db = 20.0 * (4.0 * p.sqrt() / 2f32.sqrt()).max(1e-6).log10();
            if top.map_or(true, |t| ratio_db > t.0) {
                top = Some((ratio_db, rms_db));
            }
        }
        if let Some((r, level)) = top {
            if best.map_or(true, |b| r > b.1) {
                best = Some((mains, r, level));
            }
        }
    }
    best
}

pub fn analyze(samples: &[f32], sample_rate: u32) -> MicReport {
    let x: Vec<f32> = samples.iter().map(|s| if s.is_finite() { *s } else { 0.0 }).collect();
    if sample_rate < 1000 {
        return report(0.0, 0.0, vec![finding("Recording", Level::Bad, "unknown sample rate", "This recording could not be read properly. Record it again as a normal WAV file.")]);
    }
    let rate = sample_rate as f32;
    let seconds = x.len() as f32 / rate;
    // a constant offset makes every moment look loud: measure it, then look at the sound around it
    let mean = x.iter().sum::<f32>() / x.len().max(1) as f32;
    let dc_finding = (mean.abs() >= 0.02).then(|| {
        let lvl = if mean.abs() >= 0.05 { Level::Bad } else { Level::Warn };
        finding("DC offset", lvl, format!("the signal sits {mean:.2} away from the centre"), "The input has a constant offset, which wastes headroom. It is usually a faulty cable or a cheap adapter: try another.")
    });
    let x: Vec<f32> = x.iter().map(|s| s - mean).collect();
    if seconds < MIN_SECONDS {
        return report(
            seconds,
            0.0,
            vec![finding("Length", Level::Bad, format!("only {seconds:.1} seconds"), "Too short to judge. Record at least 10 seconds of the preacher speaking at the normal volume, from where the microphone really is.")],
        );
    }
    // 20 ms frames
    let fl = (sample_rate as usize * FRAME_MS / 1000).max(1);
    let frames: Vec<f32> = x.chunks(fl).filter(|c| c.len() == fl).map(|c| db((c.iter().map(|s| s * s).sum::<f32>() / fl as f32).sqrt())).collect();
    let mut sorted = frames.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let noise_db = percentile(&sorted, 0.10);
    let threshold = (noise_db + 6.0).max(-55.0);
    let speech: Vec<usize> = (0..frames.len()).filter(|&i| frames[i] > threshold).collect();
    let speech_seconds = speech.len() as f32 * FRAME_MS as f32 / 1000.0;
    let mut out: Vec<Finding> = Vec::new();

    // clipping first: if the sound is cut off, nothing else can be trusted
    let clipped = x.iter().filter(|s| s.abs() >= 0.98).count() as f32 / x.len() as f32;
    let (clip_lvl, clip_advice) = if clipped < 0.0002 {
        (Level::Good, "")
    } else if clipped < 0.002 {
        (Level::Warn, "Some loud moments are being cut off (clipping). Lower the input gain a little.")
    } else {
        (Level::Bad, "The sound is being cut off at the loud parts, which makes words unreadable for the computer. Lower the input gain (on the mixer first, then the computer) until the loudest shout stays below the top.")
    };
    let clip_finding = finding("Distortion", clip_lvl, format!("{:.2}% of the sound is clipped", clipped * 100.0), clip_advice);

    if speech_seconds < 1.5 {
        if clip_lvl == Level::Bad {
            // too distorted to tell a voice from the background: say THAT, not "no voice"
            out.push(clip_finding);
            out.extend(dc_finding);
            return report(seconds, speech_seconds, out);
        }
        out.push(finding(
            "Speech",
            Level::Bad,
            format!("only {speech_seconds:.1} s of anything that sounds like a voice"),
            "I could not hear a voice. Check the microphone is plugged in and not muted, that the right input is chosen on the computer, and that someone really spoke while it recorded.",
        ));
        out.extend(dc_finding);
        return report(seconds, speech_seconds, out);
    }
    out.push(finding("Speech", Level::Good, format!("{speech_seconds:.0} s of speech heard"), ""));
    out.push(clip_finding);

    // loudness of the voice itself
    let power: f32 = speech.iter().map(|&i| 10f32.powf(frames[i] / 10.0)).sum::<f32>() / speech.len() as f32;
    let speech_db = 10.0 * power.max(1e-10).log10();
    let (lvl, advice) = if speech_db < -40.0 {
        (Level::Bad, "The voice is very quiet. Move the microphone closer to the speaker's mouth (a hand's width for a handheld, a lapel mic on the chest), and raise the input gain on the mixer or in the computer's sound settings until a normal speaking voice sits around -20 dB.")
    } else if speech_db < -32.0 {
        (Level::Warn, "The voice is a bit quiet. Raise the input gain a little or move the microphone closer.")
    } else if speech_db > -8.0 {
        (Level::Bad, "The voice is far too loud and will distort. Lower the input gain.")
    } else if speech_db > -12.0 {
        (Level::Warn, "The voice is rather loud. Lower the input gain a little so loud moments do not distort.")
    } else {
        (Level::Good, "")
    };
    out.push(finding("Loudness", lvl, format!("voice at {speech_db:.0} dB (aim for about -20)"), advice));

    // how far the voice stands above the background
    let snr = speech_db - noise_db;
    let (lvl, advice) = if snr >= 25.0 {
        (Level::Good, "")
    } else if snr >= 15.0 {
        (Level::Warn, "There is noticeable background noise (fans, people, the room). A closer microphone, or a feed straight from the sound desk, will help more than any setting.")
    } else {
        (Level::Bad, "The background is almost as loud as the voice, so the computer is guessing. Put the microphone much closer to the speaker (a headset or lapel microphone is best), or take the sound from the mixing desk instead of a microphone that listens to the whole room.")
    };
    out.push(finding("Background noise", lvl, format!("voice is {snr:.0} dB above the background (want 25 or more)"), advice));

    // mains hum, listened for in the gaps between words
    let quiet: Vec<f32> = x.chunks(fl).zip(frames.iter()).filter(|(c, d)| c.len() == fl && **d <= threshold).flat_map(|(c, _)| c.iter().copied()).take(sample_rate as usize * 4).collect();
    if quiet.len() as f32 >= rate * 0.8 {
        if let Some((mains, over, hum_db)) = hum(&quiet, rate) {
            // a hum matters by how loud it is NEXT TO THE VOICE, not by whether it exists:
            // many old recordings hum 35 dB below the voice and recognise perfectly
            let below_voice = speech_db - hum_db;
            if over >= 15.0 && below_voice < 32.0 {
                let lvl = if below_voice < 20.0 { Level::Bad } else { Level::Warn };
                let name = if (mains - 50.0).abs() < 1.0 { "50 Hz (UK mains)" } else { "60 Hz (mains)" };
                out.push(finding(
                    "Hum",
                    lvl,
                    format!("{name} hum, only {below_voice:.0} dB quieter than the voice (want 32 or more)"),
                    "An electrical hum is in the signal. Try a different (balanced, XLR) cable, a different power socket, keep the microphone cable away from power cables and light dimmers, or use a ground-lift or DI box.",
                ));
            }
        }
    }

    out.extend(dc_finding);
    report(seconds, speech_seconds, out)
}

fn report(seconds: f32, speech_seconds: f32, findings: Vec<Finding>) -> MicReport {
    let verdict = findings.iter().map(|f| f.level).max().unwrap_or(Level::Good);
    MicReport { seconds, speech_seconds, findings, verdict }
}

impl MicReport {
    /// A short report in plain English.
    pub fn to_text(&self) -> String {
        let mut s = format!("Microphone check ({:.0} s recorded, {:.0} s of speech)\n", self.seconds, self.speech_seconds);
        for f in &self.findings {
            let tag = match f.level {
                Level::Good => "[ OK ]",
                Level::Warn => "[CHECK]",
                Level::Bad => "[ FIX ]",
            };
            s.push_str(&format!("{tag} {}: {}\n", f.name, f.value));
            if !f.advice.is_empty() {
                s.push_str(&format!("         -> {}\n", f.advice));
            }
        }
        s.push_str(match self.verdict {
            Level::Good => "Verdict: the sound looks good. If words are still wrong, the cause is the speech model, not the microphone.\n",
            Level::Warn => "Verdict: usable, but fix the [CHECK] items for better accuracy.\n",
            Level::Bad => "Verdict: the sound needs fixing first. Software cannot repair this.\n",
        });
        s
    }
}
