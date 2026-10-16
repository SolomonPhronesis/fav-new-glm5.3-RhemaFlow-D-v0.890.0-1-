//! Spec S3.1, S3.2, S3.10 for pictures.
use image::{ImageOutputFormat, Rgb, RgbImage};
use rf_media::{decode_image, load_image, MAX_IMAGE_SIDE};
use std::io::Cursor;

fn encoded(w: u32, h: u32, fmt: ImageOutputFormat) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, _| if x < w / 2 { Rgb([200, 30, 30]) } else { Rgb([30, 30, 200]) });
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, fmt).unwrap();
    out.into_inner()
}

fn close(a: u8, b: u8, tol: u8) -> bool {
    (a as i32 - b as i32).unsigned_abs() as u8 <= tol
}

#[test]
fn s3_1_png_jpeg_gif_and_bmp_all_open_with_the_right_size_and_colours() {
    for (name, fmt, tol) in [
        ("png", ImageOutputFormat::Png, 2),
        ("jpeg", ImageOutputFormat::Jpeg(95), 25),
        ("gif", ImageOutputFormat::Gif, 40),
        ("bmp", ImageOutputFormat::Bmp, 2),
    ] {
        let img = decode_image(&encoded(64, 32, fmt), MAX_IMAGE_SIDE).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((img.width, img.height), (64, 32), "{name}");
        assert_eq!(img.pixels.len(), 64 * 32 * 4, "{name}");
        let left = &img.pixels[(5 * 64 + 5) * 4..][..4];
        let right = &img.pixels[(5 * 64 + 58) * 4..][..4];
        assert!(close(left[0], 200, tol) && close(left[2], 30, tol) && left[3] == 255, "{name} left {left:?}");
        assert!(close(right[0], 30, tol) && close(right[2], 200, tol), "{name} right {right:?}");
    }
}

#[test]
fn s3_2_a_huge_picture_is_scaled_down_once_and_keeps_its_shape() {
    let img = decode_image(&encoded(400, 200, ImageOutputFormat::Png), 100).unwrap();
    assert_eq!((img.width, img.height), (100, 50));
    // a small picture is left alone
    let img = decode_image(&encoded(40, 20, ImageOutputFormat::Png), 100).unwrap();
    assert_eq!((img.width, img.height), (40, 20));
    assert_eq!(MAX_IMAGE_SIDE, 4096);
}

#[test]
fn s3_10_garbage_and_cut_off_pictures_give_a_plain_error_not_a_crash() {
    let e = decode_image(b"this is not a picture at all", 4096).unwrap_err();
    assert!(e.contains("could not be opened"), "{e}");
    let full = encoded(64, 64, ImageOutputFormat::Png);
    assert!(decode_image(&full[..full.len() / 3], 4096).is_err());
    assert!(decode_image(&[], 4096).is_err());
}

#[test]
fn s3_10_a_missing_file_is_a_plain_error() {
    let e = load_image(std::path::Path::new("/definitely/not/here.png"), 4096).unwrap_err();
    assert!(e.contains("Cannot find"), "{e}");
}

#[test]
fn s3_1_load_image_reads_a_real_file() {
    let dir = std::env::temp_dir().join(format!("rf-media-img-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("x.png");
    std::fs::write(&p, encoded(20, 10, ImageOutputFormat::Png)).unwrap();
    let img = load_image(&p, 4096).unwrap();
    assert_eq!((img.width, img.height), (20, 10));
}
