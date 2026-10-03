use std::{fmt, fs::{self, File}};
use std::io::{self, Write};

pub struct QOI;

#[derive(Debug)]
struct QOIHeader {
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8
}

#[derive(Debug)]
pub enum QOIError {
    Io(io::Error),
    IncorrectMagicBytes,
    MalformedFile,
    DimensionOverflow,
}

impl From<io::Error> for QOIError {
    fn from(error: io::Error) -> Self {
        QOIError::Io(error)
    }
}

impl fmt::Display for QOIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QOIError::Io(error) => write!(f, "I/O error: {error}"),
            QOIError::IncorrectMagicBytes => write!(f, "[QOI header] Incorrect magic bytes!"),
            QOIError::MalformedFile => write!(f, "[QOI header] your file is all messed up bruh."),
            QOIError::DimensionOverflow => write!(f, "[QOI header] image dimensions too damn big!"),
        }
    }
}

impl std::error::Error for QOIError {}

#[derive(Copy, Clone)]
struct Pixel {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl QOI {
    pub fn decode(path: &str) -> Result<Vec<u8>, QOIError> {
        let data = fs::read(path)?;

        let QOIHeader {
            width, height, channels, colorspace
        } = Self::extract_header(&data[..14])?;

        // write chunks to file to review
        let mut file = File::create("chunks.txt")?;

        writeln!(file, "[QOI Header] Extracted...")?;
        let formatted_header = format!(
            "width: {width}, height: {height}\nchannels: {channels}, colorspace: {colorspace}"
        );
        writeln!(file, "{formatted_header}")?;

        let mut prev_pixels: [Pixel; 64] = [Pixel {
            red: 0, green: 0, blue: 0, alpha: 255
        }; 64];

        // Start parsing each of the possible QOI chunks
        let chunks = &data[14..];
        let mut chunks = chunks.iter();

        while let Some(chunk) = chunks.next() {
            // RGB and RGBA chunks takes precedence first
            if channels == 3 && *chunk == 0b1111_1110 {
                let red = chunks.next().unwrap();
                let green = chunks.next().unwrap();
                let blue = chunks.next().unwrap();

                let pixel = Pixel {
                    red: *red,
                    green: *green,
                    blue: *blue,
                    alpha: 255
                };
                let index = QOI::hash_index(&pixel);
                prev_pixels[index as usize] = pixel;

                let formatted = format!("RGB [{}, {}, {}] ", *red, *green, *blue);
                writeln!(file, "{formatted}")?;
            }
            else if channels == 4 && *chunk == 0b1111_1111 {
                let red = chunks.next().unwrap();
                let green = chunks.next().unwrap();
                let blue = chunks.next().unwrap();
                let alpha = chunks.next().unwrap();

                let pixel = Pixel {
                    red: *red,
                    green: *green,
                    blue: *blue,
                    alpha: *alpha,
                };
                let index = QOI::hash_index(&pixel);
                prev_pixels[index as usize] = pixel;

                let formatted = format!("RGBA [{}, {}, {}, {}] ", *red, *green, *blue, *alpha);
                writeln!(file, "{formatted}")?;
            }
            else {
                // THEN start parsing chunks for RUN, INDEX, DIFF, LUMA
                let tag = *chunk >> 6;
                let data = *chunk & 0b0011_1111;

                let formatted = format!("({:02b} {:06b}) ", tag, data);
                write!(file, "{formatted}")?;

                if tag == 0b00 {
                    let formatted = format!("INDEX {data}");
                    writeln!(file, "{formatted}")?;
                }
                else if tag == 0b01 {
                    let bias = 2;
                    let diff_red = (data >> 4) & 0b11;
                    let diff_green = (data >> 2) & 0b11;
                    let diff_blue = data & 0b11;

                    // TODO: handle wraparound (i.e. 0 - 2 = 254), at the pixel level
                    // let dr = diff_red - bias;
                    // let dg = diff_green - bias;
                    // let db = diff_blue - bias;

                    let formatted = format!("DIFF [{diff_red}, {diff_green}, {diff_blue}]");
                    writeln!(file, "{formatted}")?;
                }
                // LUMA has 2 bytes of data
                else if tag == 0b10 {
                    let next_byte = chunks.next().unwrap();

                    let dg = data >> 2; // 6 bits
                    let dr_dg = next_byte >> 4; // 4 bits
                    let db_dg = next_byte & 0b1111; // 4 bits

                    let formatted_byte = format!("LUMA [{dg}, {dr_dg}, {db_dg}]");
                    writeln!(file, "{formatted_byte}")?;
                }
                else if tag == 0b11 {
                    let bias = -1;
                    let actual = data as i16 - bias;

                    let formatted = format!("RUN {actual}");
                    writeln!(file, "{formatted}")?;
                }
            }

        }

        todo!()
    }

    fn hash_index(pixel: &Pixel) -> u8 {
        let red = pixel.red as u16;
        let green = pixel.green as u16;
        let blue = pixel.blue as u16;
        let alpha = pixel.alpha as u16;

        let hash = (red * 3 + green * 5 + blue * 7 + alpha * 9) % 64;
        hash as u8
    }

    // Read the header byte-by-byte (every u8)
    // 14 bytes make up the header
    // width and height both take up 4 bytes
    fn extract_header(raw_data: &[u8]) -> Result<QOIHeader, QOIError> {
        if raw_data.len() < 14 {
            return Err(QOIError::MalformedFile);
        }

        // Verify QOI header
        // If magic bytes don't read "qoif", fail with error
        const MAGIC_BYTES: [u8; 4] = [0x71, 0x6f, 0x69, 0x66];

        if &raw_data[..4] != &MAGIC_BYTES {
            return Err(QOIError::IncorrectMagicBytes);
        }

        // Then, parse the rest of the header for width, height, channels, colorspace
        // Width & height are calculated as big endian
        // Need to make sure byte conversion from little endian (most consumer processors) is proper
        // use to_be_bytes to convert
        let width = &raw_data[4..=7];
        let height = &raw_data[8..=11];
        let channels = *&raw_data[12];
        let colorspace = *&raw_data[13];

        // file already stored as BE
        let width = u32::from_be_bytes(width.try_into().unwrap());
        let height = u32::from_be_bytes(height.try_into().unwrap());

        if width > 100_000_000 || height > 100_000_000 {
            return Err(QOIError::DimensionOverflow);
        }

        Ok(QOIHeader {
            width, height, channels, colorspace
        })
    }

    pub fn encode() {

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_extract_empty() {
        let sample_data = [];

        assert!(
            QOI::extract_header(&sample_data).is_err(),
            "Should fail if header is empty!"
        );
    }

    #[test]
    fn header_extract_malformed_header() {
        let sample_data = [0x71, 0x6f, 0x69, 0x66];

        assert!(
            QOI::extract_header(&sample_data).is_err(),
            "Should fail if header is malformed!"
        );
    }

    #[test]
    fn header_extract_correct_magic_bytes() {
        let sample_data = [
            0x71, 0x6f, 0x69, 0x66, // qoif
            0x01, 0x01, 0x01, 0x01, // width
            0x01, 0x01, 0x01, 0x01, // height
            0x01, 0x01 // channels, colorspace
        ];

        assert!(
            QOI::extract_header(&sample_data).is_ok(),
            "Should fail if magic bytes don't match"
        );
    }

    #[test]
    fn header_extract_giant_dimensions() {
        let big_width = [
            0x71, 0x6f, 0x69, 0x66, // qoif
            0x01, 0xE1, 0xF5, 0x05, // width, 100_000_001
            0x01, 0x01, 0x01, 0x01, // height
            0x01, 0x01 // channels, colorspace
        ];

        let big_height = [
            0x71, 0x6f, 0x69, 0x66, // qoif
            0x01, 0x01, 0x01, 0x01, // width
            0x01, 0xE1, 0xF5, 0x05, // height, 100_000_001
            0x01, 0x01 // channels, colorspace
        ];

        assert!(
            QOI::extract_header(&big_width).is_err(),
            "Should fail if width is over 100M pixels"
        );

        assert!(
            QOI::extract_header(&big_height).is_err(),
            "Should fail if height is over 100M pixels"
        );
    }
}
