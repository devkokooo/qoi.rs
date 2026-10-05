use qoi_rs::QOI;
use image;

fn main() {
    let path = "qoi_test_images/dice.qoi";

    if let Ok(decoded) = QOI::decode(path) {
        let len = decoded.raw_pixels.len();
        println!("Total raw pixels: {len}");

        let buffer: Vec<u8> = decoded.raw_pixels
            .iter()
            .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
            .collect();

        image::save_buffer("image.png", buffer.as_slice(), decoded.header.width, decoded.header.height, image::ColorType::Rgba8).unwrap();
    }
}
