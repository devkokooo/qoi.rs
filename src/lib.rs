use std::{fmt, num::Wrapping};
use std::io;

pub mod decoder;
pub mod encoder;

pub struct QOI;

pub const MAGIC_BYTES: [u8; 4] = [0x71, 0x6f, 0x69, 0x66];

#[derive(Debug)]
pub struct QOIHeader {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub colorspace: u8
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
pub enum QoiOp {
    RGB = 0b1111_1110,
    RGBA = 0b1111_1111,
    INDEX = 0b00,
    DIFF = 0b01,
    LUMA = 0b10,
    RUN = 0b11,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Pixel {
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            red: 0, green: 0, blue: 0, alpha: 0
        }
    }

    #[must_use]
    pub const fn black() -> Self {
        Self {
            red: 0, green: 0, blue: 0, alpha: 255,
        }
    }

    #[must_use]
    pub const fn white() -> Self {
        Self {
            red: 255, green: 255, blue: 255, alpha: 255,
        }
    }
}

impl QOI {
    pub fn hash_index(pixel: &Pixel) -> u8 {
        let Pixel { red, green, blue, alpha } = pixel;

        let red = Wrapping(*red);
        let green = Wrapping(*green);
        let blue = Wrapping(*blue);
        let alpha = Wrapping(*alpha);

        let hash = (
            red * Wrapping(3) +
            green * Wrapping(5) +
            blue * Wrapping(7) +
            alpha * Wrapping(11)
        ).0;

        hash % 64
    }

    /// Parses encoded byte into the proper chunk type
    fn parse_tag(byte: u8) -> QoiOp {
        match byte {
            // RGB and RGBA chunks takes precedence first
            0b1111_1110 => QoiOp::RGB,
            0b1111_1111 => QoiOp::RGBA,
            _ => {
                let tag = byte >> 6;

                match tag {
                    0b00 => QoiOp::INDEX,
                    0b01 => QoiOp::DIFF,
                    0b10 => QoiOp::LUMA,
                    0b11 => QoiOp::RUN,
                    _ => unreachable!(),
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_fn_should_work_properly() {
        let cases = [
            (Pixel::zeroed(), 0),
            (Pixel::black(), 53),
            (Pixel::white(), 38),
            (Pixel { red: 1, green: 1, blue: 1, alpha: 1 }, 26),
        ];

        for (pixel, expected_index) in cases {
            let actual = QOI::hash_index(&pixel);

            assert_eq!(actual, expected_index);
        }
    }
}
