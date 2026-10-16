//! Feature 15: look after a big song library. Tags and categories, key and tempo, how often and when each song was
//! used, duplicates, merging two copies, and find-and-replace across every song (a wrong church name, a spelling).

use rf_service::songs::{Library, Section, Song};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongMeta {
    pub tags: Vec<String>,
    pub category: String,
    pub key: String,
    pub bpm: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub songs: HashMap<u64, SongMeta>,
    /// (song id, when it was shown: seconds since 1970)
    pub usage: Vec<(u64, i64)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stats {
    pub count: usize,
    pub last_used: Option<i64>,
}

fn tidy_tag(t: &str) -> String {
    t.trim().trim_start_matches('#').trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Meta {
    pub fn tag(&mut self, id: u64, tag: &str) -> bool {
        let t = tidy_tag(tag);
        if t.is_empty() {
            return false;
        }
        let m = self.songs.entry(id).or_default();
        if m.tags.contains(&t) {
            return false;
        }
        m.tags.push(t);
        m.tags.sort();
        true
    }
    pub fn untag(&mut self, id: u64, tag: &str) -> bool {
        let t = tidy_tag(tag);
        match self.songs.get_mut(&id) {
            Some(m) => {
                let n = m.tags.len();
                m.tags.retain(|x| *x != t);
                m.tags.len() != n
            }
            None => false,
        }
    }
    pub fn set_key(&mut self, id: u64, key: &str) {
        self.songs.entry(id).or_default().key = key.trim().to_string();
    }
    pub fn set_bpm(&mut self, id: u64, bpm: Option<u32>) {
        self.songs.entry(id).or_default().bpm = bpm.filter(|b| (30..=300).contains(b));
    }
    pub fn set_category(&mut self, id: u64, c: &str) {
        self.songs.entry(id).or_default().category = c.trim().to_string();
    }

    /// tag -> song ids
    pub fn tag_index(&self) -> BTreeMap<String, Vec<u64>> {
        let mut out: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for (id, m) in &self.songs {
            for t in &m.tags {
                out.entry(t.clone()).or_default().push(*id);
            }
        }
        for v in out.values_mut() {
            v.sort();
        }
        out
    }

    pub fn record_use(&mut self, id: u64, at: i64) {
        self.usage.push((id, at));
    }
    pub fn stats(&self, id: u64) -> Stats {
        let u: Vec<i64> = self.usage.iter().filter(|(i, _)| *i == id).map(|(_, t)| *t).collect();
        Stats { count: u.len(), last_used: u.iter().copied().max() }
    }
    /// The most used songs since `since` (seconds since 1970), most first, ties by most recent.
    pub fn most_used(&self, since: i64, n: usize) -> Vec<(u64, usize)> {
        let mut c: HashMap<u64, (usize, i64)> = HashMap::new();
        for (id, t) in self.usage.iter().filter(|(_, t)| *t >= since) {
            let e = c.entry(*id).or_insert((0, i64::MIN));
            e.0 += 1;
            e.1 = e.1.max(*t);
        }
        let mut v: Vec<(u64, usize, i64)> = c.into_iter().map(|(i, (n, l))| (i, n, l)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
        v.into_iter().take(n).map(|(i, c, _)| (i, c)).collect()
    }
    /// Songs not shown since `since` (or never): a clean-up list.
    pub fn unused_since(&self, lib: &Library, since: i64) -> Vec<u64> {
        lib.songs().iter().map(|s| s.id).filter(|id| self.stats(*id).last_used.map_or(true, |t| t < since)).collect()
    }
    /// Remove records for songs that no longer exist.
    pub fn forget_missing(&mut self, lib: &Library) {
        let ids: HashSet<u64> = lib.songs().iter().map(|s| s.id).collect();
        self.songs.retain(|i, _| ids.contains(i));
        self.usage.retain(|(i, _)| ids.contains(i));
    }

    /// Songs that carry every one of `tags` and match the words in `query` (title, author or lyrics).
    pub fn filter<'a>(&self, lib: &'a Library, tags: &[&str], query: &str) -> Vec<&'a Song> {
        let want: Vec<String> = tags.iter().map(|t| tidy_tag(t)).filter(|t| !t.is_empty()).collect();
        lib.search(query).into_iter().filter(|s| {
            let have = self.songs.get(&s.id).map(|m| m.tags.as_slice()).unwrap_or(&[]);
            want.iter().all(|w| have.contains(w))
        }).collect()
    }
}

fn norm_words(s: &str) -> Vec<String> {
    s.chars().map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' }).collect::<String>().split_whitespace().map(|w| w.to_string()).collect()
}

fn shingles(song: &Song) -> HashSet<String> {
    let words: Vec<String> = song.sections.iter().flat_map(|s| norm_words(&s.text)).collect();
    if words.len() < 3 {
        return words.into_iter().collect();
    }
    words.windows(3).map(|w| w.join(" ")).collect()
}

/// 0 (nothing alike) to 1 (the same words).
pub fn similarity(a: &Song, b: &Song) -> f32 {
    let (x, y) = (shingles(a), shingles(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    x.intersection(&y).count() as f32 / x.union(&y).count() as f32
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dup {
    pub ids: (u64, u64),
    pub score: f32,
    pub same_title: bool,
}

/// Pairs of songs that look like the same song: the same title, or lyrics at least `threshold` alike (0.8 is a good start).
pub fn find_duplicates(lib: &Library, threshold: f32) -> Vec<Dup> {
    let songs = lib.songs();
    let titles: Vec<String> = songs.iter().map(|s| norm_words(&s.title).join(" ")).collect();
    let sh: Vec<HashSet<String>> = songs.iter().map(shingles).collect();
    let mut out = Vec::new();
    for i in 0..songs.len() {
        for j in i + 1..songs.len() {
            let same_title = !titles[i].is_empty() && titles[i] == titles[j];
            let score = if sh[i].is_empty() || sh[j].is_empty() { 0.0 } else { sh[i].intersection(&sh[j]).count() as f32 / sh[i].union(&sh[j]).count() as f32 };
            if same_title || score >= threshold {
                out.push(Dup { ids: (songs[i].id, songs[j].id), score, same_title });
            }
        }
    }
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then(a.ids.cmp(&b.ids)));
    out
}

fn key_of(s: &str) -> String {
    crate::goto::label_key(s)
}

/// Fold `drop` into `keep`: parts only the other copy has are added, blank details are filled in, `drop` is deleted,
/// and its tags and usage move to `keep`.
pub fn merge(lib: &mut Library, meta: &mut Meta, keep: u64, drop: u64) -> Result<usize, String> {
    if keep == drop {
        return Err("Pick two different songs.".into());
    }
    let other = lib.get(drop).cloned().ok_or("The song to merge in is not in the library.")?;
    let mut added = 0;
    {
        let k = lib.get_mut(keep).ok_or("The song to keep is not in the library.")?;
        for s in &other.sections {
            if !k.sections.iter().any(|x| key_of(&x.label) == key_of(&s.label)) {
                k.sections.push(Section { label: s.label.clone(), text: s.text.clone() });
                added += 1;
            }
        }
        if k.author.trim().is_empty() {
            k.author = other.author.clone();
        }
        if k.ccli_song.trim().is_empty() {
            k.ccli_song = other.ccli_song.clone();
        }
        if k.copyright.trim().is_empty() {
            k.copyright = other.copyright.clone();
        }
    }
    lib.remove(drop);
    if let Some(m) = meta.songs.remove(&drop) {
        let e = meta.songs.entry(keep).or_default();
        for t in m.tags {
            if !e.tags.contains(&t) {
                e.tags.push(t);
            }
        }
        e.tags.sort();
        if e.key.is_empty() {
            e.key = m.key;
        }
        if e.bpm.is_none() {
            e.bpm = m.bpm;
        }
        if e.category.is_empty() {
            e.category = m.category;
        }
    }
    for (i, _) in meta.usage.iter_mut().filter(|(i, _)| *i == drop) {
        *i = keep;
    }
    Ok(added)
}

/// Replace in one string. Returns the new string and how many places changed.
pub fn replace_text(s: &str, find: &str, with: &str, case_sensitive: bool, whole_word: bool) -> (String, usize) {
    if find.is_empty() {
        return (s.to_string(), 0);
    }
    let fold = |c: char| if case_sensitive { c } else { c.to_lowercase().next().unwrap_or(c) };
    let hay: Vec<char> = s.chars().collect();
    let key: Vec<char> = find.chars().map(fold).collect();
    let mut out = String::new();
    let mut i = 0;
    let mut n = 0;
    while i < hay.len() {
        let hit = i + key.len() <= hay.len() && hay[i..i + key.len()].iter().zip(&key).all(|(a, b)| fold(*a) == *b) && {
            !whole_word || (!(i > 0 && hay[i - 1].is_alphanumeric()) && !(i + key.len() < hay.len() && hay[i + key.len()].is_alphanumeric()))
        };
        if hit {
            out.push_str(with);
            i += key.len();
            n += 1;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    (out, n)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replaced {
    pub id: u64,
    pub title: String,
    pub count: usize,
}

/// Find and replace in titles, authors, copyright lines and every part's words. With `apply` false nothing changes:
/// you only get the list of songs that would (a preview).
pub fn find_replace(lib: &mut Library, find: &str, with: &str, case_sensitive: bool, whole_word: bool, apply: bool) -> Vec<Replaced> {
    let ids: Vec<u64> = lib.songs().iter().map(|s| s.id).collect();
    let mut out = Vec::new();
    for id in ids {
        let Some(song) = lib.get(id).cloned() else { continue };
        let mut s = song.clone();
        let mut count = 0;
        let mut go = |text: &mut String| {
            let (t, n) = replace_text(text, find, with, case_sensitive, whole_word);
            if n > 0 {
                *text = t;
                count += n;
            }
        };
        go(&mut s.title);
        go(&mut s.author);
        go(&mut s.copyright);
        for sec in &mut s.sections {
            go(&mut sec.text);
        }
        if count > 0 {
            out.push(Replaced { id, title: song.title.clone(), count });
            if apply {
                if let Some(t) = lib.get_mut(id) {
                    *t = s;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, text: &str) -> Song {
        let mut s = Song::from_text(title, text);
        s.author = String::new();
        s
    }

    fn lib3() -> (Library, u64, u64, u64) {
        let mut l = Library::new();
        let a = l.add(song("Amazing Grace", "Verse 1:\nAmazing grace how sweet the sound\nThat saved a wretch like me\n\nChorus:\nMy chains are gone"));
        let b = l.add(song("amazing grace!", "Verse 1:\nAmazing grace how sweet the sound\nThat saved a wretch like me\n\nVerse 2:\nTwas grace that taught"));
        let c = l.add(song("Other", "Verse 1:\nSomething completely different here today"));
        (l, a, b, c)
    }

    #[test]
    fn tags_are_tidied_and_indexed() {
        let mut m = Meta::default();
        assert!(m.tag(1, "  #Easter "));
        assert!(!m.tag(1, "easter"));
        assert!(!m.tag(1, "  "));
        m.tag(2, "easter");
        m.tag(2, "slow");
        let idx = m.tag_index();
        assert_eq!(idx["easter"], vec![1, 2]);
        assert_eq!(idx["slow"], vec![2]);
        assert!(m.untag(2, "SLOW"));
        assert!(!m.untag(2, "slow"));
        assert!(!m.untag(99, "x"));
        m.set_bpm(1, Some(5000));
        assert_eq!(m.songs[&1].bpm, None);
        m.set_bpm(1, Some(72));
        assert_eq!(m.songs[&1].bpm, Some(72));
    }

    #[test]
    fn usage_counts_recent_use_and_unused_songs() {
        let (l, a, b, c) = lib3();
        let mut m = Meta::default();
        for t in [100, 200, 300] {
            m.record_use(a, t);
        }
        m.record_use(b, 250);
        assert_eq!(m.stats(a), Stats { count: 3, last_used: Some(300) });
        assert_eq!(m.stats(c), Stats { count: 0, last_used: None });
        assert_eq!(m.most_used(0, 5), vec![(a, 3), (b, 1)]);
        assert_eq!(m.most_used(260, 5), vec![(a, 1)]);
        assert_eq!(m.unused_since(&l, 260), vec![b, c]);
    }

    #[test]
    fn filtering_by_tag_and_words() {
        let (l, a, b, _) = lib3();
        let mut m = Meta::default();
        m.tag(a, "easter");
        m.tag(a, "slow");
        m.tag(b, "easter");
        assert_eq!(m.filter(&l, &["easter"], "").len(), 2);
        assert_eq!(m.filter(&l, &["easter", "slow"], "").len(), 1);
        assert_eq!(m.filter(&l, &["easter"], "chains").len(), 1);
        assert_eq!(m.filter(&l, &["nope"], "").len(), 0);
        assert_eq!(m.filter(&l, &[], "").len(), 3);
    }

    #[test]
    fn duplicates_are_found_by_title_and_by_words() {
        let (l, a, b, c) = lib3();
        let d = find_duplicates(&l, 0.5);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].ids, (a, b));
        assert!(d[0].same_title);
        assert!(d[0].score > 0.3);
        assert!(!d.iter().any(|x| x.ids.0 == c || x.ids.1 == c));
        // different title, same words
        let mut l2 = Library::new();
        l2.add(song("One", "Verse 1:\nlove lifted me when nothing else could help"));
        l2.add(song("Two", "Verse 1:\nlove lifted me when nothing else could help"));
        assert_eq!(find_duplicates(&l2, 0.8).len(), 1);
        assert!(find_duplicates(&Library::new(), 0.8).is_empty());
        assert_eq!(similarity(&song("a", "x"), &song("b", "y z w v")), 0.0);
    }

    #[test]
    fn merging_keeps_everything_useful_and_moves_tags_and_usage() {
        let (mut l, a, b, _) = lib3();
        l.get_mut(b).unwrap().ccli_song = "22025".into();
        let mut m = Meta::default();
        m.tag(b, "easter");
        m.record_use(b, 5);
        m.record_use(a, 6);
        let added = merge(&mut l, &mut m, a, b).unwrap();
        assert_eq!(added, 1); // Verse 2
        assert!(l.get(b).is_none());
        let k = l.get(a).unwrap();
        assert!(k.sections.iter().any(|s| s.label == "Verse 2"));
        assert_eq!(k.ccli_song, "22025");
        assert_eq!(m.songs[&a].tags, vec!["easter"]);
        assert_eq!(m.stats(a).count, 2);
        assert!(merge(&mut l, &mut m, a, a).is_err());
        assert!(merge(&mut l, &mut m, a, 999).is_err());
    }

    #[test]
    fn forgetting_deleted_songs_cleans_the_records() {
        let (mut l, a, b, _) = lib3();
        let mut m = Meta::default();
        m.tag(b, "x");
        m.record_use(b, 1);
        m.record_use(a, 1);
        l.remove(b);
        m.forget_missing(&l);
        assert!(!m.songs.contains_key(&b));
        assert_eq!(m.usage.len(), 1);
    }

    #[test]
    fn replace_options() {
        assert_eq!(replace_text("Grace grace GRACE", "grace", "love", false, false), ("love love love".into(), 3));
        assert_eq!(replace_text("Grace grace GRACE", "grace", "love", true, false), ("Grace love GRACE".into(), 1));
        assert_eq!(replace_text("graceful grace", "grace", "love", false, true), ("graceful love".into(), 1));
        assert_eq!(replace_text("a.b", ".", "!", true, false).0, "a!b");
        assert_eq!(replace_text("abc", "", "x", true, false), ("abc".into(), 0));
        assert_eq!(replace_text("Señor SEÑOR", "señor", "Lord", false, false).1, 2);
        assert_eq!(replace_text("aaa", "aa", "b", true, false), ("ba".into(), 1)); // no overlapping matches
    }

    #[test]
    fn find_and_replace_previews_first_then_applies_everywhere() {
        let (mut l, a, _, _) = lib3();
        l.get_mut(a).unwrap().copyright = "Grace Church 2001".into();
        let preview = find_replace(&mut l, "grace", "mercy", false, true, false);
        assert_eq!(preview.len(), 2);
        assert!(preview.iter().any(|r| r.id == a && r.count >= 3));
        assert!(l.get(a).unwrap().sections[0].text.contains("grace"), "a preview changes nothing");
        let done = find_replace(&mut l, "grace", "mercy", false, true, true);
        assert_eq!(done, preview);
        let s = l.get(a).unwrap();
        assert_eq!(s.title, "Amazing mercy");
        assert!(s.copyright.starts_with("mercy Church"));
        assert!(find_replace(&mut l, "grace", "x", false, true, true).is_empty());
    }
}
