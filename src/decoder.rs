use std::{fs, path::Path};
use crate::{MAGIC_BYTES, QOI, QOIError, QOIHeader, QoiOp};
use crate::{Pixel};

pub struct DecodedImage {
    pub header: QOIHeader,
    pub raw_pixels: Vec<Pixel>,
}

impl QOI {
    /// Step-by-step decoding process:
    /// 1. Read data from .qoi file
    /// 2. Extract and validate header
    /// 3. Parse data chunks
    /// 4. Convert data chunks into pixel values
    /// 5. Populate raw pixel vector
    /// 6. Return raw pixels 
    pub fn decode(path: impl AsRef<Path>) -> Result<DecodedImage, QOIError> {
        let data = fs::read(path)?;

        let header @ QOIHeader {
            width, height, ..
        } = Self::extract_header(&data[..14])?;

        let total_pixels = (width * height) as u64;

        let mut seen_pixels: [Pixel; 64] = [Pixel::zeroed(); 64];

        let mut prev_pixel = Pixel {
            red: 0, green: 0, blue: 0, alpha: 255,
        };

        // Start parsing each of the possible QOI chunks
        let chunks = &data[14..];
        let mut chunks = chunks.iter();

        let mut raw_pixels: Vec<Pixel> = Vec::new();

        while let Some(byte) = chunks.next() {
            // last 8 bytes of byte stream are just end markers
            if total_pixels == raw_pixels.len() as u64 {
                break;
            }

            let tag = QOI::parse_tag(*byte);

            let curr_pixel = match tag {
                QoiOp::RGB => {
                    let pixel = Pixel {
                        red: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        green: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        blue: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        alpha: prev_pixel.alpha,
                    };
                    pixel
                }
                QoiOp::RGBA => {
                    let pixel = Pixel {
                        red: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        green: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        blue: *chunks.next().ok_or(QOIError::MalformedFile)?,
                        alpha: *chunks.next().ok_or(QOIError::MalformedFile)?,
                    };
                    pixel
                }
                QoiOp::INDEX => {
                    let index = *byte & 0b0011_1111;
                    let seen = seen_pixels[index as usize];
                    seen
                }
                QoiOp::DIFF => {
                    QOI::extract_pixel_from_diff_chunk(*byte, prev_pixel)
                }
                QoiOp::LUMA => {
                    let next_byte = chunks.next().unwrap();

                    QOI::extract_pixel_from_luma_chunk(*byte, *next_byte, prev_pixel)
                }
                QoiOp::RUN => {
                    let data = *byte & 0b0011_1111;
                    let actual = data.wrapping_add(1);

                    for _ in 0..actual {
                        raw_pixels.push(prev_pixel);
                    }
                    prev_pixel
                }
            };
            let index = QOI::hash_index(&curr_pixel);
            seen_pixels[index as usize] = curr_pixel;
            prev_pixel = curr_pixel;

            if tag != QoiOp::RUN {
                raw_pixels.push(curr_pixel);
            }
        }

        Ok(DecodedImage { header, raw_pixels })
    }

    fn extract_pixel_from_diff_chunk(curr_byte: u8, prev_pixel: Pixel) -> Pixel {
        let bias = 2;
        let diff_red = (curr_byte >> 4) & 0b11;
        let diff_green = (curr_byte >> 2) & 0b11;
        let diff_blue = curr_byte & 0b11;

        let dr = diff_red.wrapping_sub(bias);
        let dg = diff_green.wrapping_sub(bias);
        let db = diff_blue.wrapping_sub(bias);

        let Pixel { red, green, blue, alpha } = prev_pixel;
        let curr_red = dr.wrapping_add(red);
        let curr_green = dg.wrapping_add(green);
        let curr_blue = db.wrapping_add(blue);

        let diff_pixel = Pixel {
            red: curr_red,
            green: curr_green,
            blue: curr_blue,
            alpha
        };
        diff_pixel
    }

    fn extract_pixel_from_luma_chunk(first_byte: u8, second_byte: u8, prev_pixel: Pixel) -> Pixel {
        let bias_green = 32;
        let bias_rb = 8;

        let dg = first_byte & 0b0011_1111; // 6 bits
        let dr_dg = second_byte >> 4; // 4 bits
        let db_dg = second_byte & 0b1111; // 4 bits

        let dg = dg.wrapping_sub(bias_green);
        let dr_dg = dr_dg.wrapping_sub(bias_rb);
        let db_dg = db_dg.wrapping_sub(bias_rb);

        let Pixel { red, green, blue, alpha } = prev_pixel;

        let curr_green = dg.wrapping_add(green);
        let curr_red = dr_dg.wrapping_add(dg).wrapping_add(red);
        let curr_blue = db_dg.wrapping_add(dg).wrapping_add(blue);

        let luma_pixel = Pixel {
            red: curr_red,
            green: curr_green,
            blue: curr_blue,
            alpha
        };
        luma_pixel
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
            (0b1111_1110 as u8, QoiOp::RGB),
            (0b1111_1111, QoiOp::RGBA),
            (0b0000_0001, QoiOp::INDEX),
            (0b0100_0001, QoiOp::DIFF),
            (0b1000_0001, QoiOp::LUMA),
            (0b1100_0001, QoiOp::RUN),
        ];

        for (byte, expected) in cases {
            let actual = QOI::parse_tag(byte);

            assert_eq!(
                actual, expected,
                "case failed for {byte:?}"
            );
        }
    }

    #[test]
    fn diff_chunk_extract_pixel() {
        let cases = [
            (
                0b0110_1010_u8, // DIFF [2-2 = 0, 2-2 = 0, 2-2 = 0]
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
            ),
            (
                0b0111_1111_u8, // DIFF [3-2 = 1, 3-2 = 1, 3-2 = 1]
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 1, green: 1, blue: 1, alpha: 255 },
            ),
            (
                0b0101_0101_u8, // DIFF [1-2 = -1, 1-2 = -1, 1-2 = -1]
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 255, green: 255, blue: 255, alpha: 255 },
            ),
            (
                0b0100_0000_u8, // DIFF [0-2 = -2, 0-2 = -2, 0-2 = -2]
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 254, green: 254, blue: 254, alpha: 255 },
            ),
            (
                0b0101_1011_u8, // DIFF [1-2 = -1, 2-2 = 0, 3-2 = 1]
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 255, green: 0, blue: 1, alpha: 255 },
            ),
        ];

        for (byte, prev, expected) in cases {
            let res = QOI::extract_pixel_from_diff_chunk(byte, prev);

            assert_eq!(res, expected);
        }
    }

    #[test]
    fn luma_chunk_extract_pixel() {
        // curr_red = dr_dg + dg + prev_red
        // curr_blue = db_dg + dg + prev_blue
        // curr_green = dg + prev_green
        let cases = [
            (
                0b1010_0000_u8, // LUMA [32-32 = 0, 8-8 = 0, 8-8 = 0]
                0b1000_1000_u8,
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
            ),
            (
                0b1000_0000_u8, // LUMA [0-32 = -32, 8-8 = 0, 8-8 = 0]
                0b1000_1000_u8,
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 224, green: 224, blue: 224, alpha: 255 },
            ),
            (
                0b1011_1111_u8, // LUMA [63-32 = 31, 8-8 = 0, 8-8 = 0]
                0b1000_1000_u8,
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 31, green: 31, blue: 31, alpha: 255 },
            ),
            (
                0b1001_1111_u8, // LUMA [31-32 = -1, 8-8 = 0, 9-8 = 1]
                0b1000_1001_u8,
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 255, green: 255, blue: 0, alpha: 255 },
            ),
            (
                0b1001_1111_u8, // LUMA [31-32 = -1, 7-8 = -1, 9-8 = 1]
                0b0111_1001_u8,
                Pixel { red: 0, green: 0, blue: 0, alpha: 255 },
                Pixel { red: 254, green: 255, blue: 0, alpha: 255 },
            ),
        ];

        for (byte1, byte2, prev, expected) in cases {
            let res = QOI::extract_pixel_from_luma_chunk(byte1, byte2, prev);

            assert_eq!(res, expected);
        }
    }
}
