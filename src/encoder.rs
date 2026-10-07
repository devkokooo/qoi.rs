use crate::{MAGIC_BYTES, Pixel, QOI, QOIError, QOIHeader, QoiOp};

pub struct EncodedImage {
    pub header: QOIHeader,
    pub bytes: Vec<u8>,
}

struct Diff {
    red: u8,
    green: u8,
    blue: u8,
}

impl Diff {
    const TAG: QoiOp = QoiOp::DIFF;
    const BIAS: u8 = 2;
    const MASK: u8 = 0b1111_1100;
    const CHUNK_SIZE: usize = 1;

    fn new(prev: Pixel, curr: Pixel) -> Self {
        Self {
            red: curr.red.wrapping_sub(prev.red),
            green: curr.green.wrapping_sub(prev.green),
            blue: curr.blue.wrapping_sub(prev.blue),
        }
    }

    /// If I can't fit a color in 2 bits, it is not representable
    fn is_representable(&self) -> bool {
        (self.biased_red() & Self::MASK) == 0
            && (self.biased_green() & Self::MASK) == 0
            && (self.biased_blue() & Self::MASK) == 0
    }

    fn biased_red(&self) -> u8 {
        self.red.wrapping_add(Self::BIAS)
    }

    fn biased_green(&self) -> u8 {
        self.green.wrapping_add(Self::BIAS)
    }

    fn biased_blue(&self) -> u8 {
        self.blue.wrapping_add(Self::BIAS)
    }

    fn to_bits(&self) -> Option<[u8; Self::CHUNK_SIZE]> {
        self.is_representable().then_some([(Self::TAG as u8) << 6
            | self.biased_red() << 4
            | self.biased_green() << 2
            | self.biased_blue()])
    }
}

struct Luma {
    diff_green: u8,
    dr_dg: u8,
    db_dg: u8,
}

impl Luma {
    const TAG: QoiOp = QoiOp::LUMA;
    const BIAS_GREEN: u8 = 32;
    const MASK_GREEN: u8 = 0b1100_0000;
    const BIAS_RB: u8 = 8;
    const MASK_RB: u8 = 0b1111_0000;
    const CHUNK_SIZE: usize = 2;

    fn new(prev: Pixel, curr: Pixel) -> Self {
        let diff_green = curr.green.wrapping_sub(prev.green);

        Self {
            diff_green,
            dr_dg: curr.red.wrapping_sub(prev.red).wrapping_sub(diff_green),
            db_dg: curr.blue.wrapping_sub(prev.blue).wrapping_sub(diff_green),
        }
    }

    fn is_representable(&self) -> bool {
        (self.biased_green() & Self::MASK_GREEN) == 0
            && (self.biased_red() & Self::MASK_RB) == 0
            && (self.biased_blue() & Self::MASK_RB) == 0
    }

    fn biased_green(&self) -> u8 {
        self.diff_green.wrapping_add(Self::BIAS_GREEN)
    }

    fn biased_red(&self) -> u8 {
        self.dr_dg.wrapping_add(Self::BIAS_RB)
    }

    fn biased_blue(&self) -> u8 {
        self.db_dg.wrapping_add(Self::BIAS_RB)
    }

    fn to_bits(&self) -> Option<[u8; Self::CHUNK_SIZE]> {
        self.is_representable().then_some([
            (Self::TAG as u8) << 6 | self.biased_green(),
            self.biased_red() << 4 | self.biased_blue(),
        ])
    }
}

impl QOI {
    /// Step-by-step encoding process:
    /// 1. Convert .png or other file type to raw bytes
    /// 2. Encode the QOI header
    /// 3. Iterate through image pixels as bytes
    /// 4. Encode pixels as QOI chunks
    /// 5. Finish encoding with byte stream end of 7 0x00 and 1 0x01
    ///
    /// # Errors
    /// TODO: sometimes it errors
    pub fn encode(
        bytes: impl IntoIterator<Item = [u8; 4]>,
        header: QOIHeader,
    ) -> Result<EncodedImage, QOIError> {
        let mut encoded: Vec<u8> = Vec::new();

        // Encode magic bytes "qoif" (4 bytes)
        encoded.extend(MAGIC_BYTES);

        // Encode header data
        let QOIHeader {
            width,
            height,
            channels,
            colorspace,
        } = header;
        encoded.extend(width.to_be_bytes());
        encoded.extend(height.to_be_bytes());
        encoded.push(channels);
        encoded.push(colorspace);

        let mut seen_pixels: [Pixel; 64] = [Pixel::zeroed(); 64];

        let mut prev_pixel = Pixel {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        };

        // Iterate image pixels and encode them to QOI chunks

        let mut run: Option<u8> = None;

        for [red, green, blue, alpha] in bytes {
            let curr_pixel = Pixel {
                red,
                green,
                blue,
                alpha,
            };

            // 1ST CASE: unique pixel color, first time seeing
            // create RGB/RGBA chunk, then save in seen_pixels arr

            // 2ND CASE: color seen before & NOT same as prev pixel
            // only 1 occurrence, create INDEX chunk

            // 3RD CASE: color seen before & same as prev pixel
            // 2+ occurrence, create RUN chunk

            // Precedence: RUN, INDEX, DIFF, LUMA, RGB, RGBA

            // multiple RUN chunks
            if let Some(rl) = run
                && rl >= 62
            {
                let chunk = Self::create_run_chunk(rl);
                encoded.push(chunk);
                run = None;
            }

            if curr_pixel == prev_pixel {
                run = Some(run.unwrap_or(0) + 1);
                prev_pixel = curr_pixel;
                continue;
            }

            // write RUN chunk only after we found the end of repetition
            // 0     1    0     0    0    0    2    0     0   1
            // RGBA  RGB  INDEX r=1  r=2  r=3  RGBA INDEX r=1 INDEX
            if let Some(rl) = run {
                let chunk = Self::create_run_chunk(rl);
                encoded.push(chunk);
                run = None;
            }

            let index = Self::hash_index(&curr_pixel);

            // INDEX chunk, cannot be 2+ INDEX chunks
            if curr_pixel == seen_pixels[index as usize] {
                let chunk = Self::create_index_chunk(index);
                encoded.push(chunk);
                prev_pixel = curr_pixel;
                continue;
            }

            seen_pixels[index as usize] = curr_pixel;

            // RGBA if alpha is different
            if prev_pixel.alpha != curr_pixel.alpha {
                let tag = QoiOp::RGBA as u8;
                let Pixel {
                    red,
                    green,
                    blue,
                    alpha,
                } = curr_pixel;

                let chunk = [tag, red, green, blue, alpha];
                encoded.extend(chunk);
                prev_pixel = curr_pixel;
                continue;
            }

            // prioritize DIFF (1-byte), then LUMA (2-bytes)
            // then RGB/RGBA (4-5 bytes), by comparing diff ranges
            if let Some(diff) = Diff::new(prev_pixel, curr_pixel).to_bits() {
                encoded.extend(diff);
                prev_pixel = curr_pixel;
                continue;
            }

            if let Some(luma) = Luma::new(prev_pixel, curr_pixel).to_bits() {
                encoded.extend(luma);
                prev_pixel = curr_pixel;
                continue;
            }

            // RGB if alpha is same as prev pixel
            let tag = QoiOp::RGB as u8;
            let Pixel {
                red,
                green,
                blue,
                alpha: _,
            } = curr_pixel;

            let chunk = [tag, red, green, blue];
            encoded.extend(chunk);
            prev_pixel = curr_pixel;
        }

        // if final pixels are RUN
        if let Some(rl) = run {
            encoded.push(Self::create_run_chunk(rl));
        }

        // End stream bytes
        encoded.extend([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);

        Ok(EncodedImage {
            header,
            bytes: encoded,
        })
    }

    fn create_run_chunk(run: u8) -> u8 {
        let mut chunk = 0b0000_0000;
        let tag = QoiOp::RUN as u8;
        let data = run - 1;

        chunk |= (tag << 6) & 0b1100_0000;
        chunk |= data & 0b0011_1111;
        chunk
    }

    fn create_index_chunk(index: u8) -> u8 {
        let mut chunk = 0b0000_0000;
        let tag = QoiOp::INDEX as u8;

        chunk |= (tag << 6) & 0b1100_0000;
        chunk |= index & 0b0011_1111;
        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn chunk_encode_run_length() {
        let all_unique: [u8; 12] = [
            0, 0, 0, 255, // RGBA
            255, 255, 255, 255, // RGBA
            127, 127, 127, 255, // RGBA
        ];
        let one_repeat: [u8; 12] = [
            0, 0, 0, 255, // RGBA
            0, 0, 0, 255, // RUN [1]
            127, 127, 127, 255, // RGBA
        ];
        let two_repeat: [u8; 12] = [
            0, 0, 0, 255, // RGBA
            0, 0, 0, 255, 0, 0, 0, 255, // RUN [2]
        ];
        let seen_before_no_repeat: [u8; 12] = [
            0, 0, 0, 255, // RGBA
            255, 255, 255, 255, // RGBA
            0, 0, 0, 255, // INDEX
        ];
        let index_and_run: [u8; 20] = [
            0, 0, 0, 255, // RGBA
            255, 255, 255, 255, // RGBA
            0, 0, 0, 255, // INDEX
            0, 0, 0, 255, 0, 0, 0, 255, // RUN [2]
        ];

        let cases: [(&[u8], &'static [QoiOp]); 4] = [
            (&all_unique, &[QoiOp::RGBA, QoiOp::RGBA, QoiOp::RGBA]),
            (&one_repeat, &[QoiOp::RGBA, QoiOp::RUN, QoiOp::RGBA]),
            (&two_repeat, &[QoiOp::RGBA, QoiOp::RUN]),
            (
                &seen_before_no_repeat,
                &[QoiOp::RGBA, QoiOp::RGBA, QoiOp::INDEX],
            ),
        ];

        for (case, expected) in cases {}
    }
}
