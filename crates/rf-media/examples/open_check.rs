fn main() {
    for p in std::env::args().skip(1) {
        match rf_media::VideoReader::open(std::path::Path::new(&p)) {
            Ok(v) => println!("OK   {p}: {:?}", v.info()),
            Err(e) => println!("FAIL {p}: {e}"),
        }
    }
}
