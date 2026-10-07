use std::path::PathBuf;

use clap::{Parser, Subcommand};

use qoi_rs::{QOI, QOIHeader};
use image::{self, ImageReader};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Converts .qoi to .png
    Decode { file_name: Option<String> },
    /// Converts .png to .qoi
    Encode {
        file_name: Option<String>
    }
}

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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Decode { file_name } => {
            let file_name = file_name.clone().unwrap_or("qoi_test_images/dice.qoi".to_string());

            if let Ok(decoded) = QOI::decode(file_name) {
                let len = decoded.raw_pixels.len();
                println!("Total raw pixels: {len}");
        
                let buffer: Vec<u8> = decoded.raw_pixels
                    .iter()
                    .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
                    .collect();
        
                image::save_buffer("image.png", buffer.as_slice(), decoded.header.width, decoded.header.height, image::ColorType::Rgba8).unwrap();
            }
            Ok(())
        }
        Commands::Encode { file_name } => {
            let file_name = file_name.clone().unwrap_or("qoi_test_images/dice.png".to_string());

            let img = ImageReader::open(file_name)?.decode()?;
            let width =  img.width();
            let height = img.height();
            let channels = img.color().channel_count();
            let header = QOIHeader { width, height, channels, colorspace: 0 };

            let buf = img.to_rgba8();
            let pixels = buf.pixels();

            let encoded = QOI::encode(pixels.map(|p| p.0), header)?;
            println!("width: {width}, height: {height}, pixel byte len: {}", buf.len());
            println!("Length of encoded bytes {}", encoded.bytes.len());

            std::fs::write("image.qoi", &encoded.bytes)?;
            Ok(())
        }
    }
}
