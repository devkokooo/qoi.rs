use std::path::PathBuf;

use clap::Parser;

use qoi_rs::QOI;
use image;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The .qoi file path to decode, relative to cwd
    #[arg(short, long, default_value = "qoi_test_images/dice.qoi")]
    file_name: PathBuf,
}

// TODO: decode and encode subcommands
// decode -i <input.qoi> -o <path/output.png>
// encode -i <input.png> -o <path/output.qoi>
// cat input.qoi | qoi-rs.exe decode > out.png
fn main() {
    let Args { file_name } = Args::parse();

    if let Ok(decoded) = QOI::decode(file_name) {
        let len = decoded.raw_pixels.len();
        println!("Total raw pixels: {len}");

        let buffer: Vec<u8> = decoded.raw_pixels
            .iter()
            .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
            .collect();

        image::save_buffer("image.png", buffer.as_slice(), decoded.header.width, decoded.header.height, image::ColorType::Rgba8).unwrap();
    }
}
