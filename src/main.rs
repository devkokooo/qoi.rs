use qoi_rs::QOI;

fn main() {
    if let Ok(pixels) = QOI::decode("qoi_test_images/dice.qoi") {
        let len = pixels.len();
        println!("Total size of raw pixels: {len}");
    }
}
