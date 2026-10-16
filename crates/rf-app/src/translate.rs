//! Live caption translation (v0.835.0).
//!
//! RhemaFlow has no translation model of its own. The captions can be sent, line by line, to a translation service
//! the church chooses: LibreTranslate (free, can run on the church's own computer so nothing leaves the building),
//! DeepL or Google Cloud Translation (their own keys). This file builds the request and reads the answer; the network
//! call uses the system `curl` like the account code. NOT tested against a live service in this version.

use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Libre,
    DeepL,
    Google,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Libre, Provider::DeepL, Provider::Google];
    pub fn label(self) -> &'static str {
        match self {
            Provider::Libre => "LibreTranslate (free, can run on this Mac)",
            Provider::DeepL => "DeepL (needs a key)",
            Provider::Google => "Google Cloud Translation (needs a key)",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Provider::Libre => "libre",
            Provider::DeepL => "deepl",
            Provider::Google => "google",
        }
    }
    pub fn from_key(k: &str) -> Provider {
        Self::ALL.into_iter().find(|p| p.key() == k).unwrap_or(Provider::Libre)
    }
    pub fn default_endpoint(self) -> &'static str {
        match self {
            Provider::Libre => "http://localhost:5000/translate",
            Provider::DeepL => "https://api-free.deepl.com/v2/translate",
            Provider::Google => "https://translation.googleapis.com/language/translate/v2",
        }
    }
}

/// (code, name) of languages offered in the picker.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("es", "Spanish"), ("fr", "French"), ("pt", "Portuguese"), ("de", "German"), ("it", "Italian"), ("zh", "Chinese"), ("ko", "Korean"),
    ("ru", "Russian"), ("ar", "Arabic"), ("hi", "Hindi"), ("pl", "Polish"), ("ro", "Romanian"), ("sw", "Swahili"), ("tl", "Filipino"),
    ("uk", "Ukrainian"), ("yo", "Yoruba"), ("tr", "Turkish"), ("vi", "Vietnamese"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub provider: Provider,
    pub endpoint: String,
    pub api_key: String,
    pub target: String,
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { provider: Provider::Libre, endpoint: Provider::Libre.default_endpoint().into(), api_key: String::new(), target: "es".into(), enabled: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Only these web addresses may be called: https anywhere, or plain http to this computer / the local network.
pub fn endpoint_ok(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    if u.starts_with("https://") {
        return true;
    }
    if let Some(rest) = u.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or("");
        return host == "localhost" || host == "127.0.0.1" || host.starts_with("192.168.") || host.starts_with("10.") || host.ends_with(".local");
    }
    false
}

pub fn build_request(cfg: &Config, text: &str) -> Request {
    let target = cfg.target.trim();
    match cfg.provider {
        Provider::Libre => {
            let mut b = json!({"q": text, "source": "auto", "target": target, "format": "text"});
            if !cfg.api_key.trim().is_empty() {
                b["api_key"] = json!(cfg.api_key.trim());
            }
            Request { url: cfg.endpoint.trim().into(), headers: vec![("Content-Type".into(), "application/json".into())], body: b.to_string() }
        }
        Provider::DeepL => Request {
            url: cfg.endpoint.trim().into(),
            headers: vec![("Content-Type".into(), "application/json".into()), ("Authorization".into(), format!("DeepL-Auth-Key {}", cfg.api_key.trim()))],
            body: json!({"text": [text], "target_lang": target.to_uppercase()}).to_string(),
        },
        Provider::Google => Request {
            url: format!("{}?key={}", cfg.endpoint.trim(), cfg.api_key.trim()),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: json!({"q": text, "target": target, "format": "text"}).to_string(),
        },
    }
}

/// Read the translated text from any of the three answer shapes.
pub fn parse_response(body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|_| "The translation service sent something unreadable.".to_string())?;
    if let Some(t) = v["translatedText"].as_str() {
        return Ok(t.trim().to_string());
    }
    if let Some(t) = v["translations"][0]["text"].as_str() {
        return Ok(t.trim().to_string());
    }
    if let Some(t) = v["data"]["translations"][0]["translatedText"].as_str() {
        return Ok(decode_entities(t.trim()));
    }
    let why = v["error"].as_str().or_else(|| v["error"]["message"].as_str()).or_else(|| v["message"].as_str()).unwrap_or("no translation in the answer");
    Err(format!("The translation service said: {why}"))
}

fn decode_entities(s: &str) -> String {
    s.replace("&#39;", "'").replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
}

/// Send one line. Blocks (call from a helper thread).
pub fn translate_blocking(cfg: &Config, text: &str) -> Result<String, String> {
    if !endpoint_ok(&cfg.endpoint) {
        return Err("The translation address must be https:// (or http:// to this computer or your local network).".into());
    }
    let r = build_request(cfg, text);
    let mut c = std::process::Command::new("curl");
    c.args(["-sS", "--max-time", "6", "-X", "POST", &r.url]);
    for (k, v) in &r.headers {
        c.args(["-H", &format!("{k}: {v}")]);
    }
    c.args(["-d", &r.body]);
    let out = c.stdin(std::process::Stdio::null()).output().map_err(|_| "Could not start curl to reach the translation service.".to_string())?;
    if !out.status.success() {
        return Err("Could not reach the translation service.".into());
    }
    parse_response(&String::from_utf8_lossy(&out.stdout))
}

/// What the captions show: the translation if there is one for this line, otherwise the original.
pub fn caption_text(original: &str, translated: Option<&str>) -> String {
    match translated {
        Some(t) if !t.trim().is_empty() => t.to_string(),
        _ => original.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(p: Provider) -> Config {
        Config { provider: p, endpoint: p.default_endpoint().into(), api_key: "K3Y".into(), target: "es".into(), enabled: true }
    }

    #[test]
    fn each_service_gets_the_request_it_expects() {
        let r = build_request(&cfg(Provider::Libre), "Grace is free");
        let b: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!((b["q"].as_str(), b["target"].as_str(), b["api_key"].as_str()), (Some("Grace is free"), Some("es"), Some("K3Y")));
        let mut no_key = cfg(Provider::Libre);
        no_key.api_key.clear();
        assert!(serde_json::from_str::<Value>(&build_request(&no_key, "x").body).unwrap().get("api_key").is_none());
        let d = build_request(&cfg(Provider::DeepL), "Hello");
        assert!(d.headers.iter().any(|(k, v)| k == "Authorization" && v == "DeepL-Auth-Key K3Y"));
        assert!(d.body.contains("\"target_lang\":\"ES\""));
        let g = build_request(&cfg(Provider::Google), "Hello");
        assert!(g.url.ends_with("?key=K3Y"));
    }

    #[test]
    fn every_answer_shape_is_read_and_errors_are_explained() {
        assert_eq!(parse_response(r#"{"translatedText":"La gracia es libre"}"#).unwrap(), "La gracia es libre");
        assert_eq!(parse_response(r#"{"translations":[{"detected_source_language":"EN","text":"Hola"}]}"#).unwrap(), "Hola");
        assert_eq!(parse_response(r#"{"data":{"translations":[{"translatedText":"Dios no &quot;ama&quot; l&#39;ombre"}]}}"#).unwrap(), "Dios no \"ama\" l'ombre");
        assert!(parse_response(r#"{"error":"Invalid API key"}"#).unwrap_err().contains("Invalid API key"));
        assert!(parse_response("<html>").unwrap_err().contains("unreadable"));
    }

    #[test]
    fn only_safe_addresses_are_called() {
        assert!(endpoint_ok("https://api.deepl.com/v2/translate"));
        assert!(endpoint_ok("http://localhost:5000/translate"));
        assert!(endpoint_ok("http://192.168.1.20:5000/translate"));
        assert!(!endpoint_ok("http://example.com/translate"));
        assert!(!endpoint_ok("file:///etc/passwd"));
        assert!(!endpoint_ok("ftp://x"));
        let mut c = cfg(Provider::Libre);
        c.endpoint = "http://evil.example.com/t".into();
        assert!(translate_blocking(&c, "hi").unwrap_err().contains("https://"));
    }

    #[test]
    fn captions_fall_back_to_the_original_when_there_is_no_translation_yet() {
        assert_eq!(caption_text("grace", Some("gracia")), "gracia");
        assert_eq!(caption_text("grace", None), "grace");
        assert_eq!(caption_text("grace", Some("  ")), "grace");
        assert_eq!(Provider::from_key("deepl"), Provider::DeepL);
        assert_eq!(Provider::from_key("???"), Provider::Libre);
    }
}
