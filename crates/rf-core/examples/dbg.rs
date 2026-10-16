use rf_bible::Bible;
use rf_core::parser::{parse_full, ParseContext};
fn main() {
    let bib = Bible::embedded();
    for said in [
        "the lord is",
        "behold i come",
        "behold i come quickly",
        "behold i come banana and my",
        "for god so",
        "and he told them the story about the fisherman and his net again",
        "the wisdom of the elders is worth remembering every single day",
    ] {
        let out = parse_full(bib, said, &ParseContext::default(), 0.75, &[], None);
        let r: Vec<String> = bib
            .match_quotations_ranked(said, 8)
            .iter()
            .map(|h| format!("{} {}:{} {}%", bib.book_name(h.book), h.chapter, h.verse, h.pct))
            .collect();
        println!(
            "{said:?}\n  dets={:?}\n  confs={:?}\n  ranked=[{}]",
            out.detections.iter().map(|d| d.passage.label()).collect::<Vec<_>>(),
            out.confirms
                .iter()
                .map(|d| format!("{:?} {} pct={} alts={}", d.kind, d.passage.label(), d.pct, d.alternatives.len()))
                .collect::<Vec<_>>(),
            r.join(", ")
        );
    }
}
