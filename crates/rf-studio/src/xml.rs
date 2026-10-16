//! A tiny, forgiving XML reader: just enough for song files (OpenLyrics, OpenSong).
//! Handles attributes, entities, CDATA, comments, self-closing tags. Namespaces are dropped
//! (`ol:song` reads as `song`). Not a validating parser, and it never panics on bad input.

#[derive(Clone, Debug, PartialEq)]
pub enum Child {
    Node(Node),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Child>,
}

impl Node {
    fn new(name: &str) -> Node {
        Node { name: name.to_string(), ..Default::default() }
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| if let Child::Node(n) = c { Some(n) } else { None })
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.nodes().find(|n| n.name == name)
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes().filter(move |n| n.name == name)
    }
    /// All the text inside, in order, without tags.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            match c {
                Child::Text(t) => s.push_str(t),
                Child::Node(n) => s.push_str(&n.text()),
            }
        }
        s
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        match rest.find(';') {
            Some(e) if e <= 10 => {
                let ent = &rest[1..e];
                let rep = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                    _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                match rep {
                    Some(c) => {
                        out.push(c);
                        rest = &rest[e + 1..];
                    }
                    None => {
                        out.push('&');
                        rest = &rest[1..];
                    }
                }
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn push_text(stack: &mut [Node], t: &str) {
    if t.is_empty() {
        return;
    }
    if let Some(top) = stack.last_mut() {
        top.children.push(Child::Text(t.to_string()));
    }
}

/// End of the tag that starts at `i` (index of its `>`), skipping `>` inside quotes.
fn tag_end(src: &str, i: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (k, b) in src.as_bytes()[i..].iter().enumerate() {
        match (quote, *b) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"') | (None, b'\'') => quote = Some(*b),
            (None, b'>') => return Some(i + k),
            _ => {}
        }
    }
    None
}

fn parse_tag(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim();
    let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
    let name = local(&inner[..name_end]).to_string();
    let mut attrs = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while !rest.is_empty() {
        let kend = rest.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(rest.len());
        let key = &rest[..kend];
        rest = rest[kend..].trim_start();
        let mut val = String::new();
        if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            if let Some(q) = r.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let body = &r[1..];
                let e = body.find(q).unwrap_or(body.len());
                val = decode_entities(&body[..e]);
                rest = body.get(e + 1..).unwrap_or("").trim_start();
            } else {
                let e = r.find(char::is_whitespace).unwrap_or(r.len());
                val = decode_entities(&r[..e]);
                rest = r[e..].trim_start();
            }
        }
        if !key.is_empty() {
            attrs.push((local(key).to_string(), val));
        }
    }
    (name, attrs)
}

/// Read the first element of an XML document.
pub fn parse(src: &str) -> Result<Node, String> {
    let src = src.trim_start_matches('\u{feff}');
    let mut stack: Vec<Node> = vec![Node::new("#root")];
    let mut i = 0;
    while i < src.len() {
        if src.as_bytes()[i] == b'<' {
            let here = &src[i..];
            if here.starts_with("<!--") {
                let e = here[4..].find("-->").ok_or("a comment is never closed")?;
                i += 4 + e + 3;
            } else if here.starts_with("<![CDATA[") {
                let e = here[9..].find("]]>").ok_or("a CDATA block is never closed")?;
                push_text(&mut stack, &here[9..9 + e]);
                i += 9 + e + 3;
            } else if here.starts_with("<?") || here.starts_with("<!") {
                let e = here.find('>').ok_or("a declaration is never closed")?;
                i += e + 1;
            } else {
                let end = tag_end(src, i).ok_or("a tag is never closed")?;
                let inner = &src[i + 1..end];
                i = end + 1;
                if let Some(name) = inner.strip_prefix('/') {
                    let name = local(name.trim()).to_string();
                    if let Some(pos) = stack.iter().rposition(|n| n.name == name) {
                        if pos > 0 {
                            while stack.len() > pos {
                                let n = stack.pop().unwrap();
                                stack.last_mut().unwrap().children.push(Child::Node(n));
                            }
                        }
                    }
                } else {
                    let selfclose = inner.trim_end().ends_with('/');
                    let inner = inner.trim_end().trim_end_matches('/');
                    let (name, attrs) = parse_tag(inner);
                    if name.is_empty() {
                        continue;
                    }
                    let node = Node { name, attrs, children: Vec::new() };
                    if selfclose {
                        stack.last_mut().unwrap().children.push(Child::Node(node));
                    } else {
                        stack.push(node);
                    }
                }
            }
        } else {
            let next = src[i..].find('<').map(|p| i + p).unwrap_or(src.len());
            push_text(&mut stack, &decode_entities(&src[i..next]));
            i = next;
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(Child::Node(n));
    }
    let root = stack.pop().unwrap();
    root.children.into_iter().find_map(|c| if let Child::Node(n) = c { Some(n) } else { None }).ok_or_else(|| "no XML element found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_elements_attributes_and_entities() {
        let n = parse("<?xml version=\"1.0\"?><!-- hi --><a x=\"1&amp;2\"><b>one &lt; two</b><c/><b>tail</b></a>").unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attr("x"), Some("1&2"));
        assert_eq!(n.children_named("b").count(), 2);
        assert_eq!(n.child("b").unwrap().text(), "one < two");
        assert!(n.child("c").is_some());
    }

    #[test]
    fn namespaces_cdata_and_numeric_entities() {
        let n = parse("<ol:song xmlns:ol=\"urn:x\"><ol:t><![CDATA[a <b> c]]>&#65;&#x42;</ol:t></ol:song>").unwrap();
        assert_eq!(n.name, "song");
        assert_eq!(n.child("t").unwrap().text(), "a <b> cAB");
    }

    #[test]
    fn broken_input_gives_errors_or_best_effort_never_a_panic() {
        assert!(parse("").is_err());
        assert!(parse("just words").is_err());
        assert!(parse("<a><b>open").is_ok()); // unclosed tags are closed for you
        assert!(parse("<a x=\"1>").is_err());
        assert!(parse("<!-- never ends").is_err());
        let n = parse("<a>1 &bogus; 2 & 3</a>").unwrap();
        assert_eq!(n.text(), "1 &bogus; 2 & 3");
        let n = parse("<a></b></a>").unwrap(); // stray close tag ignored
        assert_eq!(n.name, "a");
    }

    #[test]
    fn unicode_text_survives() {
        let n = parse("<a>Señor — Ngọc ✝</a>").unwrap();
        assert_eq!(n.text(), "Señor — Ngọc ✝");
    }
}
