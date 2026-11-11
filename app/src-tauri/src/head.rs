//! Player head: fetch the skin png, check it, crop the face and return a small png data url.
//! The png is untrusted, so size and dimensions are limited before and after decoding.

use base64::Engine;
use image::{imageops, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

use crate::net::Net;

pub async fn fetch_head(net: &Net, skin_url: &str) -> Result<String, String> {
    let bytes = net.get_bytes(skin_url, 64 * 1024).await?;
    head_from_png(&bytes)
}

pub fn head_from_png(bytes: &[u8]) -> Result<String, String> {
    let bad = || "Ogiltig skin.".to_string();
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(128);
    limits.max_image_height = Some(128);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|_| bad())?.to_rgba8();
    if !(img.width() == 64 && (img.height() == 64 || img.height() == 32)) {
        return Err(bad());
    }
    let mut face = imageops::crop_imm(&img, 8, 8, 8, 8).to_image();
    let hat = imageops::crop_imm(&img, 40, 8, 8, 8).to_image();
    imageops::overlay(&mut face, &hat, 0, 0);
    let big = imageops::resize(&face, 64, 64, imageops::FilterType::Nearest);
    let mut out = Vec::new();
    big.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).map_err(|_| bad())?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([10, 200, 30, 255]));
        let mut v = Vec::new();
        img.write_to(&mut Cursor::new(&mut v), ImageFormat::Png).unwrap();
        v
    }
    #[test]
    fn good_skin_gives_png_data_url() {
        let s = head_from_png(&png(64, 64)).unwrap();
        assert!(s.starts_with("data:image/png;base64,"));
        assert!(head_from_png(&png(64, 32)).is_ok());
    }
    #[test]
    fn rejects_wrong_sizes_and_garbage() {
        assert!(head_from_png(&png(32, 32)).is_err());
        assert!(head_from_png(&png(4000, 4000)).is_err());
        assert!(head_from_png(b"not a png").is_err());
    }
}
