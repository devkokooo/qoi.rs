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

#[derive(Debug, PartialEq)]
pub enum QOI_OP {
    RGB,
    RGBA,
    INDEX,
    DIFF,
    LUMA,
    RUN,
}

struct DecodedChunk {
    chunk_type: QOI_OP,
    pixel: Pixel,
}

#[derive(Copy, Clone)]
pub struct Pixel {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl QOI {
    /// Step-by-step decoding process:
    /// 1. Read data from .qoi file
    /// 2. Extract and validate header
    /// 3. Parse data chunks
    /// 4. Convert data chunks into pixel values
    /// 5. Populate raw pixel vector
    /// 6. Return raw pixels 
    pub fn decode(path: &str) -> Result<Vec<Pixel>, QOIError> {
        let data = fs::read(path)?;

        let QOIHeader {
            width, height, channels, colorspace
        } = Self::extract_header(&data[..14])?;

        // write chunks to file to review
        let mut file = File::create("chunks.txt")?;

        let total_pixels = (width * height) as u64;

        writeln!(file, "[QOI Header] Extracted...")?;
        writeln!(
            file,
            "width: {width}, height: {height}\nchannels: {channels}, colorspace: {colorspace}\ntotal pixels: {total_pixels}"
        )?;

        let mut seen_pixels: [Pixel; 64] = [Pixel {
            red: 0, green: 0, blue: 0, alpha: 255
        }; 64];

        let mut prev_pixel = Pixel {
            red: 0, green: 0, blue: 0, alpha: 255,
        };

        // Start parsing each of the possible QOI chunks
        let chunks = &data[14..];
        let mut chunks = chunks.iter();

        let mut raw_pixels: Vec<Pixel> = Vec::new();

        while let Some(chunk) = chunks.next() {
            let tag = QOI::parse_tag(*chunk);

            if tag == QOI_OP::RGB {
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
                seen_pixels[index as usize] = pixel;
                prev_pixel = pixel;
                
                writeln!(
                    file,
                    "RGB [{}, {}, {}]", *red, *green, *blue
                )?;

                raw_pixels.push(pixel);
            }
            else if tag == QOI_OP::RGBA {
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
                seen_pixels[index as usize] = pixel;
                prev_pixel = pixel;
                
                writeln!(
                    file,
                    "RGBA [{}, {}, {}, {}]", *red, *green, *blue, *alpha
                )?;

                raw_pixels.push(pixel);
            }
            else {
                // THEN start parsing chunks for RUN, INDEX, DIFF, LUMA
                let data = *chunk & 0b0011_1111;

                write!(file, "({:06b}) ", data)?;

                // When we find it, use the index to get pixel data from prev pixel array
                if tag == QOI_OP::INDEX {
                    let index = data;
                    let prev = seen_pixels[index as usize];
                    prev_pixel = prev;
                    let Pixel { red, green, blue, alpha } = prev;

                    writeln!(file, "INDEX {index} ➜ Pixel [{red}, {green}, {blue}, {alpha}]")?;

                    raw_pixels.push(prev)
                }
                else if tag == QOI_OP::DIFF {
                    let bias: i16 = 2;
                    let diff_red = ((data >> 4) & 0b11) as i16;
                    let diff_green = ((data >> 2) & 0b11) as i16;
                    let diff_blue = (data & 0b11) as i16;

                    let dr = diff_red - bias;
                    let dg = diff_green - bias;
                    let db = diff_blue - bias;

                    let Pixel { red, green, blue, alpha } = prev_pixel;
                    let red = red as i16;
                    let green = green as i16;
                    let blue = blue as i16;

                    let wrap_dr = QOI::wraparound_u8(red, dr);
                    let wrap_dg = QOI::wraparound_u8(green, dg);
                    let wrap_db = QOI::wraparound_u8(blue, db);

                    let diff_pixel = Pixel {
                        red: wrap_dr as u8,
                        green: wrap_dg as u8,
                        blue: wrap_db as u8,
                        alpha
                    };
                    prev_pixel = diff_pixel;
                    
                    writeln!(
                        file,
                        "DIFF [{dr}, {dg}, {db}] ➜ Pixel [{red} ({wrap_dr}), {green} ({wrap_dg}), {blue} ({wrap_db}), {alpha}]"
                    )?;

                    raw_pixels.push(diff_pixel);
                }
                else if tag == QOI_OP::LUMA {
                    let next_byte = chunks.next().unwrap();

                    let bias_green: i16 = 32;
                    let bias_rb: i16 = 8;

                    let dg = data >> 2; // 6 bits
                    let dr_dg = next_byte >> 4; // 4 bits
                    let db_dg = next_byte & 0b1111; // 4 bits

                    let dg = dg as i16 - bias_green;
                    let dr_dg = dr_dg as i16 - bias_rb;
                    let db_dg = db_dg as i16 - bias_rb;

                    let Pixel { red, green, blue, alpha } = prev_pixel;
                    let red = red as i16;
                    let green = green as i16;
                    let blue = blue as i16;

                    let wrap_dg = QOI::wraparound_u8(green, dg);
                    let wrap_dr = QOI::wraparound_u8(red, dr_dg + dg);
                    let wrap_db = QOI::wraparound_u8(blue, db_dg + dg);

                    let luma_pixel = Pixel {
                        red: wrap_dr as u8,
                        green: wrap_dg as u8,
                        blue: wrap_db as u8,
                        alpha
                    };
                    prev_pixel = luma_pixel;

                    writeln!(
                        file,
                        "LUMA [{dg}, {dr_dg}, {db_dg}] ➜ Pixel [{red} ({wrap_dr}), {green} ({wrap_dg}), {blue} ({wrap_db}), {alpha}]"
                    )?;

                    raw_pixels.push(luma_pixel);
                }
                // RUN chunk for run-length encoding
                // repeat the previously seen pixel X amount of times
                else if tag == QOI_OP::RUN {
                    let bias = -1;
                    let actual = data as i16 - bias;

                    let Pixel { red, green, blue, alpha } = prev_pixel;
                    writeln!(file, "RUN {actual} ➜ prev Pixel [{red}, {green}, {blue}, {alpha}]")?;

                    for _ in 0..actual {
                        raw_pixels.push(prev_pixel);
                    }
                }
            }

        }

        Ok(raw_pixels)
    }

    /// Parses encoded byte into the proper chunk type
    fn parse_tag(byte: u8) -> QOI_OP {
        match byte {
            // RGB and RGBA chunks takes precedence first
            0b1111_1110 => QOI_OP::RGB,
            0b1111_1111 => QOI_OP::RGBA,
            _ => {
                let tag = byte >> 6;

                match tag {
                    0b00 => QOI_OP::INDEX,
                    0b01 => QOI_OP::DIFF,
                    0b10 => QOI_OP::LUMA,
                    0b11 => QOI_OP::RUN,
                    _ => unreachable!(),
                }
            },
        }
    }

    fn hash_index(pixel: &Pixel) -> u8 {
        let red = pixel.red as u16;
        let green = pixel.green as u16;
        let blue = pixel.blue as u16;
        let alpha = pixel.alpha as u16;

        let hash = (red * 3 + green * 5 + blue * 7 + alpha * 9) % 64;
        hash as u8
    }

    // 255 + 1 = 0
    // 0 - 1 = 255
    // 0 - 2 = 254
    fn wraparound_u8(val: i16, diff: i16) -> i16 {
        let res = val + diff;

        if res < 0 { res + 256 }
        else if res > 255 { res - 256 }
        else { res }
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

    #[test]
    fn chunk_tags_should_be_parsed_correctly() {
        let cases = [
            (0b1111_1110 as u8, QOI_OP::RGB),
            (0b1111_1111, QOI_OP::RGBA),
            (0b0000_0001, QOI_OP::INDEX),
            (0b0100_0001, QOI_OP::DIFF),
            (0b1000_0001, QOI_OP::LUMA),
            (0b1100_0001, QOI_OP::RUN),
        ];

        for (byte, expected) in cases {
            let actual = QOI::parse_tag(byte);

            assert_eq!(
                actual, expected,
                "case failed for {byte:?}"
            );
        }
    }
}
